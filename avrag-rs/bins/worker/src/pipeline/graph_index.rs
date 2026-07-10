use contracts::auth_runtime::AuthContext;
use avrag_retrieval_data_plane::{
    DocumentIndexBatch, EntityIndexRecord, GraphPassageIndexRecord, MultimodalChunkIndexRecord,
    RelationIndexRecord, TextChunkIndexRecord,
};
use uuid::Uuid;

use super::document_pipeline::ParseRunState;
use super::helpers::record_graph_degrade;
use super::index_dispatch::embed_text_vectors;
use super::processor::PgTaskProcessor;
use super::triplet_extraction::ExtractedTriplet;

#[derive(Debug, Default)]
pub(crate) struct GraphIndexRecords {
    pub(crate) entities: Vec<EntityIndexRecord>,
    pub(crate) relations: Vec<RelationIndexRecord>,
    pub(crate) passages: Vec<GraphPassageIndexRecord>,
}

const VISUAL_TRIPLET_MIN_CONFIDENCE: f32 = 0.6;

fn triplet_confidence_threshold() -> f32 {
    std::env::var("INGESTION_TRIPLET_MIN_CONFIDENCE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(VISUAL_TRIPLET_MIN_CONFIDENCE)
}

pub(crate) async fn build_graph_index_records(
    processor: &PgTaskProcessor,
    triplets: Vec<ExtractedTriplet>,
    parse_run_state: &mut ParseRunState,
) -> GraphIndexRecords {
    let min_confidence = triplet_confidence_threshold();
    let triplets: Vec<ExtractedTriplet> = triplets
        .into_iter()
        .filter(|triplet| triplet.confidence >= min_confidence)
        .collect();

    if triplets.is_empty() {
        return GraphIndexRecords::default();
    }

    let mut entity_map: std::collections::BTreeMap<String, (String, Vec<Uuid>)> =
        std::collections::BTreeMap::new();
    for triplet in &triplets {
        for name in [&triplet.subject, &triplet.object] {
            let normalized = name.to_lowercase();
            let entry = entity_map
                .entry(normalized)
                .or_insert_with(|| (name.clone(), Vec::new()));
            for chunk_id in &triplet.supporting_chunk_ids {
                if !entry.1.contains(chunk_id) {
                    entry.1.push(*chunk_id);
                }
            }
        }
    }

    let entity_entries = entity_map.into_iter().collect::<Vec<_>>();
    let entity_texts = entity_entries
        .iter()
        .map(|(_, (name, _))| name.as_str())
        .collect::<Vec<_>>();
    let entity_vectors = match embed_text_vectors(processor, &entity_texts).await {
        Ok(vectors) => vectors,
        Err(error) => {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!("graph entity embedding failed: {error}"),
            );
            return GraphIndexRecords::default();
        }
    };

    let relation_texts = triplets
        .iter()
        .map(|triplet| {
            format!(
                "{} {} {}",
                triplet.subject, triplet.predicate, triplet.object
            )
        })
        .collect::<Vec<_>>();
    let relation_text_refs = relation_texts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let relation_vectors = match embed_text_vectors(processor, &relation_text_refs).await {
        Ok(vectors) => vectors,
        Err(error) => {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!("graph relation embedding failed: {error}"),
            );
            return GraphIndexRecords::default();
        }
    };

    let entities = entity_entries
        .into_iter()
        .zip(entity_vectors)
        .map(
            |((normalized_name, (name, supporting_chunk_ids)), vector)| EntityIndexRecord {
                entity_id: Uuid::new_v4(),
                name,
                normalized_name,
                entity_type: None,
                vector,
                supporting_chunk_ids,
                metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
            },
        )
        .collect::<Vec<_>>();

    let mut relations = Vec::with_capacity(triplets.len());
    let mut passages = Vec::with_capacity(triplets.len());
    for ((triplet, relation_text), vector) in triplets
        .into_iter()
        .zip(relation_texts)
        .zip(relation_vectors)
    {
        let relation_id = Uuid::new_v4();
        relations.push(RelationIndexRecord {
            relation_id,
            subject: triplet.subject.clone(),
            predicate: triplet.predicate.clone(),
            object: triplet.object.clone(),
            relation_text: relation_text.clone(),
            vector: vector.clone(),
            supporting_chunk_ids: triplet.supporting_chunk_ids.clone(),
            metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
        });
        // GraphPassageIndexRecord.chunk_id 只能来自合并后的真实 supporting chunk；
        // 如果没有 supporting chunk，不写该 graph passage。
        if let Some(chunk_id) = triplet.supporting_chunk_ids.first().copied() {
            passages.push(GraphPassageIndexRecord {
                passage_id: Uuid::new_v4(),
                chunk_id: Some(chunk_id),
                text: relation_text,
                vector,
                relation_ids: vec![relation_id],
                metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
            });
        }
    }

    GraphIndexRecords {
        entities,
        relations,
        passages,
    }
}

pub(crate) fn build_document_index_batch(
    context: &AuthContext,
    workspace_id: Option<Uuid>,
    document_id: Uuid,
    parse_run_id: Uuid,
    text_chunks: Vec<TextChunkIndexRecord>,
    multimodal_chunks: Vec<MultimodalChunkIndexRecord>,
    graph_records: GraphIndexRecords,
) -> DocumentIndexBatch {
    DocumentIndexBatch {
        owner_user_id: context.user_id(),
        workspace_id,
        document_id,
        parse_run_id,
        doc_version: 1,
        text_chunks,
        multimodal_chunks,
        entities: graph_records.entities,
        relations: graph_records.relations,
        graph_passages: graph_records.passages,
    }
}
