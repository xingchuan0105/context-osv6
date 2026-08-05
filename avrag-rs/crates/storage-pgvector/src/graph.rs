use crate::{GRAPH_CHUNK_SCORE, PgvectorDataPlane};
use avrag_retrieval_data_plane::{
    GraphSearchOutput, GraphSearchRequest, RelationPathCandidate, ScoredChunk,
};
use pgvector::Vector;
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
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                seed_entities.insert(trimmed.to_string());
            }
        }
        for name in &request.query_entities {
            let normalized = name.trim().to_lowercase();
            if !normalized.is_empty() {
                seed_entities.insert(normalized);
            }
        }

        let owner = Uuid::parse_str(&request.owner_user_id).unwrap_or(Uuid::nil());

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

        // Map lower(name)/normalized_name seeds → surface `name` stored on edges.
        // Combined with case-insensitive relation match (below), G1-S2 lowercase
        // query_entities expand correctly against subject/object like "Alpha".
        let resolved = self
            .resolve_entity_surface_names(owner, request.doc_ids.as_deref(), &seed_entities)
            .await?;
        seed_entities.extend(resolved);

        if seed_entities.is_empty() {
            return Ok(GraphSearchOutput::default());
        }

        // Visit key is lower(entity) so "alpha" and "Alpha" do not re-expand.
        let mut visited_lower: HashSet<String> = seed_entities
            .iter()
            .map(|s| s.to_lowercase())
            .collect();
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
                        doc_id: row.doc_id,
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
            cursor: None,
            member_chunk_ids: vec![],
                        });
                    }
                }

                let subj_l = row.subject.to_lowercase();
                let obj_l = row.object.to_lowercase();
                if !visited_lower.contains(&subj_l) {
                    next_boundary.insert(row.subject.clone());
                }
                if !visited_lower.contains(&obj_l) {
                    next_boundary.insert(row.object.clone());
                }
            }

            for entity in &next_boundary {
                visited_lower.insert(entity.to_lowercase());
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

    /// Resolve seed tokens (any case) to surface `name` values on `rag_kg_entities`.
    async fn resolve_entity_surface_names(
        &self,
        owner: Uuid,
        doc_ids: Option<&[Uuid]>,
        seeds: &HashSet<String>,
    ) -> anyhow::Result<HashSet<String>> {
        if seeds.is_empty() {
            return Ok(HashSet::new());
        }
        let seeds_lower: Vec<String> = seeds.iter().map(|s| s.to_lowercase()).collect();
        let rows = if let Some(doc_ids) = doc_ids {
            sqlx::query_as::<_, (String,)>(
                r#"
                SELECT name FROM rag_kg_entities
                WHERE owner_user_id = $1
                  AND doc_id = ANY($2)
                  AND (
                    lower(name) = ANY($3)
                    OR lower(normalized_name) = ANY($3)
                  )
                "#,
            )
            .bind(owner)
            .bind(doc_ids)
            .bind(&seeds_lower)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (String,)>(
                r#"
                SELECT name FROM rag_kg_entities
                WHERE owner_user_id = $1
                  AND (
                    lower(name) = ANY($2)
                    OR lower(normalized_name) = ANY($2)
                  )
                "#,
            )
            .bind(owner)
            .bind(&seeds_lower)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().map(|(n,)| n).collect())
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
        // Case-insensitive edge match: seeds from query_entities are lowercased
        // while relation subject/object keep surface form (e.g. "Alpha").
        let boundary_lower: Vec<String> = boundary.iter().map(|s| s.to_lowercase()).collect();
        let rows = if let Some(doc_ids) = doc_ids {
            sqlx::query_as::<_, RelationRow>(
                r#"
                SELECT relation_id, doc_id, parse_run_id, subject, predicate, object,
                       relation_text, supporting_chunk_ids
                FROM rag_kg_relations
                WHERE owner_user_id = $1
                  AND doc_id = ANY($2)
                  AND (
                    lower(subject) = ANY($3)
                    OR lower(object) = ANY($3)
                  )
                LIMIT $4
                "#,
            )
            .bind(owner)
            .bind(doc_ids)
            .bind(&boundary_lower)
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
                  AND (
                    lower(subject) = ANY($2)
                    OR lower(object) = ANY($2)
                  )
                LIMIT $3
                "#,
            )
            .bind(owner)
            .bind(&boundary_lower)
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
