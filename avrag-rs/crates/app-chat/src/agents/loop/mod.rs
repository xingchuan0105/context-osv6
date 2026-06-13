use std::sync::Arc;

pub mod answer_contract;
pub mod assembler;
pub mod fallback;
pub mod policy;
pub use policy::config as config;
pub use policy::disclosure_plan as disclosure_plan;
pub use policy::exit_policy as exit_policy;
pub use policy::LoopPolicy;
pub mod hooks;
pub mod iteration;
mod iteration_codegen;
mod iteration_tools;
pub mod message_queue;
pub mod optimizer;
pub mod parse;
pub mod query_normalize;
pub mod reasoning_emit;
mod run_fallback;
mod run_prepare;
mod run_retrieval;
mod run_synthesis;
mod run_result;
pub mod skill_request;
pub mod skills;
pub mod synthesis;
pub mod telemetry;

use crate::agents::capability::CapabilityRegistry;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision};
use iteration::IterationState;
use assembler::DisclosedState;
use app_core::ChatPersistencePort;
use avrag_llm::{ChatMessage, LlmClient};
use common::AppError;
use contracts::ToolResult;
use config::ModeConfig;
use hooks::StandardLoopHooks;
use optimizer::IterationProgress;
use query_normalize::normalize_query;

pub(crate) fn merge_request_doc_scope(call: &mut contracts::ToolCall, doc_scope: &[String]) {
    if doc_scope.is_empty() {
        return;
    }
    let Some(args) = call.args.as_object_mut() else {
        return;
    };
    let scope_empty = args
        .get("doc_scope")
        .and_then(|value| value.as_array())
        .is_none_or(|items| items.is_empty());
    if scope_empty {
        args.insert("doc_scope".to_string(), serde_json::json!(doc_scope));
    }
}

pub(crate) async fn dispatch_rag_tool(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &avrag_auth::AuthContext,
    call: &contracts::ToolCall,
    doc_scope: &[String],
) -> ToolResult {
    let mut call = call.clone();
    if call.tool == "dense_retrieval" || call.tool == "lexical_retrieval" {
        merge_request_doc_scope(&mut call, doc_scope);
    }
    avrag_rag_core::runtime::tools::dispatch(runtime, auth, &call).await
}

pub struct ReActLoop {
    llm: Arc<LlmClient>,
    skill_registry: Arc<CapabilityRegistry>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    code_interpreter: Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
}

impl ReActLoop {
    pub fn new(llm: Arc<LlmClient>, skill_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            llm,
            skill_registry,
            rag_runtime: None,
            search_executor: None,
            chat_persistence: None,
            code_interpreter: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.chat_persistence = chat_persistence;
        self
    }

