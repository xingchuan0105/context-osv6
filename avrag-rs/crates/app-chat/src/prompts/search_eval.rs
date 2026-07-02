use super::internal::extract_json_object;
use super::types::*;
use avrag_search::SearchResult;

pub(crate) fn build_search_strategy_evaluation_prompt(
    query: &str,
    vertical: Option<&str>,
    sub_queries: &[String],
    results: &[SearchResult],
    accumulated_count: usize,
    iteration: u8,
    max_results: usize,
) -> String {
    let sub_query_lines: Vec<String> = sub_queries
        .iter()
        .enumerate()
        .map(|(i, sq)| format!("- q{}: \"{}\"", i + 1, sq))
        .collect();

    let vertical_line = vertical
        .map(|v| format!("\nVertical used: {}", v))
        .unwrap_or_default();

    let top_results: Vec<String> = results
        .iter()
        .take(max_results)
        .enumerate()
        .map(|(i, r)| {
            format!(
                "- [{}] {}\n  {}\n  URL: {}",
                i + 1,
                r.title,
                r.snippet,
                r.url,
            )
        })
        .collect();

    let results_header = if results.len() > max_results {
        format!(
            "({}  (showing top {} of {} total))",
            top_results.len(),
            max_results,
            results.len()
        )
    } else {
        format!("({})", top_results.len())
    };

    format!(
        "User's original question:\n{}\n\n\
         Executed search queries (iteration {}):{}\n\n\
         Actual results {}:{}{}\n\n\
         Accumulated unique sources so far: {}\n\n\
         Evaluate whether these results cover the user's question. \
         If coverage is insufficient, suggest specific follow-up queries \
         or alternative search approaches.",
        query.trim(),
        iteration + 1,
        sub_query_lines.join("\n"),
        results_header,
        vertical_line,
        top_results.join("\n"),
        accumulated_count,
    )
}

pub(crate) fn parse_search_strategy_evaluation(raw: &str) -> Option<SearchStrategyEvaluation> {
    let json = extract_json_object(raw).unwrap_or_else(|| raw.trim().to_string());
    serde_json::from_str::<SearchStrategyEvaluation>(&json).ok()
}
