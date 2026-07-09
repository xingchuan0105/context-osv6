use super::super::internal::build_rag_envelope;
use super::super::*;
use contracts::chat::ChatRequest;
use contracts::RetrievalBundle;

fn request(agent_type: &str, query: &str, doc_scope: &[&str]) -> ChatRequest {
    ChatRequest {
        query: query.to_string(),
        notebook_id: None,
        session_id: None,
        agent_type: agent_type.to_string(),
        source_type: None,
        source_token: None,
        doc_scope: doc_scope.iter().map(|value| value.to_string()).collect(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
    }
}

#[test]
fn rag_envelope_formats_behavior_skill_profile_without_tools() {
    let envelope = build_rag_envelope(RagContext {
        mode: "rag-answer".to_string(),
        current_task: "summarize".to_string(),
        authoritative_context: "evidence".to_string(),
        reference_context: "none".to_string(),
        user_preference_memory: "none".to_string(),
        skill: RagBehaviorSkill {
            name: "rag-answer".to_string(),
            instructions: vec![
                "Use only RAG Evidence for factual claims.".to_string(),
                "Use preferences only for expression style.".to_string(),
            ],
        },
        output_contract: "Return natural language.".to_string(),
    });

    assert!(envelope.contains("<Behavior Skill>"));
    assert!(envelope.contains("name: rag-answer"));
    assert!(envelope.contains("- Use only RAG Evidence for factual claims."));
    assert!(!envelope.contains("<Tools>"));
    assert!(!envelope.contains("tool_schema"));
}

#[test]
fn answer_context_from_bundle_preserves_retrieval_then_summary_order() {
    let bundle = RetrievalBundle {
        chunks: vec![contracts::RetrievedChunk {
            chunk_id: "chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_type: "text".to_string(),
            page: Some(1),
            text: "retrieved".to_string(),
            score: 0.9,
            retrieval_channel: "dense".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: Vec::new(),
        }],
        graph_supported_chunks: vec![contracts::RetrievedChunk {
            chunk_id: "graph-chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_type: "text".to_string(),
            page: Some(2),
            text: "graph supported".to_string(),
            score: 0.8,
            retrieval_channel: "graph".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: Vec::new(),
        }],
        relation_paths: Vec::new(),
        citations: Vec::new(),
        summary_chunks: vec![contracts::AnswerContextChunk {
            chunk_id: "summary-doc-1".to_string(),
            doc_id: Some("doc-1".to_string()),
            chunk_type: "summary".to_string(),
            page: None,
            text: "[Document Summary] summary".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
        }],
    };

    let ctx = answer_context(&bundle);
    assert_eq!(ctx.len(), 3);
    assert_eq!(ctx[0].chunk_type, "text");
    assert_eq!(ctx[1].chunk_id, "graph-chunk-1");
    assert_eq!(ctx[2].chunk_type, "summary");
}

#[test]
fn parse_rag_plan_rejects_raw_invalid_payload_before_normalize() {
    let request = request("rag", "find rollback checklist", &["doc-1"]);
    let raw = serde_json::json!({
        "plan_version": "rag-execute-v1",
        "doc_scope": ["doc-1"],
        "items": [{
            "priority": 1.0,
            "query": "semantic lookup",
            "bm25_terms": ["exact"]
        }],
        "summary_mode": "none"
    })
    .to_string();

    assert!(parse_rag_plan_decision(&raw, &request).is_none());
}

#[test]
fn parse_rag_plan_rejects_raw_doc_scope_mismatch_before_normalize() {
    let request = request("rag", "find rollback checklist", &["doc-1"]);
    let raw = serde_json::json!({
        "plan_version": "rag-execute-v1",
        "doc_scope": ["other-doc"],
        "items": [{ "priority": 1.0, "query": "semantic lookup" }],
        "summary_mode": "none"
    })
    .to_string();

    assert!(parse_rag_plan_decision(&raw, &request).is_none());
}

#[test]
fn parse_rag_plan_accepts_new_tool_call_format() {
    let request = request("rag", "How does Atlas handle rollback?", &["doc-1"]);
    let raw = serde_json::json!({
        "calls": [
            { "tool": "dense_retrieval", "version": "1.0", "args": { "queries": ["Atlas rollback mechanism"], "modality": "text", "top_k": 10 } }
        ],
        "next_step": "answer"
    })
    .to_string();

    let decision = parse_rag_plan_decision(&raw, &request);
    assert!(
        matches!(decision, Some((RagPlanDecision::ToolCalls(ref calls), _)) if calls.len() == 1),
        "expected ToolCalls with 1 call, got {:?}",
        decision
    );
}

#[test]
fn parse_rag_plan_rejects_legacy_execute_plan_request() {
    // TN Wave 2.2 / ADR-0006: product parse path only accepts ToolCall / PlanStrategy.
    let request = request("rag", "find rollback checklist", &["doc-1"]);
    let raw = serde_json::json!({
        "plan_version": "rag-execute-v1",
        "doc_scope": ["doc-1"],
        "items": [{ "priority": 1.0, "query": "rollback checklist" }],
        "summary_mode": "none"
    })
    .to_string();

    assert!(
        parse_rag_plan_decision(&raw, &request).is_none(),
        "legacy ExecutePlanRequest must not be accepted"
    );
}

#[test]
fn parse_rag_plan_accepts_any_tool_in_new_format() {
    let request = request("rag", "read chapter 3", &["doc-1"]);
    let raw = serde_json::json!({
        "calls": [
            { "tool": "index_lookup", "version": "1.0", "args": { "doc_id": "doc-1", "chunk_ids": ["c1"] } }
        ],
        "next_step": "answer"
    })
    .to_string();

    // Phase-3c: adapter is bypassed — any valid ToolCall is accepted raw
    let decision = parse_rag_plan_decision(&raw, &request);
    assert!(
        matches!(decision, Some((RagPlanDecision::ToolCalls(ref calls), _)) if calls.len() == 1),
        "expected ToolCalls with 1 call, got {:?}",
        decision
    );
}

#[test]
fn parse_rag_plan_accepts_p4_plan_strategy_format() {
    let request = request("rag", "How does Atlas handle rollback?", &["doc-1"]);
    let raw = serde_json::json!({
        "strategy": [
            { "tool": "dense_retrieval", "queries": ["Atlas rollback mechanism"] },
            { "tool": "lexical_retrieval", "terms": ["FE-2048", "PRD"] }
        ],
        "next_step": "answer"
    })
    .to_string();

    let decision = parse_rag_plan_decision(&raw, &request);
    assert!(
        matches!(decision, Some((RagPlanDecision::Strategy(ref s), _)) if s.strategy.len() == 2),
        "expected Strategy with 2 items, got {:?}",
        decision
    );
    if let Some((RagPlanDecision::Strategy(s), _)) = decision {
        assert_eq!(s.strategy[0].tool, "dense_retrieval");
        assert_eq!(s.strategy[1].tool, "lexical_retrieval");
    }
}

#[test]
fn plan_strategy_to_tool_calls_converts_items_directly() {
    let strategy = PlanStrategy {
        strategy: vec![
            PlanStrategyItem {
                tool: "dense_retrieval".to_string(),
                params: serde_json::json!({ "queries": ["q1"], "modality": "text", "top_k": 10 }),
            },
            PlanStrategyItem {
                tool: "lexical_retrieval".to_string(),
                params: serde_json::json!({ "terms": ["a", "b"], "top_k": 5 }),
            },
        ],
        next_step: "answer".to_string(),
    };

    let calls = plan_strategy_to_tool_calls(&strategy);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].tool, "dense_retrieval");
    assert_eq!(calls[0].version, "1.0");
    assert_eq!(
        calls[0].args,
        serde_json::json!({ "queries": ["q1"], "modality": "text", "top_k": 10 })
    );
    assert_eq!(calls[1].tool, "lexical_retrieval");
}

#[test]
fn plan_strategy_to_tool_calls_handles_empty_strategy() {
    let strategy = PlanStrategy {
        strategy: vec![],
        next_step: "answer".to_string(),
    };
    let calls = plan_strategy_to_tool_calls(&strategy);
    assert!(calls.is_empty());
}
