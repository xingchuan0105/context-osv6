use crate::lib_impl::MilvusDataPlane;
use crate::schema::{RELATION_OUTPUT_FIELDS, doc_filter};
use crate::types::Result;
use crate::utils::{optional_uuid_field, string_field, uuid_field};
use avrag_retrieval_data_plane::{
    GraphSearchOutput, GraphSearchRequest, RelationPathCandidate, ScoredChunk,
};
use serde_json::{Value, json};
use uuid::Uuid;

impl MilvusDataPlane {
    pub(crate) async fn query_entities(
        &self,
        collection: &str,
        filter: String,
        limit: usize,
        output_fields: &[&str],
    ) -> Result<Vec<Value>> {
        let response = self
            .post_json(
                "/v2/vectordb/entities/query",
                self.with_database(json!({
                    "collectionName": collection,
                    "filter": filter,
                    "limit": limit,
                    "outputFields": output_fields
                })),
            )
            .await?;
        Ok(response["data"].as_array().cloned().unwrap_or_default())
    }

    pub async fn search_graph(
        &self,
        request: GraphSearchRequest,
    ) -> anyhow::Result<GraphSearchOutput> {
        if request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(GraphSearchOutput::default());
        }

        // 租户隔离与 ACL 过滤
        let tenant_filter = format!("org_id == '{}'", request.tenant_org_id);
        let base_filter = doc_filter(&request.auth, request.doc_ids.as_deref());
        let entity_filter = if !base_filter.is_empty() {
            format!("({}) && ({})", tenant_filter, base_filter)
        } else {
            tenant_filter
        };

        // Step 1: 确定初始实体集合 (Seed Entities)
        let mut seed_entities = std::collections::HashSet::new();
        for name in &request.entity_names {
            seed_entities.insert(name.clone());
        }

        // 实体向量搜索：通过语义相似度发现更多起点实体
        if !request.query_entity_vectors.is_empty() {
            for vector in &request.query_entity_vectors {
                let rows = self
                    .search_entities(
                        &self.config.collection_names().kg_entities,
                        "entity_dense",
                        json!([vector]),
                        entity_filter.clone(),
                        10,
                        &["name"],
                    )
                    .await?;
                for row in rows {
                    if let Some(name) = row.get("name").and_then(|v| v.as_str()) {
                        seed_entities.insert(name.to_string());
                    }
                }
            }
        }

        // 提取的实体名精确匹配补充
        for name in &request.query_entities {
            let normalized = name.trim().to_lowercase();
            if !normalized.is_empty() {
                seed_entities.insert(normalized);
            }
        }

        if seed_entities.is_empty() {
            return Ok(GraphSearchOutput::default());
        }

        // Step 2: 多跳子图扩展 (Multi-hop BFS expansion)
        let mut visited_entities = seed_entities.clone();
        let mut current_boundary: Vec<String> = seed_entities.into_iter().collect();
        let mut all_relations = Vec::new();
        let mut supporting_chunks = Vec::new();
        let mut seen_relation_ids = std::collections::HashSet::new();

        for _hop in 0..request.hop_limit {
            if current_boundary.is_empty() {
                break;
            }

            // 构造本跳的过滤条件：subject 或 object 在当前边界内
            let entities_json = json!(current_boundary).to_string();
            let hop_filter = format!(
                "({}) && (subject in {} || object in {})",
                entity_filter, entities_json, entities_json
            );

            let relation_rows = self
                .query_entities(
                    &self.config.collection_names().kg_relations,
                    hop_filter,
                    request.fan_out_limit,
                    &RELATION_OUTPUT_FIELDS,
                )
                .await?;

            let mut next_boundary = std::collections::HashSet::new();
            for row in relation_rows {
                // 去重关系 (Relation ID)
                let rid_opt = row.get("relation_id").and_then(|v| v.as_str());
                if let Some(rid) = rid_opt
                    && !seen_relation_ids.insert(rid.to_string())
                {
                    continue;
                }

                if all_relations.len() < request.relation_limit {
                    let candidate = relation_path_candidate(&row)?;
                    all_relations.push(candidate);

                    if supporting_chunks.len() < request.supporting_chunk_limit {
                        supporting_chunks
                            .push(scored_relation_chunk(&row, "milvus_graph_relation")?);
                    }
                }

                // 收集下一跳的实体起点
                if let Some(s) = row.get("subject").and_then(|v| v.as_str())
                    && !visited_entities.contains(s)
                {
                    next_boundary.insert(s.to_string());
                }
                if let Some(o) = row.get("object").and_then(|v| v.as_str())
                    && !visited_entities.contains(o)
                {
                    next_boundary.insert(o.to_string());
                }
            }

            // 更新访问状态和边界
            for entity in &next_boundary {
                visited_entities.insert(entity.clone());
            }
            current_boundary = next_boundary.into_iter().collect();

            // 如果已经达到全局关系限制，停止扩展
            if all_relations.len() >= request.relation_limit {
                break;
            }
        }

        Ok(GraphSearchOutput {
            relation_paths: all_relations,
            supporting_chunks,
        })
    }
}

pub(crate) fn relation_path_candidate(row: &Value) -> anyhow::Result<RelationPathCandidate> {
    Ok(RelationPathCandidate {
        subject: string_field(row, "subject").unwrap_or_default(),
        predicate: string_field(row, "predicate").unwrap_or_default(),
        object: string_field(row, "object").unwrap_or_default(),
        score: 1.0,
        supporting_chunk_ids: row
            .get("supporting_chunk_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub(crate) fn scored_relation_chunk(row: &Value, channel: &str) -> anyhow::Result<ScoredChunk> {
    Ok(ScoredChunk {
        chunk_id: uuid_field(row, "relation_id")?,
        doc_id: uuid_field(row, "doc_id")?,
        content: string_field(row, "relation_text").unwrap_or_default(),
        score: 1.0, // Graph matches are treated as strong matches
        source: channel.to_string(),
        page: None,
        chunk_type: "graph_relation".to_string(),
        asset_id: None,
        caption: None,
        image_path: None,
        parser_backend: None,
        source_locator: None,
        parse_run_id: optional_uuid_field(row, "parse_run_id")?,
    })
}
