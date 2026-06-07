use common::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::agents::skills::{ExecutionContext, SkillComponent};

/// Web Search Skill — search the web for up-to-date information.
///
/// # Gotchas
/// - Results depend on the configured search provider (Brave, etc.).
/// - The `vertical` parameter only supports "web" and "news"; other values
///   are ignored by the provider.
/// - Empty queries are rejected before hitting the provider to save tokens.
pub struct WebSearchSkill;

#[async_trait::async_trait]
impl SkillComponent for WebSearchSkill {
    fn id(&self) -> &str {
        "web_search"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    /// Index-tier routing trigger.
    fn description(&self) -> &str {
        "Load when the user asks for recent information, news, or facts not in the training data."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_search".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Search the web for up-to-date information. ",
                "Use this when you need facts, news, or current data that may not be in the knowledge base.\n",
                "Rules:\n",
                "- Write a standalone, keyword-rich search query.\n",
                "- Use 'vertical' to target news results when the query is time-sensitive.\n",
                "- Call only when web search is clearly needed."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Standalone search-engine-ready query."
                    },
                    "vertical": {
                        "type": "string",
                        "enum": ["web", "news"],
                        "default": "web",
                        "description": "Search vertical: 'web' for general, 'news' for time-sensitive."
                    }
                },
                "required": ["query"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "results": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": {"type": "string"},
                                "url": {"type": "string"},
                                "snippet": {"type": "string"}
                            }
                        }
                    }
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "Results quality depends on the configured search provider. No provider = error.",
            "The 'vertical' parameter only supports 'web' and 'news'. Other values fall back to 'web'.",
            "Always use a standalone, keyword-rich query. Do not paste the user's raw conversational text.",
        ]
    }

    fn render_hint(&self) -> &str {
        "search"
    }

    async fn execute<'a>(&self, args: &Value, ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if query.is_empty() {
            return ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": "missing query" })),
                trace: None,
            };
        }

        let vertical = args.get("vertical").and_then(|v| v.as_str());

        let Some(provider) = ctx.search_provider else {
            return ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": "search provider not available" })),
                trace: None,
            };
        };

        match provider.execute_search(query, vertical).await {
            Ok(response) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Ok,
                data: serde_json::to_value(&response).ok(),
                trace: None,
            },
            Err(error) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": error.to_string() })),
                trace: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSearchProvider;

    #[async_trait::async_trait]
    impl avrag_search::SearchProvider for FakeSearchProvider {
        async fn execute_search(
            &self,
            query: &str,
            _vertical: Option<&str>,
        ) -> anyhow::Result<avrag_search::SearchResponse> {
            Ok(avrag_search::SearchResponse {
                query_type: "test".to_string(),
                sub_queries: vec![query.to_string()],
                results: vec![avrag_search::SearchResult {
                    title: format!("Result for {query}"),
                    url: format!("https://example.com/search?q={query}"),
                    snippet: "test snippet".to_string(),
                    citation_index: Some(1),
                }],
                synthesized_answer: "test answer".to_string(),
                llm_usage: None,
            })
        }
    }

    #[tokio::test]
    async fn test_web_search_basic() {
        let skill = WebSearchSkill;
        let provider = FakeSearchProvider;
        let ctx = ExecutionContext::new(Some(&provider));
        let result = skill
            .execute(&serde_json::json!({"query": "rust lang"}), &ctx)
            .await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert_eq!(data["query_type"], "test");
        assert_eq!(data["sub_queries"].as_array().unwrap().len(), 1);
        let results = data["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["title"], "Result for rust lang");
    }

    #[tokio::test]
    async fn test_web_search_no_provider() {
        let skill = WebSearchSkill;
        let ctx = ExecutionContext::new(None);
        let result = skill
            .execute(&serde_json::json!({"query": "rust"}), &ctx)
            .await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("not available"));
    }

    #[tokio::test]
    async fn test_web_search_missing_query() {
        let skill = WebSearchSkill;
        let provider = FakeSearchProvider;
        let ctx = ExecutionContext::new(Some(&provider));
        let result = skill.execute(&serde_json::json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing query"));
    }

    #[tokio::test]
    async fn test_web_search_with_vertical() {
        let skill = WebSearchSkill;
        let provider = FakeSearchProvider;
        let ctx = ExecutionContext::new(Some(&provider));
        let result = skill
            .execute(
                &serde_json::json!({"query": "news", "vertical": "news"}),
                &ctx,
            )
            .await;
        assert_eq!(result.status, ToolStatus::Ok);
    }
}
