use chrono::Utc;
use uuid::Uuid;

use crate::events::{CostEvent, CostEventName, ProductEvent, ProductEventName, ResultTag, Surface};

#[test]
fn product_event_serializes_with_required_fields() {
    let event = ProductEvent {
        event_id: Uuid::new_v4(),
        event_time: Utc::now(),
        user_id: Uuid::new_v4(),
        session_id: None,
        notebook_id: None,
        surface: Surface::Workspace,
        event_name: ProductEventName::ChatCompleted,
        result: ResultTag::Success,
        request_id: Some("req-1".to_string()),
        trace_id: Some("trace-1".to_string()),
        client_platform: "web".to_string(),
        metadata: serde_json::json!({"agent_type": "rag"}),
    };

    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["surface"], "workspace");
    assert_eq!(value["event_name"], "chat_completed");
    assert_eq!(value["result"], "success");
}

#[test]
fn cost_event_serializes_provider_and_usage_fields() {
    let event = CostEvent {
        event_id: Uuid::new_v4(),
        event_time: Utc::now(),
        user_id: Uuid::new_v4(),
        session_id: None,
        notebook_id: None,
        event_name: CostEventName::LlmUsageMetered,
        feature: "answer".to_string(),
        provider: "dmxapi".to_string(),
        model: "gemini-3.1-flash".to_string(),
        prompt_tokens: 100,
        completion_tokens: 200,
        embedding_tokens: 0,
        usage_units: 12,
        storage_bytes_delta: 0,
        external_call_count: 1,
        source: "graphflow".to_string(),
        metadata: serde_json::json!({"mode": "rag"}),
    };

    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["event_name"], "llm_usage_metered");
    assert_eq!(value["provider"], "dmxapi");
    assert_eq!(value["usage_units"], 12);
}

#[test]
fn activation_rule_requires_notebook_upload_and_chat() {
    let flags = crate::rollups::ActivationInputs {
        created_notebook: true,
        uploaded_document: true,
        completed_chat: true,
    };

    assert!(crate::rollups::is_activated(&flags));
}

#[test]
fn burst_detector_flags_short_window_replay() {
    let result = crate::anomaly::detect_request_burst(&[10, 11, 12, 13, 14], 5, 60);
    assert!(result.is_some());
}
