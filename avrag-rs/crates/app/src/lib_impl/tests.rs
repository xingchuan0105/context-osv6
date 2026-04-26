#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_docscope_metadata_dedupes_known_profile_values() {
        let metadata = vec![
            common::SummaryMetadata {
                doc_id: "doc-1".to_string(),
                filename: "atlas-1.md".to_string(),
                docname: "Atlas One".to_string(),
                language: "zh".to_string(),
                domain: "technology".to_string(),
                genre: "manual".to_string(),
                era: "contemporary".to_string(),
            },
            common::SummaryMetadata {
                doc_id: "doc-2".to_string(),
                filename: "atlas-2.md".to_string(),
                docname: "Atlas Two".to_string(),
                language: "zh".to_string(),
                domain: "technology".to_string(),
                genre: "report".to_string(),
                era: "unknown".to_string(),
            },
            common::SummaryMetadata {
                doc_id: "doc-3".to_string(),
                filename: "atlas-3.md".to_string(),
                docname: "Atlas Three".to_string(),
                language: "en".to_string(),
                domain: "unknown".to_string(),
                genre: "".to_string(),
                era: "modern".to_string(),
            },
        ];

        let result = build_docscope_metadata(metadata.clone());

        assert_eq!(result.documents.len(), 3);
        assert_eq!(
            result.profile.languages,
            vec!["en".to_string(), "zh".to_string()]
        );
        assert_eq!(result.profile.domains, vec!["technology".to_string()]);
        assert_eq!(
            result.profile.genres,
            vec!["manual".to_string(), "report".to_string()]
        );
        assert_eq!(
            result.profile.eras,
            vec!["contemporary".to_string(), "modern".to_string()]
        );
    }

    #[test]
    fn build_rag_session_context_drops_blank_summary_and_empty_payload() {
        assert!(AppState::build_rag_session_context(Vec::new(), Some("   ".to_string())).is_none());

        let context = AppState::build_rag_session_context(
            vec![ChatMessage {
                id: 1,
                session_id: "s1".to_string(),
                role: "user".to_string(),
                content: "hello".to_string(),
                answer_blocks: Vec::new(),
                agent_id: None,
                agent_name: None,
                agent_icon: None,
                citations: Vec::new(),
                created_at: "2026-03-25T00:00:00Z".to_string(),
            }],
            Some("  carry this forward  ".to_string()),
        )
        .unwrap();

        assert_eq!(context.messages.len(), 1);
        assert_eq!(context.summary.as_deref(), Some("carry this forward"));
    }

    #[test]
    fn infer_url_import_mime_type_prefers_html_when_body_looks_like_html() {
        assert_eq!(
            infer_url_import_mime_type(
                "text/plain",
                br#"<!doctype html><html><body>Hello</body></html>"#
            ),
            "text/html"
        );
    }

    #[test]
    fn build_url_source_filename_uses_title_and_extension() {
        let url = Url::parse("https://example.com/reports/q1").unwrap();
        assert_eq!(
            build_url_source_filename(&url, "text/html", Some("Quarterly / Report")),
            "Quarterly _ Report.html"
        );
    }

    #[test]
    fn normalize_imported_text_collapses_blank_lines() {
        assert_eq!(
            normalize_imported_text("  First line \n\n\n Second line  \n"),
            "First line\nSecond line"
        );
    }

    #[test]
    fn general_profile_custom_preferences_preserves_agent_memory() {
        let mut agent_memory = common::AgentPreferenceMemory::default();
        agent_memory.active.push(common::AgentPreference {
            id: "pref-1".to_string(),
            text: "Use concise answers".to_string(),
            category: "interaction".to_string(),
            scope: "global".to_string(),
            confidence: "explicit".to_string(),
            source: "test".to_string(),
            updated_at: "2026-04-26T00:00:00Z".to_string(),
        });

        let merged = merge_general_profile_custom_preferences(
            serde_json::json!({ "theme": "dark" }),
            agent_memory,
            "hello",
            "hello refined",
        );

        assert_eq!(
            merged.get("theme").and_then(|value| value.as_str()),
            Some("dark")
        );
        assert_eq!(
            merged
                .pointer("/agent_memory/active/0/text")
                .and_then(|value| value.as_str()),
            Some("Use concise answers")
        );
        assert_eq!(
            merged
                .get("last_general_query")
                .and_then(|value| value.as_str()),
            Some("hello")
        );
    }

    #[tokio::test]
    async fn memory_compat_execute_plan_caps_total_bundle_by_final_budget() {
        let state = AppState::new(AppConfig::default());
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "budget".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();

        let mut doc_scope = Vec::new();
        for name in ["one.txt", "two.txt"] {
            let upload = state
                .create_document_upload(
                    &notebook.id,
                    CreateDocumentRequest {
                        filename: name.to_string(),
                        file_size: 32,
                        mime_type: "text/plain".to_string(),
                    },
                )
                .await
                .unwrap();
            state
                .put_uploaded_document(&upload.document_id, b"atlas rollback checklist".to_vec())
                .await
                .unwrap();
            state
                .transition_document_status(&upload.document_id, DocumentStatus::Completed)
                .await
                .unwrap();
            doc_scope.push(upload.document_id);
        }

        let response = state
            .execute_rag_execute_plan(common::ExecutePlanRequest {
                plan_version: "rag-execute-v1".to_string(),
                doc_scope,
                items: vec![common::ExecutePlanItem {
                    priority: 1.0,
                    query: Some("atlas".to_string()),
                    bm25_terms: None,
                }],
                summary_mode: common::ExecutePlanSummaryMode::All,
                budget: Some(common::ExecutePlanBudget {
                    total_candidate_budget: Some(4),
                    final_chunk_budget: Some(1),
                }),
                trace: None,
            })
            .await
            .unwrap();

        assert!(response.bundle.chunks.len() + response.bundle.summary_chunks.len() <= 1);
        assert_eq!(
            response.coverage.summary_chunk_count,
            response.bundle.summary_chunks.len()
        );
    }

    #[tokio::test]
    async fn explicit_agent_preference_is_stored_without_answer_fact_extraction() {
        let mut config = AppConfig::default();
        config.user_id = "00000000-0000-0000-0000-000000000002".to_string();
        let state = AppState::new(config);

        state
            .remember_explicit_agent_preference("remember that I prefer concise answers")
            .await
            .unwrap();
        state
            .remember_explicit_agent_preference("This answer contains a factual claim.")
            .await
            .unwrap();

        let preferences = state.current_user_preferences().await.unwrap();
        assert_eq!(preferences.agent_memory.active.len(), 1);
        assert_eq!(
            preferences.agent_memory.active[0].text,
            "I prefer concise answers"
        );
    }
}
