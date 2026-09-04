use web_sdk::{
    AUTH_PERSISTED_COOKIE_NAME, AUTH_SESSION_COOKIE_NAME, AUTH_STORAGE_KEY, AuthUser, PersistedAuth,
    auth_me_url, has_auth_session_hint, parse_auth_me_json, parse_persisted_auth_cookie,
    parse_persisted_auth_json, persisted_clear_cookie, persisted_set_cookie,
    session_hint_clear_cookie, session_hint_set_cookie,
};

fn sample_user() -> AuthUser {
    AuthUser {
        id: "user-1".to_string(),
        email: "user@example.com".to_string(),
        full_name: "User Example".to_string(),
        bio: None,
        contact_url: None,
        avatar_url: None,
        banner_url: None,
        public_profile_enabled: None,
    }
}

fn sample_auth() -> PersistedAuth {
    PersistedAuth {
        token: "token-1".to_string(),
        user: sample_user(),
    }
}

#[test]
fn storage_and_cookie_names_match_next() {
    assert_eq!(AUTH_STORAGE_KEY, "avrag.auth.v1");
    assert_eq!(AUTH_SESSION_COOKIE_NAME, "avrag.auth.session");
    assert_eq!(AUTH_PERSISTED_COOKIE_NAME, "avrag.auth.persisted");
}

#[test]
fn persisted_json_roundtrip_matches_next_shape() {
    let raw = serde_json::to_string(&sample_auth()).unwrap();
    assert_eq!(
        raw,
        r#"{"token":"token-1","user":{"id":"user-1","email":"user@example.com","full_name":"User Example"}}"#
    );
    assert_eq!(parse_persisted_auth_json(&raw), Some(sample_auth()));
    assert_eq!(parse_persisted_auth_json("{\"token\":\"\"}"), None);
}

#[test]
fn session_hint_cookie_is_observable_and_lax() {
    let set = session_hint_set_cookie(false);
    assert!(set.starts_with("avrag.auth.session=1; Path=/; SameSite=Lax; Max-Age="));
    assert!(!set.contains("Secure"));
    assert!(has_auth_session_hint("theme=dark; avrag.auth.session=1"));

    let https = session_hint_set_cookie(true);
    assert!(https.contains("SameSite=Lax; Secure; Max-Age="));

    let clear = session_hint_clear_cookie(false);
    assert!(clear.contains("Max-Age=0"));
}

#[test]
fn persisted_cookie_roundtrip_uses_uri_encoding() {
    let set = persisted_set_cookie(&sample_auth(), false);
    assert!(set.starts_with("avrag.auth.persisted="));
    assert!(set.contains("SameSite=Lax"));
    let header = set.split("; ").next().unwrap();
    assert_eq!(parse_persisted_auth_cookie(header), Some(sample_auth()));
    assert!(persisted_clear_cookie(false).contains("Max-Age=0"));
}

#[test]
fn auth_me_url_and_envelope() {
    assert_eq!(auth_me_url("http://127.0.0.1:18081"), "http://127.0.0.1:18081/api/auth/me");
    assert_eq!(auth_me_url(""), "/api/auth/me");
    let body = br#"{"success":true,"data":{"token":"","user":{"id":"user-1","email":"user@example.com","full_name":"User Example"},"reset_ticket":null},"error":null}"#;
    assert_eq!(parse_auth_me_json(body).unwrap().id, "user-1");
    assert!(parse_auth_me_json(br#"{"success":false,"data":null,"error":"invalid"}"#).is_none());
}
