//! Unit tests for llm_real stream observability helpers (no network).
use super::*;
use crate::product_e2e::{ChatResponse, SseEvent};

#[test]
fn collect_reasoning_summary_concatenates_deltas() {
    let events = vec![
        SseEvent {
            event: "reasoning_summary_delta".to_string(),
            data: serde_json::json!({"content": "Step 1. "}),
        },
        SseEvent {
            event: "token".to_string(),
            data: serde_json::json!({"content": "answer"}),
        },
        SseEvent {
            event: "reasoning_summary_delta".to_string(),
            data: serde_json::json!({"content": "Step 2."}),
        },
    ];
    let capture = collect_observability_from_events(&events);
    assert_eq!(capture.summary, "Step 1. Step 2.");
    assert_eq!(capture.delta_count, 2);
    assert!(capture.trace_reasoning.is_empty());
    assert!(capture.prompt_snapshots.is_empty());
}

#[test]
fn collect_observability_parses_trace_reasoning() {
    let events = vec![SseEvent {
        event: "trace".to_string(),
        data: serde_json::json!({
            "stage": "plan_decision",
            "status": "ok",
            "detail": {
                "reasoning": "Need retrieval for document QA.",
                "selected_tools": ["dense_retrieval"]
            }
        }),
    }];
    let capture = collect_observability_from_events(&events);
    assert_eq!(capture.trace_reasoning.len(), 1);
    assert_eq!(capture.trace_reasoning[0].stage, "plan_decision");
    assert_eq!(
        capture.trace_reasoning[0].reasoning,
        serde_json::json!("Need retrieval for document QA.")
    );
}

#[test]
fn collect_observability_parses_evaluation_trace() {
    let events = vec![SseEvent {
        event: "trace".to_string(),
        data: serde_json::json!({
            "stage": "evaluation",
            "status": "ok",
            "detail": {
                "decision": "native_tool_call",
                "reasoning": "2 tool calls",
                "signals": { "iteration": 0 }
            }
        }),
    }];
    let capture = collect_observability_from_events(&events);
    assert_eq!(capture.trace_reasoning.len(), 1);
    assert_eq!(capture.trace_reasoning[0].stage, "evaluation");
}

#[test]
fn collect_observability_ignores_non_string_reasoning() {
    let events = vec![SseEvent {
        event: "trace".to_string(),
        data: serde_json::json!({
            "stage": "evaluation",
            "detail": { "reasoning": { "nested": true } }
        }),
    }];
    let capture = collect_observability_from_events(&events);
    assert!(capture.trace_reasoning.is_empty());
}

#[test]
fn collect_observability_parses_prompt_snapshot() {
    let events = vec![SseEvent {
        event: "trace".to_string(),
        data: serde_json::json!({
            "stage": "prompt_snapshot",
            "status": "debug",
            "detail": {
                "phase": "retrieve",
                "iteration": 0,
                "system_content": "You are a RAG assistant."
            }
        }),
    }];
    let capture = collect_observability_from_events(&events);
    assert_eq!(capture.prompt_snapshots.len(), 1);
    assert_eq!(
        capture.prompt_snapshots[0]
            .get("phase")
            .and_then(|v| v.as_str()),
        Some("retrieve")
    );
}

#[test]
fn summarize_tool_activity_merges_sse_and_response_tools() {
    use contracts::chat::{ToolResult, ToolStatus};
    let events = vec![
        SseEvent {
            event: "trace".to_string(),
            data: serde_json::json!({
                "stage": "tool_result.code_gen",
                "status": "ok",
                "detail": { "tool": "code_gen" }
            }),
        },
        SseEvent {
            event: "trace".to_string(),
            data: serde_json::json!({
                "stage": "tool_result.dense_retrieval",
                "status": "ok",
                "detail": { "tool": "dense_retrieval" }
            }),
        },
    ];
    let resp = ChatResponse {
        answer: String::new(),
        answer_blocks: Vec::new(),
        session_id: "s1".into(),
        agent_type: "rag".into(),
        sources: Vec::new(),
        citations: Vec::new(),
        trace: contracts::chat::TraceInfo { mode: "rag".into() },
        degrade_trace: Vec::new(),
        planner_output: None,
        mode_debug: None,
        message_id: None,
        guard_report: None,
        tool_results: vec![ToolResult {
            tool: "index_lookup".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: None,
            trace: None,
        }],
        usage: None,
        agent_operation_guide: None,
    };
    let tools = summarize_tool_activity(&events, &resp);
    assert_eq!(
        tools,
        vec![
            "code_gen".to_string(),
            "dense_retrieval".to_string(),
            "index_lookup".to_string(),
        ]
    );
    assert_eq!(count_sse_trace_stage(&events, "turn_start"), 0);
}

#[test]
fn parse_chat_response_uses_last_done_payload() {
    let partial = serde_json::json!({
        "event": "done",
        "payload": {"answer": "partial", "session_id": "s1", "agent_type": "rag",
            "sources": [], "citations": [], "trace": {"mode": "rag"}, "degrade_trace": []}
    });
    let full = serde_json::json!({
        "event": "done",
        "payload": {"answer": "final", "session_id": "s1", "agent_type": "rag",
            "sources": [], "citations": [], "trace": {"mode": "rag"}, "degrade_trace": []}
    });
    let events = vec![
        SseEvent {
            event: "done".to_string(),
            data: partial,
        },
        SseEvent {
            event: "done".to_string(),
            data: full,
        },
    ];
    let resp = parse_chat_response_from_stream_events(&events).expect("parse done");
    assert_eq!(resp.answer, "final");
}

#[test]
fn classify_reqwest_error_categories() {
    assert_eq!(classify_reqwest_error("operation timed out"), "timeout");
    assert_eq!(
        classify_reqwest_error("request timed out after 30s"),
        "timeout"
    );
    assert_eq!(classify_reqwest_error("connection reset by peer"), "reset");
    assert_eq!(classify_reqwest_error("broken pipe (os error 32)"), "reset");
    assert_eq!(
        classify_reqwest_error("error connecting to 127.0.0.1:3645: connection refused"),
        "connect"
    );
    assert_eq!(classify_reqwest_error("dns error: host not found"), "other");
}
