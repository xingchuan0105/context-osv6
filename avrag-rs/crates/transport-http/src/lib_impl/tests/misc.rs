use super::support::*;

#[tokio::test]
async fn admin_billing_route_allows_real_admin_with_rls_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("admin-billing-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Admin Billing User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let _register_token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    state
        .grant_e2e_admin_role(&email)
        .await
        .expect("admin role grant should succeed");

    let login_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "email": email,
                "password": "password123"
            })
            .to_string(),
        ))
        .unwrap();
    let login_resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_body = to_bytes(login_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_payload: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    let token = login_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let admin_req = Request::builder()
        .uri("/api/v1/admin/billing")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let admin_resp = app.clone().oneshot(admin_req).await.unwrap();
    assert_eq!(admin_resp.status(), StatusCode::OK);
    let admin_body = to_bytes(admin_resp.into_body(), usize::MAX).await.unwrap();
    let admin_payload: serde_json::Value = serde_json::from_slice(&admin_body).unwrap();
    assert_eq!(admin_payload["error"], serde_json::Value::Null);
    assert!(admin_payload["data"]["active_subscriptions"].is_number());
}

