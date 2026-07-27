use crate::{GRAPH_CHUNK_SCORE, PgvectorDataPlane};
use pgvector::Vector;
use avrag_retrieval_data_plane::{
    GraphSearchOutput, GraphSearchRequest, RelationPathCandidate, ScoredChunk,
};
use std::collections::HashSet;
use uuid::Uuid;

impl PgvectorDataPlane {
    pub(crate) async fn search_graph_impl(
        &self,
        request: GraphSearchRequest,
    ) -> anyhow::Result<GraphSearchOutput> {
        if request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(GraphSearchOutput::default());
        }

        // Seed entities (names + query entities + ANN on entity vectors).
        let mut seed_entities: HashSet<String> = HashSet::new();
        for name in &request.entity_names {
            if !name.trim().is_empty() {
                seed_entities.insert(name.clone());
            }
        }
        for name in &request.query_entities {
            let normalized = name.trim().to_lowercase();
            if !normalized.is_empty() {
                seed_entities.insert(normalized);
            }
        }

        let owner = Uuid::parse_str(&request.owner_user_id).unwrap_or(Uuid::nil());
        // Prefer GraphSearchRequest.owner_user_id; fall back handled above.

        if !request.query_entity_vectors.is_empty() {
            for vector in &request.query_entity_vectors {
                let names = self
                    .ann_entity_names(owner, request.doc_ids.as_deref(), vector, 10)
                    .await?;
                for n in names {
                    seed_entities.insert(n);
                }
            }
        }

        if seed_entities.is_empty() {
            return Ok(GraphSearchOutput::default());
        }

        let mut visited_entities = seed_entities.clone();
        let mut current_boundary: Vec<String> = seed_entities.into_iter().collect();
        let mut all_relations = Vec::new();
        let mut supporting_chunks = Vec::new();
        let mut seen_relation_ids = HashSet::new();

        for _hop in 0..request.hop_limit {
            if current_boundary.is_empty() {
                break;
            }

            let rows = self
                .query_relations_touching(
                    owner,
                    request.doc_ids.as_deref(),
                    &current_boundary,
                    request.fan_out_limit,
                )
                .await?;

            let mut next_boundary = HashSet::new();
            for row in rows {
                if !seen_relation_ids.insert(row.relation_id) {
                    continue;
                }

                if all_relations.len() < request.relation_limit {
                    all_relations.push(RelationPathCandidate {
                        subject: row.subject.clone(),
                        predicate: row.predicate.clone(),
                        object: row.object.clone(),
                        score: GRAPH_CHUNK_SCORE,
                        supporting_chunk_ids: row.supporting_chunk_ids.clone(),
                    });

                    if supporting_chunks.len() < request.supporting_chunk_limit {
                        supporting_chunks.push(ScoredChunk {
                            chunk_id: row.relation_id,
                            doc_id: row.doc_id,
                            content: row.relation_text.clone(),
                            score: GRAPH_CHUNK_SCORE,
                            source: "pgvector_graph_relation".to_string(),
                            page: None,
                            chunk_type: "graph_relation".to_string(),
                            asset_id: None,
                            caption: None,
                            image_path: None,
                            parser_backend: None,
                            source_locator: None,
                            parse_run_id: Some(row.parse_run_id),
                        });
                    }
                }

                if !visited_entities.contains(&row.subject) {
                    next_boundary.insert(row.subject.clone());
                }
                if !visited_entities.contains(&row.object) {
                    next_boundary.insert(row.object.clone());
                }
            }

            for entity in &next_boundary {
                visited_entities.insert(entity.clone());
            }
            current_boundary = next_boundary.into_iter().collect();

            if all_relations.len() >= request.relation_limit {
                break;
            }
        }

        Ok(GraphSearchOutput {
            relation_paths: all_relations,
            supporting_chunks,
        })
    }

    async fn ann_entity_names(
        &self,
        owner: Uuid,
        doc_ids: Option<&[Uuid]>,
        vector: &[f32],
        limit: i64,
    ) -> anyhow::Result<Vec<String>> {
        if vector.is_empty() {
            return Ok(Vec::new());
        }
        let dense = Vector::from(vector.to_vec());
        let rows = if let Some(doc_ids) = doc_ids {
            sqlx::query_as::<_, (String,)>(
                r#"
                SELECT name FROM rag_kg_entities
                WHERE owner_user_id = $1 AND doc_id = ANY($2)
                ORDER BY entity_dense <=> $3
                LIMIT $4
                "#,
            )
            .bind(owner)
            .bind(doc_ids)
            .bind(&dense)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (String,)>(
                r#"
                SELECT name FROM rag_kg_entities
                WHERE owner_user_id = $1
                ORDER BY entity_dense <=> $2
                LIMIT $3
                "#,
            )
            .bind(owner)
            .bind(&dense)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }

    async fn query_relations_touching(
        &self,
        owner: Uuid,
        doc_ids: Option<&[Uuid]>,
        boundary: &[String],
        fan_out_limit: usize,
    ) -> anyhow::Result<Vec<RelationRow>> {
        let limit = fan_out_limit as i64;
        let rows = if let Some(doc_ids) = doc_ids {
            sqlx::query_as::<_, RelationRow>(
                r#"
                SELECT relation_id, doc_id, parse_run_id, subject, predicate, object,
                       relation_text, supporting_chunk_ids
                FROM rag_kg_relations
                WHERE owner_user_id = $1
                  AND doc_id = ANY($2)
                  AND (subject = ANY($3) OR object = ANY($3))
                LIMIT $4
                "#,
            )
            .bind(owner)
            .bind(doc_ids)
            .bind(boundary)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, RelationRow>(
                r#"
                SELECT relation_id, doc_id, parse_run_id, subject, predicate, object,
                       relation_text, supporting_chunk_ids
                FROM rag_kg_relations
                WHERE owner_user_id = $1
                  AND (subject = ANY($2) OR object = ANY($2))
                LIMIT $3
                "#,
            )
            .bind(owner)
            .bind(boundary)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }
}

#[derive(sqlx::FromRow)]
struct RelationRow {
    relation_id: Uuid,
    doc_id: Uuid,
    parse_run_id: Uuid,
    subject: String,
    predicate: String,
    object: String,
    relation_text: String,
    supporting_chunk_ids: Vec<Uuid>,
}
