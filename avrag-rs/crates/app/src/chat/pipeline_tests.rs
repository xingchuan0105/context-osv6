// Tests for the linear chat pipeline (replacement for graphflow_tests.rs).
//
// Migration notes:
// - `mode_select_routes_*` (graphflow) → `dispatch_*` here. The new pipeline
//   does not have a separate ModeSelectTask; routing is inlined in
//   `dispatch_mode`, so the same decisions are tested by inspecting the
//   `ChatExecution` returned for each agent_type / doc_scope shape.
// - `app_error_roundtrip_preserves_code_and_status` was deleted: there is no
//   `graph_flow::Error` bridge anymore — `AppError` now propagates directly.
// - `graph_builder_contains_all_chat_tasks` was deleted: there is no graph.
// - `build_response_task_persists_final_chat_response` was deleted: response
//   construction is now a plain return at the end of `run_pipeline`, not a
//   task that mutates a Context map.
// - `normalize_rag_plan_injects_original_query_as_text_dense_item` is already
//   covered by `crates/common/tests/rag_execute_contract.rs`.

#[cfg(test)]
mod tests {
    use crate::AppConfig;
    use crate::AppState;
    use crate::chat::pipeline_steps::dispatch_mode;
    use common::{ChatRequest, ChatSession, CreateNotebookRequest, now_rfc3339};

    fn request_with_mode(agent_type: &str, doc_scope: Vec<String>) -> ChatRequest {
        ChatRequest {
            query: "test".to_string(),
            notebook_id: Some("notebook-1".to_string()),
            session_id: None,
            agent_type: agent_type.to_string(),
            source_type: None,
            source_token: None,
            doc_scope,
            messages: vec![],
            stream: false,
            language: None,
        }
    }

    fn session_for(agent_type: &str) -> ChatSession {
        let now = now_rfc3339();
        ChatSession {
            id: "session-1".to_string(),
            notebook_id: "notebook-1".to_string(),
            title: None,
            agent_type: agent_type.to_string(),
            summary: None,
            pinned: false,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn dispatch_rag_without_docscope_returns_clarify_response() {
        let state = AppState::new(AppConfig::default());
        let request = request_with_mode("rag", vec![]);
        let session = session_for("rag");

        let execution = dispatch_mode(&state, &request, &session, None).await.unwrap();

        // Clarify shortcut: no citations, no output guard, mode echoes agent_type.
        assert_eq!(execution.mode, "rag");
        assert!(!execution.apply_output_guard);
        assert!(execution.response.citations.is_empty());
        assert!(execution.response.sources.is_empty());
        assert!(!execution.response.answer.is_empty());
    }

    #[tokio::test]
    async fn dispatch_rag_with_memory_adapters_uses_memory_chat_compat() {
        // AppState::new without pg ⇒ uses_memory_adapters() == true.
        let state = AppState::new(AppConfig::default());
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "Test Notebook".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();
        let request = request_with_mode("rag", vec![notebook.id.clone()]);
        let mut session = session_for("rag");
        session.notebook_id = notebook.id;

        let execution = dispatch_mode(&state, &request, &session, None).await.unwrap();

        assert_eq!(execution.mode, "rag");
        assert_eq!(execution.response.session_id, session.id);
        assert!(!execution.apply_output_guard);
    }
}
