//! Billing checkout consent gate (HTTP black-box).

use crate::product_e2e::TestContext;

#[tokio::test]
async fn checkout_without_payment_consent_returns_consent_required() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let email = format!("billing-smoke-{}@example.test", uuid::Uuid::new_v4());
    let token = ctx
        .register_user_token(&email, "Billing Smoke User")
        .await
        .expect("register user");

    let resp = ctx
        .create_checkout_session_with_token(&token, "plus")
        .await
        .expect("checkout response");

    assert_eq!(
        resp.status, 200,
        "billing API returns envelope HTTP 200, body carries error code"
    );
    assert_eq!(
        resp.body_json.get("ok").and_then(|v| v.as_bool()),
        Some(false),
        "checkout without consent should return ok=false, body={}",
        resp.body_json
    );
    assert_eq!(
        resp.body_json
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|v| v.as_str()),
        Some("consent_required"),
        "checkout without payment legal acceptance must return consent_required, body={}",
        resp.body_json
    );
}
