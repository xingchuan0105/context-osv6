use web_sdk::{
    BrowserRestClient, TransportError, create_note_json, create_workspace_json,
    parse_workspace_list, workspace_document_url, workspace_documents_url, workspace_note_url,
    workspace_notes_url, workspace_url, workspaces_url,
};

#[test]
fn workspace_urls_are_canonical() {
    assert_eq!(
        workspaces_url("http://x/"),
        "http://x/api/v1/workspaces"
    );
    assert_eq!(
        workspace_url("", "ws-1"),
        "/api/v1/workspaces/ws-1"
    );
    assert_eq!(
        workspace_documents_url("http://x", "ws-1"),
        "http://x/api/v1/workspaces/ws-1/documents"
    );
    assert_eq!(
        workspace_document_url("http://x", "ws-1", "doc-1"),
        "http://x/api/v1/workspaces/ws-1/documents/doc-1"
    );
    assert_eq!(
        workspace_notes_url("http://x", "ws-1"),
        "http://x/api/v1/workspaces/ws-1/notes"
    );
    assert_eq!(
        workspace_note_url("http://x", "ws-1", "note-1"),
        "http://x/api/v1/workspaces/ws-1/notes/note-1"
    );
}

#[test]
fn json_payload_generation_matches_contracts() {
    let ws_body = create_workspace_json("研发部", "核心资料库").expect("json");
    let val: serde_json::Value = serde_json::from_slice(&ws_body).expect("parse");
    assert_eq!(val["name"], "研发部");
    assert_eq!(val["description"], "核心资料库");

    let note_body = create_note_json("会议纪要", "# 讨论内容").expect("json");
    let val2: serde_json::Value = serde_json::from_slice(&note_body).expect("parse");
    assert_eq!(val2["title"], "会议纪要");
    assert_eq!(val2["content"], "# 讨论内容");
}

#[test]
fn parse_workspace_list_from_wire() {
    let raw = r#"{
        "workspaces": [
            {
                "id": "ws-1",
                "owner_user_id": "u1",
                "owner_id": "u1",
                "name": "材料研发",
                "title": "材料研发",
                "description": "说明",
                "created_at": "2026-09-01T00:00:00Z",
                "updated_at": "2026-09-04T00:00:00Z",
                "document_count": 5,
                "shared": false
            }
        ]
    }"#;
    let resp = parse_workspace_list(raw.as_bytes()).expect("parse");
    assert_eq!(resp.workspaces.len(), 1);
    assert_eq!(resp.workspaces[0].name, "材料研发");
    assert_eq!(resp.workspaces[0].document_count, 5);
}

#[tokio::test]
async fn native_workspace_methods_are_unavailable() {
    let client = BrowserRestClient::new("http://x", Some("tok".to_string()));
    assert!(matches!(
        client.list_workspaces().await.unwrap_err(),
        TransportError::Unavailable(_)
    ));
    assert!(matches!(
        client.create_workspace("a", "b").await.unwrap_err(),
        TransportError::Unavailable(_)
    ));
    assert!(matches!(
        client.list_workspace_documents("ws-1").await.unwrap_err(),
        TransportError::Unavailable(_)
    ));
    assert!(matches!(
        client.list_workspace_notes("ws-1").await.unwrap_err(),
        TransportError::Unavailable(_)
    ));
}
