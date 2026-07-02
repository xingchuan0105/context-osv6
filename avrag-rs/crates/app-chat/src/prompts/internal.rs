use super::types::*;
use contracts::AnswerContextChunk;
use contracts::{ExecutePlanItem, GraphHint, PlaceholderTriplet, QueryEntity};
use std::collections::HashSet;

pub(crate) const RAG_EXECUTE_PLAN_VERSION: &str = "rag-execute-v1";

pub(crate) fn extract_json_object(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (start <= end).then(|| raw[start..=end].to_string())
}

pub(crate) fn build_rag_envelope(context: RagContext) -> String {
    format!(
        "<Mode>\n{}\n\n<Current Task>\n{}\n\n<Authoritative Context>\n{}\n\n<Reference Context>\n{}\n\n<User Preference Memory>\n{}\n\n<Behavior Skill>\n{}\n\n<Output Contract>\n{}",
        context.mode,
        context.current_task,
        context.authoritative_context,
        context.reference_context,
        context.user_preference_memory,
        format_behavior_skill(&context.skill),
        context.output_contract,
    )
}

fn format_behavior_skill(skill: &RagBehaviorSkill) -> String {
    let instructions = if skill.instructions.is_empty() {
        "- none".to_string()
    } else {
        skill
            .instructions
            .iter()
            .map(|instruction| format!("- {instruction}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!("name: {}\ninstructions:\n{}", skill.name, instructions)
}

pub(crate) fn normalize_execute_plan_item(item: ExecutePlanItem) -> Option<ExecutePlanItem> {
    let query = item
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let bm25_terms = item.bm25_terms.map(|terms| {
        terms
            .into_iter()
            .map(|term| term.trim().to_string())
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>()
    });
    let has_query = query.is_some();
    let has_bm25_terms = bm25_terms.as_ref().is_some_and(|terms| !terms.is_empty());

    if has_query {
        Some(ExecutePlanItem {
            priority: item.priority.clamp(0.0, 1.0),
            query,
            bm25_terms: None,
        })
    } else if has_bm25_terms {
        Some(ExecutePlanItem {
            priority: item.priority.clamp(0.0, 1.0),
            query: None,
            bm25_terms,
        })
    } else {
        None
    }
}

pub(crate) fn normalize_query_entities(entities: Vec<QueryEntity>) -> Vec<QueryEntity> {
    let mut seen = HashSet::new();
    entities
        .into_iter()
        .filter_map(|entity| {
            let text = entity.text.trim().to_string();
            if text.is_empty() {
                return None;
            }
            let key = text.to_lowercase();
            if !seen.insert(key) {
                return None;
            }
            Some(QueryEntity {
                text,
                kind: entity
                    .kind
                    .as_deref()
                    .map(str::trim)
                    .filter(|kind| !kind.is_empty())
                    .map(ToOwned::to_owned),
            })
        })
        .collect()
}

pub(crate) fn normalize_graph_hints(hints: Vec<GraphHint>) -> Vec<GraphHint> {
    hints
        .into_iter()
        .filter_map(|hint| {
            let subject = hint
                .subject
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let predicate = hint
                .predicate
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let object = hint
                .object
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            (subject.is_some() || predicate.is_some() || object.is_some()).then_some(GraphHint {
                subject,
                predicate,
                object,
            })
        })
        .collect()
}

pub(crate) fn normalize_placeholder_triplets(
    triplets: Vec<PlaceholderTriplet>,
) -> Vec<PlaceholderTriplet> {
    let mut seen = HashSet::new();
    triplets
        .into_iter()
        .filter_map(|triplet| {
            let subject = triplet.subject.trim().to_string();
            let predicate = triplet.predicate.trim().to_string();
            let object = triplet.object.trim().to_string();
            if subject.is_empty() || predicate.is_empty() || object.is_empty() {
                return None;
            }
            let key = (
                subject.to_lowercase(),
                predicate.to_lowercase(),
                object.to_lowercase(),
            );
            seen.insert(key).then_some(PlaceholderTriplet {
                subject,
                predicate,
                object,
            })
        })
        .take(6)
        .collect()
}

#[allow(dead_code)]
fn build_chunk_id_reference_table(
    answer_context: &[AnswerContextChunk],
    citations: &[contracts::chat::Citation],
) -> String {
    if answer_context.is_empty() {
        return "No chunks available.".to_string();
    }

    let citation_by_chunk: std::collections::HashMap<String, &contracts::chat::Citation> =
        citations
            .iter()
            .filter_map(|c| c.chunk_id.as_ref().map(|id| (id.clone(), c)))
            .collect();

    let mut lines = vec!["Available chunk IDs for citation:".to_string()];
    for chunk in answer_context.iter().take(20) {
        let doc_name = citation_by_chunk
            .get(&chunk.chunk_id)
            .map(|c| c.doc_name.as_str())
            .unwrap_or("unknown");
        let preview = chunk.text.chars().take(80).collect::<String>();
        lines.push(format!(
            "  - CHUNK_ID: {} | Doc: {} | Preview: {}...",
            chunk.chunk_id, doc_name, preview
        ));
    }
    if answer_context.len() > 20 {
        lines.push(format!(
            "  ... and {} more chunks",
            answer_context.len() - 20
        ));
    }
    lines.push("".to_string());
    lines.push("Citation syntax:".to_string());
    lines.push("  [[cite:CHUNK_ID]] - reference a text chunk".to_string());
    lines.push("  [[image:CHUNK_ID]] - reference an image chunk".to_string());
    lines.join("\n")
}
