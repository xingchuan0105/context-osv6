//! Answer-contract tests (split out of `answer_contract/mod.rs`, C5-S4).

use super::*;

    #[test]
    fn code_only_detector_flags_block_answers() {
        // The observed terminal-answer failure shapes.
        assert!(is_code_only_answer(
            "<code language=\"python\">print(1)</code>"
        ));
        assert!(is_code_only_answer("```python\nprint(1)\n```"));
        assert!(is_code_only_answer("```sql\nSELECT 1\n```"));
        // Truncated stream: unclosed fence is still a code-only answer.
        assert!(is_code_only_answer("```python\nprint(1)"));
    }

    #[test]
    fn code_only_detector_accepts_prose() {
        assert!(!is_code_only_answer("答案是 LPDT-03。"));
        // Prose quoting a fenced query is a valid answer, not a violation.
        assert!(!is_code_only_answer(
            "查询结果如下：\n```sql\nSELECT 1\n```\n如上所示共 3 行。"
        ));
        // Inline `<code>` inside prose leaves prose behind.
        assert!(!is_code_only_answer("使用 <code>foo()</code> 即可。"));
        // Empty / whitespace answers are a different classification.
        assert!(!is_code_only_answer(""));
        assert!(!is_code_only_answer("  \n  "));
    }

    #[test]
    fn host_observation_shell_detector_flags_pasted_shells() {
        // q088 observed failure: fabricated host observation shell as answer.
        assert!(contains_host_observation_shell(
            "<loop_budget round=\"1\" max_rounds=\"12\" />\n\n<retrieval_summary>\nDense hits: ...\n</retrieval_summary>"
        ));
        assert!(contains_host_observation_shell("<retrieval_summary>"));
        assert!(contains_host_observation_shell("[retrieval_summary]"));
        assert!(contains_host_observation_shell("<code_execution_result>"));
        assert!(contains_host_observation_shell("<docscope_metadata>"));
        // q086 observed failure: pasted `<retrieve_cluster_index>` shell
        // (reworded inner description) instead of prose.
        assert!(contains_host_observation_shell(
            "<retrieve_cluster_index>\n- **knowledge-base**: Use dense/lexical/grep to locate the IPD\ndocument…\n</retrieve_cluster_index>"
        ));
        assert!(contains_host_observation_shell("<synthesis_skill_index>"));
        // Plain prose must never trip the format check.
        assert!(!contains_host_observation_shell(
            "验证阶段与发布阶段均有主题相关片段。"
        ));
        assert!(!contains_host_observation_shell(""));
    }

    #[test]
    fn template_artifact_detector_flags_response_tag_leak() {
        // q018 observed failure (run v2_20260802-045319): entire final answer
        // was a 12-char template-token leak.
        assert!(contains_template_artifact("`</response>"));
        assert!(contains_template_artifact("答案：</response>"));
        assert!(contains_template_artifact("<|im_end|>"));
        assert!(!contains_template_artifact("验证阶段与发布阶段均有主题相关片段。"));
        // The word "response" in prose is fine.
        assert!(!contains_template_artifact("该 response 的字段含义如下"));
    }

    #[test]
    fn executable_code_form_detector_flags_working_drafts() {
        // q095/q102 observed failure: debug narration + unexecuted code block
        // leaked as the final answer (prose present → is_code_only_answer
        // deliberately does not fire).
        assert!(contains_executable_code_form(
            "修正类型处理，稳妥打印 grep 命中。\n<code language=\"python\">\nimport asyncio\n</code>"
        ));
        // Markdown-fenced quotes are the legitimate prose form.
        assert!(!contains_executable_code_form(
            "可以这样写：\n```python\nprint(1)\n```"
        ));
        assert!(!contains_executable_code_form("答案是 LPDT-03。\n\nSELECTED: #1"));
    }

    #[test]
    fn trailing_code_fence_detector_flags_working_draft_tail() {
        // q017 observed failure (run v2_20260803-030014): debug narration +
        // an unexecuted ```python block as the tail of the final answer. It
        // slips past code_only (narration prose) and executable_code
        // (markdown fence, not the `<code language=…>` form).
        let v = check_final_answer(
            "grep 返回结构里 text 字段是 list，我的处理方式有误。先打一下原始结构确认字段形态。\n\n```python\nprint(1)\n```",
        )
        .expect("violation");
        assert_eq!(v.rule_id, "trailing_code_fence");
        // No prose at all still classifies as code_only (rule order).
        let v = check_final_answer("```python\nprint(1)\n```").expect("violation");
        assert_eq!(v.rule_id, "code_only");
        // Prose after the fence is a grounded answer.
        assert!(check_final_answer("可以这样写：\n```python\nprint(1)\n```\n如上所示。").is_none());
        // Inline code tail in prose is not a fence.
        assert!(check_final_answer("入口是 `main()`。").is_none());
        // SELECTED citation tail after a quoted block stays clean.
        assert!(check_final_answer("命中两行：\n```\nfoo\n```\n\nSELECTED: #1").is_none());
    }

    #[test]
    fn final_answer_contract_violation_covers_all_classes() {
        assert!(final_answer_contract_violation("```python\nprint(1)\n```"));
        assert!(final_answer_contract_violation("<retrieval_summary>"));
        assert!(final_answer_contract_violation("`</response>"));
        assert!(final_answer_contract_violation(
            "先看看命中。\n<code language=\"python\">\npass\n</code>"
        ));
        assert!(!final_answer_contract_violation(
            "根据回传，概念阶段第一个活动是接受任务书（LPDT-03）。\n\nSELECTED: #2"
        ));
    }

    #[test]
    fn rule_card_reports_rule_id_and_matched_marker() {
        let cases: &[(&str, &str)] = &[
            // (rule_id, offending input)
            ("code_only", "```python\nprint(1)\n```"),
            ("host_shell", "<retrieval_summary>"),
            ("template_artifact", "</response>"),
            ("executable_code", "先看看命中。\n<code language=\"python\">\npass\n</code>"),
            ("trailing_code_fence", "先确认字段形态。\n```python\npass\n```"),
        ];
        for (expected_id, text) in cases {
            let v = check_final_answer(text).expect("expected a violation");
            assert_eq!(v.rule_id, *expected_id, "input {text:?}");
            assert!(!v.matched.is_empty(), "matched marker must be non-empty");
            assert!(!v.feedback_hint.is_empty(), "feedback hint must be non-empty");
        }
    }

    #[test]
    fn rule_card_order_prefers_code_only_over_host_shell() {
        // Both classes present → the first card (code_only) wins by order.
        let v = check_final_answer("```python\n<retrieval_summary>\n```").expect("violation");
        assert_eq!(v.rule_id, "code_only");
    }

    #[test]
    fn rule_card_passes_clean_prose() {
        assert!(check_final_answer("根据回传，概念阶段第一个活动是接受任务书（LPDT-03）。\n\nSELECTED: #2").is_none());
        assert!(check_final_answer("").is_none());
    }

    /// Legacy `internal_answer_v1` envelope machinery tests: `modes/rag.yaml` is
    /// ProseOnly now (PR-A 2026-07-20 — worker final = handoff JSON). Force the
    /// historical contract so the envelope code paths stay under test.
    fn legacy_rag_mode() -> ModeConfig {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        mode.synthesis_output.contract = AnswerContractKind::InternalAnswerV1;
        mode
    }

    #[test]
    fn parses_valid_rag_json() {
        let mode = legacy_rag_mode();
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"Hi [[cite:a]]","citations":[{"chunk_id":"a"}]}"#;
        let parsed = parse_synthesis_answer(raw, &mode).unwrap();
        match parsed {
            ParsedSynthesisAnswer::Rag(a) => assert_eq!(a.citations[0].chunk_id, "a"),
            _ => panic!("expected rag"),
        }
    }

    #[test]
    fn validate_rejects_unknown_chunk() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let answer = ParsedSynthesisAnswer::Rag(InternalAnswerV1 {
            schema_version: "internal_answer_v1".to_string(),
            answer_text: "Text [[cite:missing]]".to_string(),
            citations: vec![InternalCitationV1 {
                chunk_id: "missing".to_string(),
                quote_span: None,
                confidence: None,
            }],
            coverage: Some("full".to_string()),
            refusal_reason: None,
        });
        let errors = validate_synthesis_answer(&answer, &[], &[], &mode);
        assert!(!errors.is_empty());
    }

    #[test]
    fn validates_search_combined_index_markers() {
        let mode = super::super::config::load_mode_config("search").unwrap();
        let answer = ParsedSynthesisAnswer::Search(InternalSearchAnswerV1 {
            schema_version: "internal_search_answer_v1".to_string(),
            answer_text: "Sources [[1, 2]] agree.".to_string(),
            citations: vec![
                InternalSearchCitationV1 { index: 1 },
                InternalSearchCitationV1 { index: 2 },
            ],
            coverage: Some("full".to_string()),
            refusal_reason: None,
        });
        assert!(validate_synthesis_answer(&answer, &[], &[], &mode).is_empty());
    }

    #[test]
    fn rejects_coverage_none_without_refusal_reason() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let answer = ParsedSynthesisAnswer::Rag(InternalAnswerV1 {
            schema_version: "internal_answer_v1".to_string(),
            answer_text: "No evidence.".to_string(),
            citations: vec![],
            coverage: Some("none".to_string()),
            refusal_reason: None,
        });
        let errors = validate_synthesis_answer(&answer, &[], &[], &mode);
        assert!(errors.iter().any(|e| e.contains("refusal_reason")));
    }

    #[test]
    fn lifts_rag_prose_with_cite_markers() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "abc"}]})),
            trace: None,
        }];
        let lifted = lift_prose_to_contract(
            "Antifragility means gain from disorder [[cite:abc]]",
            &tool_results,
            &[],
            &mode,
        )
        .unwrap();
        assert!(validate_synthesis_answer(&lifted, &tool_results, &[], &mode).is_empty());
    }

    #[test]
    fn contract_violation_fallback_rag_is_chinese() {
        let fallback = contract_violation_fallback("rag");
        assert!(!fallback.contains("I found"));
        assert!(
            fallback.contains('，')
                || fallback.contains('。')
                || fallback.chars().any(|c| c > '\u{4e00}')
        );
    }

    #[test]
    fn extract_partial_fallback_strips_invalid_citations() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "good"}]})),
            trace: None,
        }];
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"公司于2019年在大连建厂[[cite:good]][[cite:bad]]，营收550万元。","citations":[{"chunk_id":"good"},{"chunk_id":"bad"}],"coverage":"full","refusal_reason":null}"#;
        let partial = extract_partial_synthesis_fallback(&[raw], &tool_results, &[], &mode)
            .expect("expected partial answer");
        assert!(partial.contains("2019年在大连建厂"));
        assert!(partial.contains("[[cite:good]]"));
        assert!(!partial.contains("[[cite:bad]]"));
    }

    #[test]
    fn unwrap_synthesis_json_envelope_extracts_answer_text() {
        let raw = r#"{
  "schema_version": "internal_answer_v1",
  "answer_text": "这篇报告与最佳实践的差距在于未提及 IaC。",
  "citations": [{"chunk_id": "e8018cfe"}],
  "coverage": "full",
  "refusal_reason": null
}"#;
        let text = unwrap_synthesis_json_envelope(raw).expect("unwrap");
        assert!(text.contains("差距"));
        assert!(!text.contains("schema_version"));
    }

    #[test]
    fn unwrap_mangled_keys_without_underscores() {
        let raw = r#"{"schemaversion":"internalanswerv1","answertext":"正文[[cite:a]]与网页[[1]]EVIDENCEINSUFFICIENTFALLBACK","citations":[{"chunkid":"a"}],"coverage":"partial","refusal_reason":null}"#;
        let text = unwrap_synthesis_json_envelope(raw).expect("unwrap mangled");
        assert!(text.contains("正文"));
        assert!(!text.contains("EVIDENCEINSUFFICIENTFALLBACK"));
        assert!(text.contains("[[web:1]]") || text.contains("[[1]]"));
        assert!(!text.contains("schemaversion"));
    }

    #[test]
    fn strip_model_source_wrappers_removes_laiyuan_shells() {
        let raw = "[来源： ]** --- ## 二、框架\n根据报告[[web:4]]：\n**[来源：[[web:4]] [[web:2]]]**\n正文";
        let cleaned = strip_model_source_wrappers(raw);
        assert!(!cleaned.contains("[来源"));
        assert!(cleaned.contains("[[web:4]]") || cleaned.contains("框架"));
        assert!(cleaned.contains("正文") || cleaned.contains("框架"));
    }

    #[test]
    fn ensure_user_visible_peels_full_unified_envelope() {
        let raw = r##"{ "schema_version": "internal_answer_unified_v1", "answer_text": "差距分析：正文[[web:1]]", "citations": [ {"kind": "web", "id": "1"} ], "coverage": "full", "refusal_reason": null }"##;
        let text = ensure_user_visible_answer_text(raw);
        assert!(text.contains("差距分析"));
        assert!(text.contains("[[web:1]]"));
        assert!(!text.contains("schema_version"));
        assert!(!text.trim_start().starts_with('{'));
    }

    #[test]
    fn lift_unified_does_not_keep_envelope_as_answer_text() {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        mode.synthesis_output.contract = AnswerContractKind::InternalAnswerUnifiedV1;
        let raw = r#"{"schema_version":"internal_answer_unified_v1","answer_text":"仅正文[[web:2]]","citations":[{"kind":"web","id":"2"}],"coverage":"full","refusal_reason":null}"#;
        let lifted = lift_prose_to_contract(raw, &[], &[], &mode).expect("lift");
        let prose = render_synthesis_prose(&lifted);
        assert_eq!(prose.contains("schema_version"), false);
        assert!(prose.contains("仅正文"));
        assert!(prose.contains("[[web:2]]"));
    }

    #[test]
    fn parse_unified_contract_with_doc_and_web() {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        mode.synthesis_output.contract = AnswerContractKind::InternalAnswerUnifiedV1;
        let raw = r#"{"schema_version":"internal_answer_unified_v1","answer_text":"文档点[[cite:c1]]与网页[[web:2]]","citations":[{"kind":"doc","id":"c1"},{"kind":"web","id":"2"}],"coverage":"full","refusal_reason":null}"#;
        let parsed = parse_synthesis_answer(raw, &mode).expect("parse unified");
        match parsed {
            ParsedSynthesisAnswer::Unified(u) => {
                assert_eq!(u.citations.len(), 2);
                assert!(u.answer_text.contains("[[web:2]]"));
            }
            _ => panic!("expected unified"),
        }
    }

    #[test]
    fn resolve_sanitizes_unknown_cites_instead_of_failing() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "good"}]})),
            trace: None,
        }];
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"正文[[cite:good]]与未知[[cite:bad]]","citations":[{"chunk_id":"good"},{"chunk_id":"bad"}],"coverage":"full","refusal_reason":null}"#;
        let resolved =
            resolve_synthesis_answer(&[raw], &tool_results, &[], &mode).expect("should sanitize");
        let prose = render_synthesis_prose(&resolved);
        assert!(prose.contains("正文"));
        assert!(prose.contains("[[cite:good]]"));
        assert!(!prose.contains("[[cite:bad]]"));
        assert!(!prose.contains("schema_version"));
    }

    #[test]
    fn analytical_weiti_phrase_does_not_abort_partial_salvage() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": []})),
            trace: None,
        }];
        // 「未提及」used to false-positive as refusal and return None (leaking JSON upstream).
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"报告采用了容器化，但未提及基础设施即代码（IaC）。","citations":[{"chunk_id":"missing"}],"coverage":"full","refusal_reason":null}"#;
        let partial = extract_partial_synthesis_fallback(&[raw], &tool_results, &[], &mode)
            .expect("should salvage analytical prose");
        assert!(partial.contains("未提及"));
        assert!(!partial.contains("schema_version"));
    }

    #[test]
    fn extract_partial_fallback_returns_insufficient_zh_when_text_empty_after_strip() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "good"}]})),
            trace: None,
        }];
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"[[cite:bad]]","citations":[{"chunk_id":"bad"}],"coverage":"full","refusal_reason":null}"#;
        let partial = extract_partial_synthesis_fallback(&[raw], &tool_results, &[], &mode)
            .expect("expected insufficient fallback");
        assert_eq!(partial, partial_evidence_insufficient_zh());
    }

    #[test]
    fn extract_partial_fallback_skips_when_draft_contains_refusal() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"文档中未找到保修期限相关信息。","citations":[],"coverage":"none","refusal_reason":"not found"}"#;
        assert!(extract_partial_synthesis_fallback(&[raw], &[], &[], &mode).is_none());
    }

    #[test]
    fn extract_partial_fallback_prefers_latest_candidate() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "a"}]})),
            trace: None,
        }];
        let first = r#"{"schema_version":"internal_answer_v1","answer_text":"旧答案[[cite:missing]]","citations":[{"chunk_id":"missing"}]}"#;
        let second = r#"{"schema_version":"internal_answer_v1","answer_text":"新答案基于证据[[cite:a]]","citations":[{"chunk_id":"a"}]}"#;
        let partial =
            extract_partial_synthesis_fallback(&[first, second], &tool_results, &[], &mode)
                .expect("expected partial answer");
        assert!(partial.contains("新答案"));
        assert!(!partial.contains("旧答案"));
    }
