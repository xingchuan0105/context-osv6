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

    let expected_stages = ["research", "skeleton", "draft", "refine", "validate"];
    let hit = expected_stages
        .iter()
        .filter(|s| {
            phases
                .iter()
                .any(|got| got == *s || got.starts_with(&format!("{s}_")))
        })
        .count();
    assert!(
        hit >= 2,
        "expected ≥2 write pipeline stages among {expected_stages:?}, got activity phases: {phases:?}"
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
