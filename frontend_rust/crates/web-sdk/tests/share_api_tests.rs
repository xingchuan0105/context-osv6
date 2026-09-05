use web_sdk::{
    BrowserRestClient, TransportError, parse_share_token, parse_shared_workspace,
    public_user_shares_url, share_access_logs_url, share_analytics_url, share_settings_url,
    share_url, shared_kb_url,
};

#[test]
fn share_urls_are_canonical() {
    assert_eq!(
        share_url("http://x/", "ws-1"),
        "http://x/api/v1/workspaces/ws-1/share"
    );
    assert_eq!(
        share_settings_url("", "ws-1"),
        "/api/v1/workspaces/ws-1/share/settings"
    );
    assert_eq!(
        share_analytics_url("http://x", "ws-1"),
        "http://x/api/v1/workspaces/ws-1/share/analytics"
    );
    assert_eq!(
        share_access_logs_url("http://x", "ws-1"),
        "http://x/api/v1/workspaces/ws-1/share/access-logs"
    );
    assert_eq!(
        shared_kb_url("http://x", "tok-123"),
        "http://x/api/shared/kb/tok-123"
    );
    assert_eq!(
        public_user_shares_url("http://x", "u-123"),
        "http://x/api/public/users/u-123/shares"
    );
}

#[test]
fn parse_share_payloads() {
    let tok_raw = br#"{"share_token":"token-abc"}"#;
    let tok_resp = parse_share_token(tok_raw).expect("token");
    assert_eq!(tok_resp.share_token, "token-abc");

    let kb_raw = r#"{
        "knowledge_base": {
            "id": "ws-1",
            "title": "公开知识库",
            "description": "公开资料"
        },
        "share": {
            "permission": "read_only",
            "allow_download": true,
            "scope": "full"
        },
        "sources": [
            {
                "id": "s-1",
                "file_name": "manual.pdf",
                "status": "completed"
            }
        ],
        "owner": {
            "display_name": "张工",
            "profile_enabled": true
        }
    }"#;
    let kb_resp = parse_shared_workspace(kb_raw.as_bytes()).expect("kb");
    assert_eq!(kb_resp.knowledge_base.title, "公开知识库");
    assert_eq!(kb_resp.sources.len(), 1);
    assert!(kb_resp.owner.unwrap().profile_enabled);
}

#[tokio::test]
async fn native_share_methods_are_unavailable() {
    let client = BrowserRestClient::new("http://x", Some("tok".to_string()));
    assert!(matches!(
        client.create_share("ws-1").await.unwrap_err(),
        TransportError::Unavailable(_)
    ));
    assert!(matches!(
        client.get_shared_workspace("tok").await.unwrap_err(),
        TransportError::Unavailable(_)
    ));
}
