use web_sdk::{
    BrowserRestClient, ProviderSecretRow, TransportError, has_quick_chat_byok, model_role_label,
    parse_provider_secrets, provider_secrets_url,
};

#[test]
fn provider_secrets_url_trims_slash() {
    assert_eq!(
        provider_secrets_url("http://127.0.0.1:18081/"),
        "http://127.0.0.1:18081/api/v1/settings/provider-secrets"
    );
    assert_eq!(
        provider_secrets_url(""),
        "/api/v1/settings/provider-secrets"
    );
}

#[test]
fn parse_provider_secrets_from_wire() {
    let raw = br#"{
        "secrets": [
            {
                "id": "sec-1",
                "purpose": "quick_chat",
                "provider": "bailian",
                "model_hint": "qwen3.8-flash",
                "is_active": true
            },
            {
                "id": "sec-2",
                "purpose": "llm",
                "provider": "deepseek",
                "model_hint": "deepseek-v4-flash"
            }
        ]
    }"#;
    let resp = parse_provider_secrets(raw).expect("parse");
    assert_eq!(resp.secrets.len(), 2);
    assert!(has_quick_chat_byok(&resp.secrets));
}

#[test]
fn has_quick_chat_byok_checks_revoked_and_active() {
    let active = vec![ProviderSecretRow {
        id: "1".into(),
        purpose: "quick_chat".into(),
        provider: "bailian".into(),
        model_hint: None,
        is_active: Some(true),
        revoked_at: None,
    }];
    assert!(has_quick_chat_byok(&active));

    let revoked = vec![ProviderSecretRow {
        id: "2".into(),
        purpose: "quick_chat".into(),
        provider: "bailian".into(),
        model_hint: None,
        is_active: Some(true),
        revoked_at: Some("2026-09-01T00:00:00Z".into()),
    }];
    assert!(!has_quick_chat_byok(&revoked));

    let inactive = vec![ProviderSecretRow {
        id: "3".into(),
        purpose: "quick_chat".into(),
        provider: "bailian".into(),
        model_hint: None,
        is_active: Some(false),
        revoked_at: None,
    }];
    assert!(!has_quick_chat_byok(&inactive));

    let other_purpose = vec![ProviderSecretRow {
        id: "4".into(),
        purpose: "llm".into(),
        provider: "deepseek".into(),
        model_hint: None,
        is_active: Some(true),
        revoked_at: None,
    }];
    assert!(!has_quick_chat_byok(&other_purpose));
}

#[test]
fn model_role_label_covers_quick_chat_and_agent() {
    assert_eq!(
        model_role_label("quick_chat", false),
        "对话 · qwen3.8-flash (默认)"
    );
    assert_eq!(
        model_role_label("quick_chat", true),
        "对话 · qwen3.8-flash (自定义密钥)"
    );
    assert_eq!(model_role_label("agent", false), "工作区 Agent");
}

#[tokio::test]
async fn native_list_provider_secrets_is_unavailable() {
    let client = BrowserRestClient::new("http://127.0.0.1:18081", Some("token".into()));
    let err = client.list_provider_secrets().await.expect_err("unavailable");
    assert!(matches!(err, TransportError::Unavailable(_)));
}