    fn effective_chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.chat_persistence.clone().or_else(|| {
            self.rag_runtime
                .as_ref()
                .and_then(|runtime| runtime.chat_persistence())
        })
    }

    pub fn with_rag_runtime(mut self, runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
        self.rag_runtime = runtime;
        self
    }

    pub fn with_search_executor(
        mut self,
        executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    ) -> Self {
        self.search_executor = executor;
        self
    }

    pub async fn run(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let start_time = std::time::Instant::now();
        let cancel = request.cancellation_token.clone().unwrap_or_default();
        if cancel.is_cancelled() {
            return Err(crate::agents::react_loop::cancellation_error());
        }
        let loop_exit = mode.loop_exit_for_mode();
        let hooks = StandardLoopHooks::default();

        let norm = normalize_query(&self.llm, mode, &request).await?;
        if let Some(clarify) = norm.clarify_answer {
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: clarify.clone(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(clarify.clone()),
                    usage: None,
                })
                .await;
            return Ok(AgentRunResult {
                answer: clarify.clone(),
                final_decision: Some(FinalDecision::Clarified { question: clarify }),
                ..AgentRunResult::default()
            });
        }

        let (request, base_message_count, max_iterations, auth, loop_user_query) =
            self.prepare_run_request(mode, request, norm, sink).await?;

        let mut state = IterationState {
            messages: self.build_initial_messages(mode, &request, &loop_user_query),
            disclosed: DisclosedState::default(),
            tool_results: Vec::new(),
            progress: IterationProgress::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
        };
        let (iteration, direct_answer, telemetry_records, total_usage) = self
            .run_retrieval_loop(
                mode,
                &request,
                &auth,
                &loop_exit,
                &hooks,
                base_message_count,
                max_iterations,
                &cancel,
                &mut state,
                sink,
            )
            .await?;

        let mut messages = state.messages;
        let mut disclosed_state = state.disclosed;
        let mut collected_tool_results = state.tool_results;
        let total_tool_calls = state.total_tool_calls;
        let reasoning_summary_acc = state.reasoning_acc;

        if cancel.is_cancelled() {
            return Err(crate::agents::react_loop::cancellation_error());
        }

        let retrieval_query = request.effective_query().to_string();
        if let Some(result) = self
            .resolve_synthesis_gate(
                mode,
                &loop_exit,
                &request,
                &auth,
                &retrieval_query,
                direct_answer.as_deref(),
                &mut messages,
                &mut collected_tool_results,
                &disclosed_state,
                sink,
                iteration,
                max_iterations,
                total_tool_calls,
                &telemetry_records,
                &total_usage,
                &reasoning_summary_acc,
                start_time,
            )
            .await?
        {
            return Ok(result);
        }

        self.run_synthesis_phase(
            mode,
            &request,
            &mut disclosed_state,
            &messages,
            &collected_tool_results,
            sink,
            &cancel,
            iteration,
            max_iterations,
            total_tool_calls,
            &telemetry_records,
            &total_usage,
            &reasoning_summary_acc,
            start_time,
        )
        .await
    }

}

/// Safely truncate a string to at most `max_chars` characters (not bytes).
pub(crate) fn truncate_preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Build an OpenAI-format `assistant` message carrying `tool_calls`.
/// `call_ids` must be parallel to `calls` (e.g. `call_0`, `call_1`, ...).
/// If the LLM also emitted reasoning text in `content`, it is preserved so
/// the next iteration can see the model's chain-of-thought.
pub(crate) fn build_assistant_message_with_tool_calls(
    calls: &[contracts::ToolCall],
    call_ids: &[String],
    content: &str,
    reasoning_content: Option<String>,
) -> ChatMessage {
    let openai_calls: Vec<serde_json::Value> = calls
        .iter()
        .zip(call_ids.iter())
        .map(|(call, id)| {
            serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": call.tool,
                    "arguments": serde_json::to_string(&call.args)
                        .unwrap_or_else(|_| "{}".to_string()),
                }
            })
        })
        .collect();

    ChatMessage {
        role: "assistant".to_string(),
        content: content.to_string(),
        multimodal_content: None,
        name: None,
        tool_call_id: None,
        tool_calls: Some(serde_json::json!(openai_calls)),
        reasoning_content,
    }
}

