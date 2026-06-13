mod calculator;
mod code_interpreter;
mod dispatch_batch;
mod enforcement;
mod memory;
mod retry;
mod unsupported;
mod weather;
mod web_search;

pub(super) use support::{tool_call, FakeSearchProvider};

mod support {
    use contracts::ToolCall;

    pub fn tool_call(tool: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            args,
        }
    }

    pub struct FakeSearchProvider;

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
}
