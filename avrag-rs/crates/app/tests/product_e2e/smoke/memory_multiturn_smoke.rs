//! S2/S3: Multi-turn anaphora + on-demand memory tools.
// ADR-0010: server-side query normalization removed; the previous
// `multiturn_anaphora_writes_resolved_query_to_db` test was deleted because
// the LLM now resolves anaphora on its own via the memory cluster and no
// resolved_query is written back to the DB.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext, assertions::*};

async fn ingest_antifragile(ctx: &mut TestContext) -> (String, String) {
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    assert_eq!(upload.status, 201);
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);
    (upload.workspace_id, upload.document_id)
}

#[tokio::test]
async fn on_demand_conversation_history_load_returns_pg_messages() {
    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let (workspace_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    let turn1_http = ctx
        .chat("What is antifragility?", &workspace_id, &doc_scope)
        .await
        .unwrap();
    assert_http_ok(&turn1_http);
    let turn1: ChatResponse = turn1_http.into_business().unwrap();
    let session_id = turn1.session_id;

    let turn2_http = ctx
        .chat_with_session(
            "Tell me more about that concept.",
            &workspace_id,
            &doc_scope,
            &session_id,
        )
        .await
        .unwrap();
    assert_http_ok(&turn2_http);

    let turn3_http = ctx
        .chat_with_session(
            "Can you give one more example of antifragility?",
            &workspace_id,
            &doc_scope,
            &session_id,
        )
        .await
        .unwrap();
    assert_http_ok(&turn3_http);

    ctx.set_mock_rag_skill_request_memory(true);
    ctx.set_mock_emit_memory_tool(Some("conversation_history_load"));
    let turn4_http = ctx
        .chat_with_session(
            "Please recall our earlier discussion about antifragility.",
            &workspace_id,
            &doc_scope,
            &session_id,
        )
        .await
        .unwrap();
    assert_http_ok(&turn4_http);
    let turn4: ChatResponse = turn4_http.into_business().unwrap();

    let history = turn4
        .tool_results
        .iter()
        .find(|r| r.tool == "conversation_history_load")
        .expect("conversation_history_load in tool_results");
    assert_eq!(history.status, contracts::chat::ToolStatus::Ok);
    let data: &serde_json::Value = history.data.as_ref().expect("history tool data");
    let message_count = data
        .get("message_count")
        .and_then(|v: &serde_json::Value| v.as_i64())
        .unwrap_or(0);
    assert!(
        message_count > 0,
        "expected real PG history, got data: {data}"
    );
    let messages = data
        .get("messages")
        .and_then(|v: &serde_json::Value| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        !messages.is_empty(),
        "expected non-empty messages array, got data: {data}"
    );
    assert_eq!(
        data.get("scope")
            .and_then(|v: &serde_json::Value| v.as_str()),
        Some("notebook"),
        "expected default notebook scope, got: {data}"
    );
}

#[tokio::test]
async fn notebook_scope_conversation_history_load_spans_sessions() {
    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let (workspace_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    let session_a_http = ctx
        .chat("What is antifragility?", &workspace_id, &doc_scope)
        .await
        .unwrap();
    assert_http_ok(&session_a_http);
    let session_a: ChatResponse = session_a_http.into_business().unwrap();
    let session_a = session_a.session_id;

    let search_tokens = ctx
        .query_latest_user_search_tokens(&session_a)
        .await
        .unwrap();
    assert!(
        search_tokens.as_ref().is_some_and(|t| !t.trim().is_empty()),
        "user messages should store jieba search_tokens"
    );

    let session_b_http = ctx
        .chat(
            "Start a fresh session in the same notebook.",
            &workspace_id,
            &doc_scope,
        )
        .await
        .unwrap();
    assert_http_ok(&session_b_http);
    let session_b: ChatResponse = session_b_http.into_business().unwrap();
    let session_b = session_b.session_id;
    assert_ne!(
        session_a, session_b,
        "second chat without session_id should open a new session"
    );

    ctx.set_mock_rag_skill_request_memory(true);
    ctx.set_mock_emit_memory_tool(Some("conversation_history_load"));
    let recall_http = ctx
        .chat_with_session(
            "Search notebook history for antifragility.",
            &workspace_id,
            &doc_scope,
            &session_b,
        )
        .await
        .unwrap();
    assert_http_ok(&recall_http);
    let recall: ChatResponse = recall_http.into_business().unwrap();

    let history = recall
        .tool_results
        .iter()
        .find(|r| r.tool == "conversation_history_load")
        .expect("conversation_history_load in tool_results");
    assert_eq!(history.status, contracts::chat::ToolStatus::Ok);
    let data: &serde_json::Value = history.data.as_ref().expect("history tool data");
    assert_eq!(
        data.get("scope")
            .and_then(|v: &serde_json::Value| v.as_str()),
        Some("notebook")
    );
    let messages = data
        .get("messages")
        .and_then(|v: &serde_json::Value| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        !messages.is_empty(),
        "expected notebook-scoped hits, got: {data}"
    );
    let spans_sessions = messages.iter().any(|msg: &serde_json::Value| {
        msg.get("session_id")
            .and_then(|v: &serde_json::Value| v.as_str())
            .is_some_and(|sid| sid == session_a)
    });
    let mentions_antifragility = messages.iter().any(|msg: &serde_json::Value| {
        msg.get("content")
            .and_then(|v: &serde_json::Value| v.as_str())
            .is_some_and(|c: &str| c.to_lowercase().contains("antifragil"))
    });
    assert!(
        spans_sessions || mentions_antifragility,
        "expected cross-session recall from session_a, got: {data}"
    );
}

#[tokio::test]
async fn on_demand_user_profile_load_returns_profile_shape() {
    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let (workspace_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    let turn1_http = ctx
        .chat("What is antifragility?", &workspace_id, &doc_scope)
        .await
        .unwrap();
    assert_http_ok(&turn1_http);
    let turn1: ChatResponse = turn1_http.into_business().unwrap();
    let session_id = turn1.session_id;

    ctx.set_mock_rag_skill_request_memory(true);
    ctx.set_mock_emit_memory_tool(Some("user_profile_load"));
    let turn2_http = ctx
        .chat_with_session(
            "What do you know about my preferences?",
            &workspace_id,
            &doc_scope,
            &session_id,
        )
        .await
        .unwrap();
    assert_http_ok(&turn2_http);
    let turn2: ChatResponse = turn2_http.into_business().unwrap();

    let profile = turn2
        .tool_results
        .iter()
        .find(|r| r.tool == "user_profile_load")
        .expect("user_profile_load in tool_results");
    assert_eq!(profile.status, contracts::chat::ToolStatus::Ok);
    let data = profile.data.as_ref().expect("profile tool data");
    assert!(
        data.get("structured_profile").is_some(),
        "expected structured_profile field, got: {data}"
    );
    assert!(
        data.get("expertise_domains")
            .and_then(|v| v.as_array())
            .is_some(),
        "expected expertise_domains array, got: {data}"
    );
}

/// First HTTP turn (session_id=None) can still run memory tools after pipeline session backfill.
#[tokio::test]
async fn first_turn_memory_tool_works_with_resolved_session_id() {
    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let (workspace_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    ctx.set_mock_rag_skill_request_memory(true);
    ctx.set_mock_emit_memory_tool(Some("conversation_history_load"));

    let http_resp = ctx
        .chat_without_mock_chunk_pin(
            "Please load my conversation history.",
            &workspace_id,
            &doc_scope,
        )
        .await
        .unwrap();
    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();

    assert_tool_result_ok(&resp, "conversation_history_load");
    assert!(
        resp.degrade_trace
            .iter()
            .all(|item| item.stage != "conversation_history_load"),
        "memory tool should not degrade when session is backfilled: {:?}",
        resp.degrade_trace
    );
}