/// Build a `tool` role message from a native tool result, keyed by the
/// synthetic call id used in the assistant message.
pub(crate) fn build_tool_message(call_id: &str, tool_name: &str, result: &contracts::ToolResult) -> ChatMessage {
    let body = serde_json::json!({
        "tool": tool_name,
        "status": result.status,
        "data": result.data,
    });
    ChatMessage {
        role: "tool".to_string(),
        content: body.to_string(),
        multimodal_content: None,
        name: Some(tool_name.to_string()),
        tool_call_id: Some(call_id.to_string()),
        tool_calls: None,
        reasoning_content: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::r#loop::config::BudgetConfig;
    use std::collections::HashMap;

    #[test]
    fn assistant_tool_calls_use_openai_format() {
        let calls = vec![contracts::ToolCall {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            args: serde_json::json!({"query": "rust"}),
        }];
        let call_ids = vec!["call_0".to_string()];
        let msg = build_assistant_message_with_tool_calls(
            &calls,
            &call_ids,
            "thinking...",
            Some("internal reasoning".to_string()),
        );

        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "thinking...");
        assert_eq!(msg.reasoning_content.as_deref(), Some("internal reasoning"));
        let tc = msg.tool_calls.unwrap();
        let arr = tc.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "call_0");
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "dense_retrieval");
        assert_eq!(
            arr[0]["function"]["arguments"].as_str().unwrap(),
            r#"{"query":"rust"}"#
        );
    }

    #[test]
    fn tool_message_carries_matching_call_id() {
        let result = contracts::ToolResult {
            tool: "web_search".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"hits": 3})),
            trace: None,
        };
        let msg = build_tool_message("call_1", "web_search", &result);

        assert_eq!(msg.role, "tool");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(msg.name.as_deref(), Some("web_search"));
        assert!(msg.content.contains("\"hits\":3"));
    }

    #[test]
    fn budget_config_uses_tier_override_when_present() {
        let mut tiers = HashMap::new();
        tiers.insert("free".to_string(), 2);
        tiers.insert("pro".to_string(), 6);
        let cfg = BudgetConfig {
            max_iterations: 4,
            by_user_tier: Some(tiers),
        };
        assert_eq!(
            cfg.resolve_max_iterations(Some(&serde_json::json!("free"))),
            2
        );
        assert_eq!(
            cfg.resolve_max_iterations(Some(&serde_json::json!("PRO"))),
            6
        );
    }

    #[test]
    fn budget_config_falls_back_to_max_iterations_for_unknown_tier() {
        let mut tiers = HashMap::new();
        tiers.insert("free".to_string(), 2);
        let cfg = BudgetConfig {
            max_iterations: 4,
            by_user_tier: Some(tiers),
        };
        assert_eq!(
            cfg.resolve_max_iterations(Some(&serde_json::json!("enterprise"))),
            4
        );
    }

    #[test]
    fn budget_config_falls_back_when_no_tier() {
        let cfg = BudgetConfig {
            max_iterations: 4,
            by_user_tier: None,
        };
        assert_eq!(cfg.resolve_max_iterations(None), 4);
    }

    #[test]
    fn budget_config_clamps_to_at_least_one() {
        let cfg = BudgetConfig {
            max_iterations: 0,
            by_user_tier: None,
        };
        assert_eq!(cfg.resolve_max_iterations(None), 1);
    }

    #[test]
    fn fallback_dense_args_roundtrips() {
        let args = serde_json::to_value(contracts::DenseRetrievalArgs {
            queries: vec!["rust".to_string()],
            modality: contracts::DenseRetrievalModality::Text,
            top_k: 10,
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: contracts::DenseRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.queries, vec!["rust"]);
        assert_eq!(round.top_k, 10);
    }

    #[test]
    fn fallback_lexical_args_roundtrips() {
        let args = serde_json::to_value(contracts::LexicalRetrievalArgs {
            terms: vec!["rust".to_string(), "lang".to_string()],
            top_k: 10,
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: contracts::LexicalRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.terms, vec!["rust", "lang"]);
        assert_eq!(round.top_k, 10);
    }

    #[test]
    fn fallback_graph_args_roundtrips() {
        let args = serde_json::to_value(contracts::GraphRetrievalArgs {
            graph_hints: Vec::new(),
            placeholder_triplets: Vec::new(),
            relation_limit: 20,
            supporting_chunk_limit: 10,
            hop_limit: 1,
            fan_out_limit: 10,
            query: Some("rust".to_string()),
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: contracts::GraphRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.query.as_deref(), Some("rust"));
        assert_eq!(round.hop_limit, 1);
    }

    #[test]
    fn auto_fallback_config_deserializes_vertical() {
        let yaml = r#"
enabled: true
tool_id: web_search
top_k: 10
vertical: news
"#;
        let cfg: super::config::AutoFallbackConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.vertical.as_deref(), Some("news"));
    }

    #[test]
    fn auto_fallback_config_default_vertical_none() {
        let yaml = r#"
enabled: true
tool_id: dense_retrieval
top_k: 10
"#;
        let cfg: super::config::AutoFallbackConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(cfg.vertical.is_none());
    }
}
