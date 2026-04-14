use avrag_llm::{ChatMessage, LlmUsage};

use crate::SearchConfig;

#[derive(Debug, Clone)]
pub(crate) struct SearchPlan {
    pub query_type: String,
    pub sub_queries: Vec<String>,
}

pub(crate) async fn plan_query(query: &str, config: &SearchConfig) -> anyhow::Result<SearchPlan> {
    let (plan, _) = plan_query_with_usage(query, config).await?;
    Ok(plan)
}

pub(crate) async fn plan_query_with_usage(
    query: &str,
    config: &SearchConfig,
) -> anyhow::Result<(SearchPlan, Option<LlmUsage>)> {
    if config.mode == "llm_tools"
        && config.planner_enabled
        && config.planner_llm.is_some()
        && let Ok((plan, usage)) = plan_query_with_llm(query, config).await
        && !plan.sub_queries.is_empty()
    {
        return Ok((plan, Some(usage)));
    }
    Ok((plan_query_heuristically(query, config), None))
}

pub(crate) async fn plan_query_with_llm(
    query: &str,
    config: &SearchConfig,
) -> anyhow::Result<(SearchPlan, LlmUsage)> {
    let planner = config
        .planner_llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("search planner llm not configured"))?;
    let response = planner
        .complete(
            &[
                ChatMessage::system(
                    r#"You plan web search queries. Return strict JSON:
{
  "query_type": "freshness|fact|navigational|comparative|broad",
  "sub_queries": ["...", "..."]
}
Rules:
- sub_queries must contain 1 to 3 concrete web search queries.
- Preserve named entities and exact dates.
- Add freshness wording when the user asks for latest, recent, today, this week, this month.
- Do not include markdown or explanation."#,
                ),
                ChatMessage::user(query.to_string()),
            ],
            Some(0.2),
        )
        .await?;

    #[derive(serde::Deserialize)]
    struct Planned {
        query_type: Option<String>,
        sub_queries: Option<Vec<String>>,
    }

    let plan: Planned = serde_json::from_str(response.content.trim())?;
    let mut sub_queries = dedup_queries(
        plan.sub_queries
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        config.max_sub_queries,
    );
    if sub_queries.is_empty() {
        sub_queries.push(query.trim().to_string());
    }
    Ok((SearchPlan {
        query_type: plan
            .query_type
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| classify_query_type(query)),
        sub_queries,
    }, response.usage))
}

pub(crate) fn plan_query_heuristically(query: &str, config: &SearchConfig) -> SearchPlan {
    let query_type = if config.query_type_enabled {
        classify_query_type(query)
    } else {
        "broad".to_string()
    };
    let mut sub_queries = vec![query.trim().to_string()];
    let lowered = query.to_lowercase();
    if query_type == "freshness" && !lowered.contains("latest") {
        sub_queries.push(format!("{query} latest"));
    }
    if query_type == "comparative" && lowered.contains(" vs ") {
        sub_queries.push(query.replace(" vs ", " comparison "));
    }
    if let Some(stripped) = strip_search_noise(query) {
        sub_queries.push(stripped);
    }
    SearchPlan {
        query_type,
        sub_queries: dedup_queries(sub_queries, config.max_sub_queries),
    }
}

fn classify_query_type(query: &str) -> String {
    let lowered = query.to_lowercase();
    if [
        "latest",
        "recent",
        "today",
        "this week",
        "this month",
        "news",
    ]
    .iter()
    .any(|token| lowered.contains(token))
    {
        return "freshness".to_string();
    }
    if lowered.contains(" vs ") || lowered.contains("compare") || lowered.contains("difference") {
        return "comparative".to_string();
    }
    if lowered.starts_with("what is ")
        || lowered.starts_with("who is ")
        || lowered.starts_with("when is ")
        || lowered.starts_with("where is ")
    {
        return "fact".to_string();
    }
    if lowered.contains("official site") || lowered.contains("homepage") {
        return "navigational".to_string();
    }
    "broad".to_string()
}

fn strip_search_noise(query: &str) -> Option<String> {
    let stripped = query
        .replace("please", "")
        .replace("could you", "")
        .replace("can you", "")
        .replace("search for", "")
        .trim()
        .to_string();
    if stripped.is_empty() || stripped.eq_ignore_ascii_case(query.trim()) {
        return None;
    }
    Some(stripped)
}

fn dedup_queries(queries: Vec<String>, max_sub_queries: usize) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    queries
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .filter(|item| seen.insert(item.to_lowercase()))
        .take(max_sub_queries.max(1))
        .collect()
}
