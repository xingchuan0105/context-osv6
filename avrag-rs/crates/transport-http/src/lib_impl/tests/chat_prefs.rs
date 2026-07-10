use super::support::*;

#[tokio::test]
async fn agent_preferences_api_can_get_put_and_delete_preferences() {
    let state = test_app_state();
    let app = build_router(state);
    let owner_user_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let put_req = Request::builder()
        .uri("/api/auth/agent-preferences")
        .method("PUT")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, owner_user_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"active":[{"id":"pref-1","text":"Use concise answers","category":"interaction","scope":"global","confidence":"explicit","source":"test","updated_at":"2026-04-26T00:00:00Z"}],"superseded":[],"blocked":[],"daily_log":[],"last_consolidated_at":null}"#,
        ))
        .unwrap();
    let put_resp = app.clone().oneshot(put_req).await.unwrap();
    assert_eq!(put_resp.status(), StatusCode::OK);

    let get_req = Request::builder()
        .uri("/api/auth/agent-preferences")
        .method("GET")
        .header(middleware::HEADER_OWNER_USER_ID, owner_user_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body = to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
    let get_payload: serde_json::Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(get_payload["active"][0]["id"], "pref-1");

    let delete_req = Request::builder()
        .uri("/api/auth/agent-preferences/pref-1")
        .method("DELETE")
        .header(middleware::HEADER_OWNER_USER_ID, owner_user_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let delete_resp = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);
    let delete_body = to_bytes(delete_resp.into_body(), usize::MAX).await.unwrap();
    let delete_payload: serde_json::Value = serde_json::from_slice(&delete_body).unwrap();
    assert!(delete_payload["active"].as_array().unwrap().is_empty());
    assert_eq!(delete_payload["blocked"][0]["id"], "pref-1");
}


#[tokio::test]
async fn profile_update_roundtrip_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("profile-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Initial Name")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = payload["data"]["token"].as_str().unwrap().to_string();

    let update_req = Request::builder()
        .uri("/api/auth/profile")
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"full_name":"Updated Name"}"#))
        .unwrap();
    let update_resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(update_resp.status(), StatusCode::OK);

    let me_req = Request::builder()
        .uri("/api/auth/me")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let me_resp = app.oneshot(me_req).await.unwrap();
    assert_eq!(me_resp.status(), StatusCode::OK);
    let me_body = to_bytes(me_resp.into_body(), usize::MAX).await.unwrap();
    let me_payload: serde_json::Value = serde_json::from_slice(&me_body).unwrap();
    assert_eq!(
        me_payload["data"]["user"]["full_name"].as_str(),
        Some("Updated Name")
    );
}


#[tokio::test]
async fn preferences_roundtrip_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("prefs-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Prefs User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let update_req = Request::builder()
        .uri("/api/auth/preferences")
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"dashboard":{"favorite_workspace_ids":[],"workspace_drafts":[{"workspace_id":"00000000-0000-0000-0000-000000000010","notes":"hello notes"}]},"notifications":{"email_enabled":true,"product_enabled":true,"security_enabled":true,"weekly_digest_enabled":false,"quiet_hours_start":null,"quiet_hours_end":null}}"#,
        ))
        .unwrap();
    let update_resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(update_resp.status(), StatusCode::OK);

    let get_req = Request::builder()
        .uri("/api/auth/preferences")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let get_resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body = to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(
        payload["dashboard"]["workspace_drafts"][0]["notes"].as_str(),
        Some("hello notes")
    );
}

