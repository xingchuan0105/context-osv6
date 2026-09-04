use contracts::workspaces::ConversationScopeKind;
use web_sdk::{
    BrowserRestClient, TransportError, parse_message_list, parse_session, parse_session_list,
    session_messages_url, session_url, sessions_url,
};

#[test]
fn session_urls_trim_slash_and_encode_id() {
    assert_eq!(
        sessions_url("http://127.0.0.1:18081/"),
        "http://127.0.0.1:18081/api/v1/chat/sessions"
    );
    assert_eq!(
        session_url("", "sess-900"),
        "/api/v1/chat/sessions/sess-900"
    );
    assert_eq!(
        session_messages_url("http://x", "a/b"),
        "http://x/api/v1/chat/sessions/a%2Fb/messages"
    );
}

#[test]
fn parse_session_list_from_wire_json() {
    let body = r#"{
        "sessions": [
            {
                "id": "11111111-1111-1111-1111-111111111111",
                "owner_user_id": "user-1",
                "workspace_id": "ws-1",
                "scope_kind": "workspace",
                "workspace_name": "材料",
                "title": "上周讨论",
                "agent_type": "chat",
                "model_role": "quick_chat",
                "pinned": false,
                "created_at": "2026-09-01T00:00:00Z",
                "updated_at": "2026-09-04T00:00:00Z"
            }
        ]
    }"#
    .as_bytes();
    let list = parse_session_list(body).expect("session list");
    assert_eq!(list.sessions.len(), 1);
    let session = &list.sessions[0];
    assert_eq!(session.id, "11111111-1111-1111-1111-111111111111");
    assert_eq!(session.workspace_id.as_deref(), Some("ws-1"));
    assert_eq!(session.scope_kind, ConversationScopeKind::Workspace);
    assert_eq!(session.title.as_deref(), Some("上周讨论"));
}

#[test]
fn parse_session_and_messages_from_wire_json() {
    let session = parse_session(
        r#"{
            "id": "sess-1",
            "owner_user_id": "user-1",
            "scope_kind": "personal",
            "agent_type": "chat",
            "model_role": "quick_chat",
            "created_at": "2026-09-01T00:00:00Z",
            "updated_at": "2026-09-04T00:00:00Z"
        }"#
        .as_bytes(),
    )
    .expect("session");
    assert_eq!(session.id, "sess-1");
    assert_eq!(session.scope_kind, ConversationScopeKind::Personal);
    assert!(session.workspace_id.is_none());

    let list = parse_message_list(
        r#"{
            "messages": [
                {
                    "id": 11,
                    "session_id": "sess-1",
                    "role": "user",
                    "content": "你好",
                    "created_at": "2026-09-04T00:00:00Z"
                },
                {
                    "id": 12,
                    "session_id": "sess-1",
                    "role": "assistant",
                    "content": "你好，我是助手。",
                    "answer_blocks": [{"type": "text", "text": "你好，我是助手。", "citations": []}],
                    "citations": [
                        {
                            "citation_id": 1,
                            "doc_id": "doc-1",
                            "doc_name": "手册",
                            "score": 0.9
                        }
                    ],
                    "created_at": "2026-09-04T00:00:01Z"
                }
            ]
        }"#
        .as_bytes(),
    )
    .expect("messages");
    assert_eq!(list.messages.len(), 2);
    assert_eq!(list.messages[0].role, "user");
    assert_eq!(list.messages[1].content, "你好，我是助手。");
    assert_eq!(list.messages[1].citations.len(), 1);
    assert_eq!(list.messages[1].citations[0].doc_name, "手册");
}

#[test]
fn parse_session_list_rejects_unknown_envelope() {
    let err = parse_session_list(br#"{"data":[]}"#).expect_err("bad envelope");
    assert!(matches!(err, TransportError::Serialization(_)));
}

#[tokio::test]
async fn native_rest_client_is_unavailable() {
    let client = BrowserRestClient::new("http://127.0.0.1:18081", Some("token".to_string()));
    let err = client.list_sessions().await.expect_err("native unavailable");
    assert!(matches!(err, TransportError::Unavailable(_)));
}

#[test]
fn http_status_mapper_matches_chat_transport() {
    assert!(matches!(
        TransportError::from_http_status(401, String::new()),
        TransportError::Unauthorized
    ));
    assert!(matches!(
        TransportError::from_http_status(429, String::new()),
        TransportError::RateLimited
    ));
}
