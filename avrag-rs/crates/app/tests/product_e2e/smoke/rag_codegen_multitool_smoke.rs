//! S1: Multiround RAG codegen — doc_profile (archive) → chunk_fetch (body) → synthesis.

use crate::product_e2e::{
    ChatResponse, HttpResponse, assertions::*, fixtures::shared_ready_rag_context,
};

#[tokio::test]
async fn rag_multiround_profile_codegen_doc_profile_then_chunk_fetch() {
    super::require_smoke_suite();
    let fixture = crate::product_e2e::fixtures::shared_rag_fixture().await;
    let upload = &fixture.upload;
    let ctx = shared_ready_rag_context().await;
    ctx.reset_mock_state();

    let chunk_ids = ctx
        .query_document_chunk_ids(&upload.document_id)
        .await
        .unwrap();
    assert!(!chunk_ids.is_empty(), "expected chunk ids after ingestion");

    ctx.set_mock_rag_multiround_profile(true);
    ctx.set_mock_rag_codegen_doc_id(&upload.document_id);
    ctx.set_mock_rag_chunk_id(&chunk_ids[0]);

    let http_resp: HttpResponse = ctx
        .chat_without_mock_chunk_pin(
            "What does the opening section of this book discuss?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
        )
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();

    assert!(
        resp.degrade_trace.is_empty(),
        "multiround codegen happy path should not degrade: {:?}",
        resp.degrade_trace
    );
    assert_tool_result_ok(&resp, "doc_profile");
    assert_tool_result_ok(&resp, "index_lookup");

    let profile_idx = resp
        .tool_results
        .iter()
        .position(|r| r.tool == "doc_profile")
        .expect("doc_profile in tool_results");
    let lookup_idx = resp
        .tool_results
        .iter()
        .position(|r| r.tool == "index_lookup")
        .expect("index_lookup (chunk_fetch bridge) in tool_results");
    assert!(
        profile_idx < lookup_idx,
        "doc_profile should precede index_lookup, got order: {:?}",
        resp.tool_results
            .iter()
            .map(|r| &r.tool)
            .collect::<Vec<_>>()
    );

    assert_has_citations(&resp);
    assert_citations_use_document_chunks(&resp, &chunk_ids);
    assert_answer_substantive(&resp, 50);
}
