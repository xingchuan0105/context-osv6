//! P0: Write-mode smoke — mock LLM routes skeleton / draft / write_refine_finish.
//!
//! Pipeline: research (Search worker + mock search/LLM) → skeleton JSON → draft
//! prose → WriteRefine finish tool → validate. Research/validation degrade is OK.

use std::collections::HashSet;
use std::time::Duration;

use crate::product_e2e::{
    ChatStreamParams, TestContext, assertions::assert_answer_substantive,
    llm_real::parse_chat_response_from_stream_events,
};

const WRITE_SMOKE_DEADLINE: Duration = Duration::from_secs(180);
const WRITE_SMOKE_MAX_EVENTS: usize = 2048;

#[tokio::test]
async fn write_mode_returns_article_under_mock_llm() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("write-smoke").await.unwrap();

    let params = ChatStreamParams {
        query: "用两百字介绍量子纠缠的基本概念",
        agent_type: "write",
        workspace_id: &notebook.id,
        doc_scope: &[],
        session_id: None,
        format_hint: None,
        debug: true,
        pin_mock_chunk_ids: false,
    };

    let events = ctx
        .chat_stream_with_params(params, WRITE_SMOKE_MAX_EVENTS, WRITE_SMOKE_DEADLINE)
        .await
        .expect("write stream under mock llm");

    // ChatEvent::Activity serializes agent stage as `phase` (see SseSink).
    let phases: HashSet<String> = events
        .iter()
        .filter(|e| e.event == "activity")
        .filter_map(|e| {
            e.data
                .get("phase")
                .or_else(|| e.data.get("stage"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();

    let stage_hit = |name: &str| -> bool {
        phases
            .iter()
            .any(|got| got == name || got.starts_with(&format!("{name}_")))
    };

    let expected_stages = ["research", "skeleton", "draft", "refine", "validate"];
    let hit = expected_stages.iter().filter(|s| stage_hit(s)).count();

    // Hardening (H1): require skeleton + a post-skeleton phase; ≥3 stages total.
    assert!(
        stage_hit("skeleton"),
        "write pipeline must emit skeleton activity; got phases: {phases:?}"
    );
    assert!(
        stage_hit("draft") || stage_hit("refine") || stage_hit("validate"),
        "write pipeline must emit draft|refine|validate after skeleton; got phases: {phases:?}"
    );
    assert!(
        hit >= 3,
        "expected ≥3 write pipeline stages among {expected_stages:?}, got activity phases: {phases:?} (hit={hit})"
    );

    let resp = parse_chat_response_from_stream_events(&events).unwrap_or_else(|| {
        let names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();
        panic!(
            "terminal done payload for write smoke missing; event types={names:?}"
        );
    });
    assert_eq!(
        resp.agent_type, "write",
        "write agent_type expected, got {}",
        resp.agent_type
    );
    assert_answer_substantive(&resp, 40);
}

/// H5: `write_refine` is an internal control ring, not a user-selectable agent_type.
#[tokio::test]
async fn write_refine_agent_type_returns_400() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("write-refine-reject").await.unwrap();

    let http_resp = ctx
        .http_client
        .post(format!("{}/api/v1/chat", ctx.base_url))
        .json(&serde_json::json!({
            "query": "should not run as write_refine agent",
            "agent_type": "write_refine",
            "workspace_id": notebook.id,
            "doc_scope": Vec::<String>::new(),
            "stream": false,
        }))
        .send()
        .await
        .expect("chat request");
    let status = http_resp.status().as_u16();
    let body_json: serde_json::Value = http_resp.json().await.expect("json body");

    assert!(
        status < 500,
        "write_refine agent_type must not 5xx; got HTTP {status}, body={body_json}"
    );
    assert_eq!(
        status, 400,
        "expected HTTP 400 for write_refine agent_type, body={body_json}"
    );
    let error = body_json
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        error, "write_refine_not_user_mode",
        "expected write_refine_not_user_mode, body={body_json}"
    );
}
