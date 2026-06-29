//! PR-2 (plan §5.1): MCP agent flow — an external agent builds a workspace
//! API key, ingests a document through MCP `tools/call`, then asks a RAG
//! question and receives citations that reference the ingested document.
//!
//! Black-box over the real integration stack (real PG + Milvus + worker; mock
//! LLM/embedding). Every MCP call is driven with the API key Bearer (no test
//! proxy headers), so the agent identity — not the test identity — authorizes
//! the upload, status polling, and query.

use std::time::Duration;

use common::CreateApiKeyRequest;
use serde_json::{Value, json};

use crate::product_e2e::{
    ChatResponse, TestContext, assertions::*, mock_servers::{
        set_mock_rag_codegen_chunk_id, set_mock_rag_codegen_query,
    }, setup,
};

/// POST `/api/v1/mcp` `tools/call` with a Bearer API key.
///
/// Uses a clean reqwest client (no proxy headers) so the API key is the
/// authenticated subject, mirroring how an external agent would call the API.
pub(crate) async fn mcp_tools_call(
    ctx: &TestContext,
    bearer: &str,
    tool: &str,
    arguments: Value,
) -> (u16, Value) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("mcp reqwest client");
    let resp = client
        .post(format!("{}/api/v1/mcp", ctx.base_url))
        .header("Authorization", format!("Bearer {bearer}"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }))
        .send()
        .await
        .expect("mcp tools/call send");
    let status = resp.status().as_u16();
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
}

#[tokio::test]
async fn mcp_agent_flow_create_upload_complete_query_returns_citations() {
    super::require_integration_suite();

    let ctx = TestContext::new_smoke_with_rag().await;

    // 1. Create a workspace as the test user (plan §5.1 step 1: helper/JWT).
    let notebook = ctx.create_notebook("mcp-agent-flow").await.unwrap();

    // 2. Create a workspace API key with index + query permissions.
    let state = ctx
        .app_state
        .as_ref()
        .expect("app_state present in integration profile")
        .clone();
    let key = state
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec!["index".to_string(), "query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .expect("create workspace api key");
    let bearer = key.plaintext_key;

    // 3. MCP `workspace.create_upload` — agent initiates an upload.
    let fixture = "antifragile.txt";
    let content = setup::load_fixture(fixture).expect("load fixture");
    let bytes = content.into_bytes();
    let (status, payload) = mcp_tools_call(
        &ctx,
        &bearer,
        "workspace.create_upload",
        json!({
            "notebook_id": notebook.id,
            "filename": fixture,
            "mime_type": "text/plain",
            "file_size": bytes.len(),
        }),
    )
    .await;
    assert_eq!(status, 200, "create_upload: HTTP {status}, body: {payload}");
    let document_id = payload
        .pointer("/result/structuredContent/data/document_id")
        .and_then(|v| v.as_str())
        .expect("document_id in create_upload result")
        .to_string();

    // 4. PUT file bytes to the upload endpoint (same route the REST upload path
    //    uses; the document_id in the path authorizes the write).
    let put = ctx
        .http_client
        .put(format!("{}/dev-upload/{document_id}", ctx.base_url))
        .body(bytes)
        .send()
        .await
        .expect("upload PUT send");
    assert!(
        put.status().is_success(),
        "upload PUT failed: HTTP {}",
        put.status()
    );

    // 5. MCP `workspace.complete_upload` — finalize, hand off to the worker.
    let (status, payload) = mcp_tools_call(
        &ctx,
        &bearer,
        "workspace.complete_upload",
        json!({ "notebook_id": notebook.id, "document_id": document_id }),
    )
    .await;
    assert_eq!(
        status, 200,
        "complete_upload: HTTP {status}, body: {payload}"
    );

    // 6. Poll MCP `workspace.document_status` until the worker reports completed.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let mut last_status = String::new();
    loop {
        let (status, body) = mcp_tools_call(
            &ctx,
            &bearer,
            "workspace.document_status",
            json!({ "notebook_id": notebook.id, "document_id": document_id }),
        )
        .await;
        assert_eq!(status, 200, "document_status: HTTP {status}, body: {body}");
        let current = body
            .pointer("/result/structuredContent/data/status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        if current != last_status {
            eprintln!("[mcp_agent_flow] doc={document_id} status={current}");
            last_status = current.clone();
        }
        match current.as_str() {
            "completed" => break,
            "failed" => panic!("ingestion failed for doc={document_id}"),
            _ => {}
        }
        if tokio::time::Instant::now() > deadline {
            panic!("document_status timed out before completed (last={last_status})");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // 7. MCP `workspace.rag_query` — pin the mock to cite the ingested chunk so
    //    the mock LLM produces a citation-bearing answer (mirrors ctx.chat()).
    let query = "What is antifragility?";
    set_mock_rag_codegen_query(query);
    if let Ok(chunk_id) = ctx.query_first_chunk_id(&document_id).await {
        set_mock_rag_codegen_chunk_id(chunk_id);
    }
    let (status, payload) = mcp_tools_call(
        &ctx,
        &bearer,
        "workspace.rag_query",
        json!({ "notebook_id": notebook.id, "query": query }),
    )
    .await;
    assert_eq!(status, 200, "rag_query: HTTP {status}, body: {payload}");

    // 8. The MCP result wraps the ChatResponse under result.structuredContent.data.
    let data = payload
        .pointer("/result/structuredContent/data")
        .cloned()
        .unwrap_or(Value::Null);
    let resp: ChatResponse =
        serde_json::from_value(data).expect("parse ChatResponse from mcp rag_query result");
    assert_has_citations(&resp);
    assert_citation_doc_id(&resp, &document_id);
    assert_answer_substantive(&resp, 20);
}
