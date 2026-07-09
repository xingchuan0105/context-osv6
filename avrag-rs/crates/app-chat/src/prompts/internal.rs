use super::types::*;
use contracts::AnswerContextChunk;

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
