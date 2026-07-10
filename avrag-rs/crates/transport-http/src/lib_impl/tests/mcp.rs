use super::support::*;

#[tokio::test]
async fn mcp_jsonrpc_initialize_and_tools_list() {
    let state = test_app_state();
    let notebook = state.workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "MCP Workspace".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let app = build_router(state);
    let owner_user_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let init_req = Request::builder()
        .uri(format!("/mcp/workspaces/{}", notebook.id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, owner_user_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":"1","method":"initialize","params":{}}"#,
        ))
        .unwrap();
    let init_resp = app.clone().oneshot(init_req).await.unwrap();
    assert_eq!(init_resp.status(), StatusCode::OK);
    let init_body = to_bytes(init_resp.into_body(), usize::MAX).await.unwrap();
    let init_payload: serde_json::Value = serde_json::from_slice(&init_body).unwrap();
    assert_eq!(init_payload["result"]["serverInfo"]["name"], "context-os");

    let list_req = Request::builder()
        .uri(format!("/mcp/workspaces/{}", notebook.id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, owner_user_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":"2","method":"tools/list","params":{}}"#,
        ))
        .unwrap();
    let list_resp = app.oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = to_bytes(list_resp.into_body(), usize::MAX).await.unwrap();
    let list_payload: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let tool_names: Vec<&str> = list_payload["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(tool_names.contains(&"workspace.rag_query"));
    assert!(tool_names.contains(&"workspace.search_query"));
}


#[tokio::test]
async fn unified_mcp_lists_org_and_workspace_tools() {
    let state = test_app_state();
    let app = build_router(state);
    let owner_user_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let list_req = Request::builder()
        .uri("/api/v1/mcp")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, owner_user_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}"#,
        ))
        .unwrap();
    let list_resp = app.oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = to_bytes(list_resp.into_body(), usize::MAX).await.unwrap();
    let list_payload: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let tool_names: Vec<&str> = list_payload["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(tool_names.contains(&"account.create_workspace"));
    assert!(tool_names.contains(&"workspace.create_upload"));
    assert!(tool_names.contains(&"workspace.rag_query"));
}


#[tokio::test]
async fn org_mcp_create_workspace_returns_workspace_id() {
    let state = test_app_state();
    let app = build_router(state);
    let owner_user_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let call_req = Request::builder()
        .uri("/api/v1/mcp")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, owner_user_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "tools/call",
                "params": {
                    "name": "account.create_workspace",
                    "arguments": {
                        "name": "Agent Workspace",
                        "description": "created via MCP"
                    }
                }
            })
            .to_string(),
        ))
        .unwrap();
    let call_resp = app.oneshot(call_req).await.unwrap();
    assert_eq!(call_resp.status(), StatusCode::OK);
    let body = to_bytes(call_resp.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        payload
            .pointer("/result/structuredContent/data/workspace/id")
            .and_then(|value| value.as_str())
            .is_some()
    );
    assert_eq!(
        payload
            .pointer("/result/structuredContent/agent_operation_guide/mode")
            .and_then(|value| value.as_str()),
        Some("workspace.create")
    );
}

