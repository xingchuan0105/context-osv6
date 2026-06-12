use super::internal::extract_json_object;
use super::types::*;

pub(crate) fn build_rag_strategy_evaluation_prompt(
    query: &str,
    sub_queries: &[SubQueryItem],
    tool_results: &[common::ToolResult],
    chunks: &[common::RetrievedChunk],
    iteration: u8,
    max_chunks: usize,
) -> String {
    let sub_query_lines: Vec<String> = sub_queries
        .iter()
        .map(|item| {
            let count = tool_results
                .get(item.tool_index)
                .and_then(|r| r.data.as_ref().and_then(|d| d.as_array()).map(|a| a.len()))
                .unwrap_or(0);
            let status = tool_results
                .get(item.tool_index)
                .map_or("unknown".to_string(), |r| {
                    if r.status == common::ToolStatus::Ok {
                        format!("{} results", count)
                    } else {
                        format!("{:?}", r.status)
                    }
                });
            format!("- {}: \"{}\" -> {}", item.id, item.text, status)
        })
        .collect();

    let mapped_indices: std::collections::HashSet<usize> =
        sub_queries.iter().map(|item| item.tool_index).collect();

    let extra_tools: Vec<String> = tool_results
        .iter()
        .enumerate()
        .filter(|(idx, _)| !mapped_indices.contains(idx))
        .map(|(_, r)| {
            let count = r
                .data
                .as_ref()
                .and_then(|d| d.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if r.status == common::ToolStatus::Ok {
                format!("- tool={} -> {} results", r.tool, count)
            } else {
                format!("- tool={} -> {:?}", r.tool, r.status)
            }
        })
        .collect();

    let tools_line = if !extra_tools.is_empty() {
        format!("\nAdditional tool calls:\n{}", extra_tools.join("\n"))
    } else {
        String::new()
    };

    let doc_profile_hint = {
        let has_doc_profile = tool_results
            .iter()
            .any(|r| r.tool == "doc_profile" && r.status == common::ToolStatus::Ok);
        let has_index_lookup = tool_results
            .iter()
            .any(|r| r.tool == "index_lookup" && r.status == common::ToolStatus::Ok);
        if has_doc_profile && !has_index_lookup {
            "\n\nNote: Document profile was retrieved but section content (index_lookup) has not been fetched yet. If the user's question requires reading specific sections, recommend Replan and suggest calling index_lookup with the relevant chunk_ids from the profile sections."
        } else {
            ""
        }
    };

    let top_chunks: Vec<String> = chunks
        .iter()
        .take(max_chunks)
        .enumerate()
        .map(|(i, c)| {
            format!(
                "- [{}] (score={:.2}, source={})\n  {}",
                i + 1,
                c.score,
                c.doc_id,
                c.text,
            )
        })
        .collect();

    let truncation_note = if chunks.len() > max_chunks {
        format!(
            "\n(showing top {} of {} total chunks)",
            max_chunks,
            chunks.len()
        )
    } else {
        String::new()
    };

    format!(
        "User's original question:\n{}\n\n\
         Executed sub-queries (iteration {}):\n{}{}\n\n\
         Retrieved chunks ({}):{}{}\n\n\
         Evaluate whether these chunks cover the user's question. \
         If coverage is insufficient, suggest specific follow-up queries \
         or alternative retrieval tools.{}",
        query.trim(),
        iteration + 1,
        sub_query_lines.join("\n"),
        tools_line,
        top_chunks.len(),
        truncation_note,
        top_chunks.join("\n"),
        doc_profile_hint,
    )
}

pub(crate) fn parse_rag_strategy_evaluation(raw: &str) -> Option<RagStrategyEvaluation> {
    let json = extract_json_object(raw).unwrap_or_else(|| raw.trim().to_string());
    serde_json::from_str::<RagStrategyEvaluation>(&json).ok()
}

