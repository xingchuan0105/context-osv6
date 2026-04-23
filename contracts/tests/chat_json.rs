use contracts::chat::{
    AnswerBlock, ChatDonePayload, ChatEvent, ChatRequest, ChatResponse, Citation, DegradeTraceItem,
    SourceRef, TraceInfo,
};

#[test]
fn chat_request_deserializes_with_minimal_defaults_and_no_request_id_field() {
    let json = serde_json::json!({
        "query": "hello"
    });

    let request: ChatRequest = serde_json::from_value(json).expect("request should deserialize");

    assert!(!request.stream, "stream should default to false");

    let serialized = serde_json::to_value(request).expect("request should serialize");
    assert_eq!(
        serialized,
        serde_json::json!({
            "query": "hello",
            "notebook_id": null,
            "session_id": null,
            "agent_type": "rag",
            "source_type": null,
            "source_token": null,
            "doc_scope": [],
            "messages": [],
            "stream": false
        })
    );
    assert!(
        serialized.get("request_id").is_none(),
        "request_id should not be part of the shared contract"
    );
}

#[test]
fn chat_event_serializes_with_a_stable_event_tag() {
    let event = ChatEvent::Start {
        request_id: "req-123".to_string(),
        session_id: "session-123".to_string(),
    };

    let json = serde_json::to_value(event).expect("event should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "event": "start",
            "request_id": "req-123",
            "session_id": "session-123"
        })
    );
}

#[test]
fn chat_event_roundtrips_supported_variants() {
    let cases = vec![
        ChatEvent::Start {
            request_id: "req-123".to_string(),
            session_id: "session-123".to_string(),
        },
        ChatEvent::Trace {
            request_id: "req-123".to_string(),
            stage: "planner".to_string(),
            status: "ok".to_string(),
            detail: Some(serde_json::json!({"step": "trace"})),
        },
        ChatEvent::Token {
            request_id: "req-123".to_string(),
            message_id: 7,
            content: "hello".to_string(),
        },
        ChatEvent::Citations {
            request_id: "req-123".to_string(),
            message_id: 7,
            citations: vec![serde_json::json!({"citation_id": 1})],
        },
        ChatEvent::Done {
            request_id: "req-123".to_string(),
            session_id: "session-123".to_string(),
            message_id: 7,
            payload: serde_json::json!({"status": "done"}),
        },
        ChatEvent::Error {
            request_id: "req-123".to_string(),
            code: "bad_request".to_string(),
            message: "boom".to_string(),
        },
    ];

    for event in cases {
        let json = serde_json::to_value(&event).expect("event should serialize");
        let parsed: ChatEvent = serde_json::from_value(json).expect("event should deserialize");
        assert_eq!(parsed, event);
    }
}

#[test]
fn legacy_type_tagged_planner_complete_event_is_rejected() {
    let json = serde_json::json!({
        "type": "planner_complete",
        "payload": {"status": "done"}
    });

    let parsed = serde_json::from_value::<ChatEvent>(json);

    assert!(
        parsed.is_err(),
        "legacy planner_complete payload should be rejected"
    );
}

#[test]
fn error_event_exposes_request_id_and_stable_code() {
    let event = ChatEvent::Error {
        request_id: "req-err".to_string(),
        code: "validation_error".to_string(),
        message: "boom".to_string(),
    };

    let json = serde_json::to_value(event).expect("error event should serialize");
    assert_eq!(json["request_id"], "req-err");
    assert_eq!(json["code"], "validation_error");
    assert_eq!(json["message"], "boom");
}

#[test]
fn chat_response_roundtrips_shared_nested_types() {
    let response = ChatResponse {
        answer: "hello".to_string(),
        answer_blocks: vec![AnswerBlock::Text {
            text: "hello".to_string(),
            citations: vec!["1".to_string()],
        }],
        session_id: "session-123".to_string(),
        agent_type: "rag".to_string(),
        sources: vec![SourceRef {
            id: "chunk-1".to_string(),
            title: "Doc".to_string(),
            snippet: Some("snippet".to_string()),
            doc_id: Some("doc-1".to_string()),
            page: Some(1),
        }],
        citations: vec![Citation {
            citation_id: 1,
            doc_id: "doc-1".to_string(),
            chunk_id: Some("chunk-1".to_string()),
            page: Some(1),
            doc_name: "Doc".to_string(),
            preview: Some("preview".to_string()),
            content: Some("content".to_string()),
            score: 0.9,
            layer: Some("summary".to_string()),
            chunk_type: Some("text".to_string()),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
        }],
        trace: TraceInfo {
            mode: "rag".to_string(),
        },
        degrade_trace: vec![DegradeTraceItem {
            stage: "planner".to_string(),
            reason: "fallback".to_string(),
            impact: "quality".to_string(),
        }],
        planner_output: None,
        mode_debug: None,
        message_id: Some(7),
        guard_report: None,
    };

    let json = serde_json::to_value(&response).expect("response should serialize");
    let parsed: ChatResponse = serde_json::from_value(json).expect("response should deserialize");
    assert_eq!(parsed.answer, "hello");
    assert_eq!(parsed.message_id, Some(7));
    assert_eq!(parsed.citations.len(), 1);
    assert_eq!(parsed.sources.len(), 1);
}

#[test]
fn done_payload_exposes_terminal_response_fields() {
    let payload = ChatDonePayload {
        request_id: "req-123".to_string(),
        session_id: "session-123".to_string(),
        message_id: 7,
        response: ChatResponse {
            answer: "done".to_string(),
            answer_blocks: Vec::new(),
            session_id: "session-123".to_string(),
            agent_type: "general".to_string(),
            sources: Vec::new(),
            citations: Vec::new(),
            trace: TraceInfo {
                mode: "general".to_string(),
            },
            degrade_trace: Vec::new(),
            planner_output: None,
            mode_debug: None,
            message_id: Some(7),
            guard_report: None,
        },
    };

    let json = serde_json::to_value(payload).expect("done payload should serialize");
    assert_eq!(json["request_id"], "req-123");
    assert_eq!(json["session_id"], "session-123");
    assert_eq!(json["message_id"], 7);
    assert_eq!(json["response"]["answer"], "done");
}
