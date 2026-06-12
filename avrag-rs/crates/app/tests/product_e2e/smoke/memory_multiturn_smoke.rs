//! S2/S3: Multi-turn anaphora + resolved_query DB write-back + on-demand memory tools.

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
    (upload.notebook_id, upload.document_id)
}

#[tokio::test]
async fn multiturn_anaphora_writes_resolved_query_to_db() {

    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let (notebook_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    let turn1_http: HttpResponse = ctx
        .chat("What is antifragility?", &notebook_id, &doc_scope)
        .await
        .unwrap();
    assert_http_ok(&turn1_http);
    let turn1: ChatResponse = turn1_http.into_business().unwrap();
    assert!(
        turn1.degrade_trace.is_empty(),
        "turn1 degrade: {:?}",
        turn1.degrade_trace
    );

    let session_id = turn1.session_id.clone();
    let follow_up = "Who wrote the book about it?";
    let turn2_http = ctx
        .chat_with_session(follow_up, &notebook_id, &doc_scope, &session_id)
        .await
        .unwrap();
    assert_http_ok(&turn2_http);
    let turn2: ChatResponse = turn2_http.into_business().unwrap();

    assert_answer_substantive(&turn2, 20);
    assert!(
        turn2.degrade_trace.is_empty(),
        "turn2 degrade: {:?}",
        turn2.degrade_trace
    );

    let (raw_content, resolved) = ctx
        .query_latest_user_resolved_query(&session_id)
        .await
        .unwrap();
    assert_eq!(raw_content, follow_up);
    let resolved = resolved.expect("resolved_query should be written for anaphoric follow-up");
    assert_ne!(
        resolved, follow_up,
        "resolved_query should expand the pronoun, got: {resolved}"
    );
    assert!(
        resolved.to_lowercase().contains("taleb")
            || resolved.to_lowercase().contains("antifragil"),
        "resolved query should anchor to prior entity, got: {resolved}"
    );
}

#[tokio::test]
async fn on_demand_conversation_history_load_returns_pg_messages() {

    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let (notebook_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    let turn1_http = ctx
        .chat("What is antifragility?", &notebook_id, &doc_scope)
        .await
        .unwrap();
    assert_http_ok(&turn1_http);
    let turn1: ChatResponse = turn1_http.into_business().unwrap();
    let session_id = turn1.session_id;

    let turn2_http = ctx
        .chat_with_session(
            "Tell me more about that concept.",
            &notebook_id,
            &doc_scope,
            &session_id,
        )
        .await
        .unwrap();
    assert_http_ok(&turn2_http);

    ctx.set_mock_rag_skill_request_memory(true);
    ctx.set_mock_emit_memory_tool(Some("conversation_history_load"));
    let turn3_http = ctx
        .chat_with_session(
            "Please recall our earlier discussion.",
            &notebook_id,
            &doc_scope,
            &session_id,
        )
        .await
        .unwrap();
    assert_http_ok(&turn3_http);
    let turn3: ChatResponse = turn3_http.into_business().unwrap();

    let history = turn3
        .tool_results
        .iter()
        .find(|r| r.tool == "conversation_history_load")
        .expect("conversation_history_load in tool_results");
    assert_eq!(history.status, contracts::chat::ToolStatus::Ok);
    let data = history.data.as_ref().expect("history tool data");
    let message_count = data
        .get("message_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert!(
        message_count > 0,
        "expected real PG history, got data: {data}"
    );
    let messages = data
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        !messages.is_empty(),
        "expected non-empty messages array, got data: {data}"
    );
}

#[tokio::test]
async fn on_demand_user_profile_load_returns_profile_shape() {

    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let (notebook_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    let turn1_http = ctx
        .chat("What is antifragility?", &notebook_id, &doc_scope)
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
            &notebook_id,
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
    let (notebook_id, doc_id) = ingest_antifragile(&mut ctx).await;
    let doc_scope = vec![doc_id];

    ctx.set_mock_rag_skill_request_memory(true);
    ctx.set_mock_emit_memory_tool(Some("conversation_history_load"));

    let http_resp = ctx
        .chat_without_mock_chunk_pin(
            "Please load my conversation history.",
            &notebook_id,
            &doc_scope,
        )
        .await
        .unwrap();
    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();

    assert_tool_result_ok(&resp, "conversation_history_load");
    assert!(
        resp.degrade_trace.iter().all(|item| item.stage != "conversation_history_load"),
        "memory tool should not degrade when session is backfilled: {:?}",
        resp.degrade_trace
    );
}
