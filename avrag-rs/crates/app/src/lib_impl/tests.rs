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
}
