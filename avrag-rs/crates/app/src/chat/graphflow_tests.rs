#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;
    use common::{Citation, DegradeTraceItem, SourceRef, TraceInfo};

    fn request_with_mode(agent_type: &str) -> ChatRequest {
        ChatRequest {
            query: "test".to_string(),
            notebook_id: Some("notebook-1".to_string()),
            session_id: None,
            agent_type: agent_type.to_string(),
            source_type: None,
            source_token: None,
            doc_scope: vec!["notebook-1".to_string()],
            messages: vec![],
            stream: false,
        }
    }

    #[tokio::test]
    async fn mode_select_routes_memory_runtime_to_canonical_agent_task() {
        let task = ModeSelectTask {
            state: AppState::new(AppConfig::default()),
        };
        let context = Context::new();
        context.set(KEY_REQUEST, request_with_mode("search")).await;

        let result = task.run(context).await.unwrap();

        assert_eq!(
            result.next_action,
            NextAction::GoTo(TASK_SEARCH.to_string())
        );
    }

    #[tokio::test]
    async fn mode_select_keeps_memory_rag_compat_for_memory_adapters() {
        let task = ModeSelectTask {
            state: AppState::new(AppConfig::default()),
        };
        let context = Context::new();
        context.set(KEY_REQUEST, request_with_mode("rag")).await;

        let result = task.run(context).await.unwrap();

        assert_eq!(
            result.next_action,
            NextAction::GoTo(TASK_MEMORY_MODE.to_string())
        );
    }

    #[test]
    fn app_error_roundtrip_preserves_code_and_status() {
        let error = AppError::not_found("missing_thing", "thing not found");
        let graph_error = graph_app_error(error.clone());
        let mapped = map_graphflow_error(graph_error);

        assert_eq!(mapped.code(), error.code());
        assert_eq!(mapped.http_status(), error.http_status());
        assert_eq!(mapped.message(), error.message());
    }

    #[test]
    fn normalize_rag_plan_injects_original_query_as_text_dense_item() {
        let mut request = common::ExecutePlanRequest {
            plan_version: "rag-execute-v1".to_string(),
            doc_scope: vec!["doc-1".to_string()],
            items: vec![common::ExecutePlanItem {
                priority: 0.5,
                query: None,
                bm25_terms: Some(vec!["exact".to_string(), "term".to_string()]),
            }],
            summary_mode: common::ExecutePlanSummaryMode::None,
            budget: None,
            channel_budget: None,
            query_entities: Vec::new(),
            graph_hints: Vec::new(),
            placeholder_triplets: Vec::new(),
            trace: None,
        };

        request.ensure_original_query_text_dense_item("original question");

        assert_eq!(request.items[0].query.as_deref(), Some("original question"));
        assert_eq!(request.items[1].bm25_terms.as_ref().unwrap(), &vec!["exact".to_string(), "term".to_string()]);
        request.validate().unwrap();
    }

    #[tokio::test]
    async fn build_response_task_persists_final_chat_response() {
        let task = BuildResponseTask;
        let context = Context::new();
        let response = ChatResponse {
            answer: "hello".to_string(),
            answer_blocks: common::plain_text_answer_blocks("hello"),
            session_id: "session-1".to_string(),
            agent_type: "rag".to_string(),
            sources: vec![SourceRef {
                id: "source-1".to_string(),
                title: "title".to_string(),
                snippet: Some("snippet".to_string()),
                doc_id: Some("doc-1".to_string()),
                page: Some(1),
            }],
            citations: vec![Citation {
                citation_id: 1,
                doc_id: "doc-1".to_string(),
                chunk_id: Some("chunk-1".to_string()),
                page: Some(1),
                doc_name: "title".to_string(),
                preview: Some("preview".to_string()),
                content: Some("content".to_string()),
                score: 0.8,
                layer: Some("chunk".to_string()),
                chunk_type: Some("text".to_string()),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            }],
            trace: TraceInfo {
                mode: "rag".to_string(),
            },
            degrade_trace: vec![DegradeTraceItem {
                stage: "test".to_string(),
                reason: "none".to_string(),
                impact: "test".to_string(),
            }],
            planner_output: None,
            mode_debug: None,
            message_id: Some(7),
            guard_report: None,
        };
        let execution = ChatGraphExecution {
            mode: "rag".to_string(),
            input_usage_text: "hello".to_string(),
            apply_output_guard: true,
            response: response.clone(),
            llm_usage: None,
            debug_metadata: None,
        };
        context.set(KEY_EXECUTION, execution).await;

        let result = task.run(context.clone()).await.unwrap();
        let stored: ChatResponse = context.get(KEY_RESPONSE).await.unwrap();

        assert_eq!(result.next_action, NextAction::End);
        assert_eq!(result.response.as_deref(), Some("hello"));
        assert_eq!(stored.answer, response.answer);
        assert_eq!(stored.session_id, response.session_id);
    }

    #[test]
    fn graph_builder_contains_all_chat_tasks() {
        let graph = build_chat_graph(AppState::new(AppConfig::default()));

        assert!(graph.get_task(TASK_PREFLIGHT).is_some());
        assert!(graph.get_task(TASK_SESSION).is_some());
        assert!(graph.get_task(TASK_MODE_SELECT).is_some());
        assert!(graph.get_task(TASK_GENERAL).is_some());
        assert!(graph.get_task(TASK_SEARCH).is_some());
        assert!(graph.get_task(TASK_RAG_PREPARE_PLANNER_INPUT).is_some());
        assert!(graph.get_task(TASK_RAG_CALL_PLANNER).is_some());
        assert!(graph.get_task(TASK_RAG_NORMALIZE_PLAN).is_some());
        assert!(graph.get_task(TASK_RAG_EXECUTE_PLAN).is_some());
        assert!(graph.get_task(TASK_RAG_ANSWER_SYNTHESIZE).is_some());
        assert!(graph.get_task(TASK_RAG_VALIDATE_CITATIONS).is_some());
        assert!(graph.get_task(TASK_OUTPUT_GUARD).is_some());
        assert!(graph.get_task(TASK_PERSIST).is_some());
        assert!(graph.get_task(TASK_USAGE).is_some());
        assert!(graph.get_task(TASK_NOTIFY).is_some());
        assert!(graph.get_task(TASK_BUILD_RESPONSE).is_some());
    }
}
