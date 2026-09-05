use web_sdk::{
    BrowserRestClient, CheckoutRequest, TransportError, billing_plans_url, checkout_session_url,
    order_status_url, parse_billing_plans, parse_checkout_response, parse_order_status,
    parse_topup_packs, parse_wallet_balance, topup_packs_url, wallet_balance_url,
};

#[test]
fn billing_urls_are_canonical() {
    assert_eq!(
        billing_plans_url("http://x/"),
        "http://x/api/v1/billing/plans"
    );
    assert_eq!(
        wallet_balance_url(""),
        "/api/v1/billing/wallet"
    );
    assert_eq!(
        topup_packs_url("http://x"),
        "http://x/api/v1/billing/wallet/topup-packs"
    );
    assert_eq!(
        checkout_session_url("http://x"),
        "http://x/api/v1/billing/checkout-session"
    );
    assert_eq!(
        order_status_url("http://x", "ord-123"),
        "http://x/api/v1/billing/orders/ord-123"
    );
}

#[test]
fn parse_billing_payloads() {
    let plans_raw = r#"{
        "plans": [
            {
                "plan_id": "free",
                "name": "免费体验版",
                "description": "基础功能体验",
                "price_label_cny": "¥0",
                "interval": "month",
                "current": true
            },
            {
                "plan_id": "pro",
                "name": "Pro 专业版",
                "description": "高级 Agent 与大容量资料库",
                "price_label_cny": "¥99/月",
                "interval": "month",
                "current": false
            }
        ],
        "current_plan_id": "free"
    }"#;
    let plans = parse_billing_plans(plans_raw.as_bytes()).expect("plans");
    assert_eq!(plans.plans.len(), 2);
    assert_eq!(plans.current_plan_id, "free");

    let wallet_raw = r#"{"user_id":"u-1","balance_fen":2000,"lifetime_paid_topup_fen":5000}"#;
    let wallet = parse_wallet_balance(wallet_raw.as_bytes()).expect("wallet");
    assert_eq!(wallet.balance_fen, 2000);

    let packs_raw = r#"[{"pack_id":"topup_50","amount_fen":5000,"amount_yuan":50,"label_cny":"50元"}]"#;
    let packs = parse_topup_packs(packs_raw.as_bytes()).expect("packs");
    assert_eq!(packs.len(), 1);

    let checkout_raw = r#"{"url":"https://checkout.example.com/pay","session_id":"cs_123","order_id":"ord_123"}"#;
    let co = parse_checkout_response(checkout_raw.as_bytes()).expect("checkout");
    assert_eq!(co.session_id, "cs_123");

    let order_raw = r#"{"order_id":"ord_123","status":"paid","plan_id":"pro"}"#;
    let ord = parse_order_status(order_raw.as_bytes()).expect("order");
    assert_eq!(ord.status, "paid");
}

#[tokio::test]
async fn native_billing_methods_are_unavailable() {
    let client = BrowserRestClient::new("http://x", Some("tok".to_string()));
    assert!(matches!(
        client.get_billing_plans().await.unwrap_err(),
        TransportError::Unavailable(_)
    ));
    assert!(matches!(
        client
            .create_checkout_session(&CheckoutRequest {
                plan_id: Some("pro".to_string()),
                provider: Some("alipay".to_string()),
                kind: None,
                topup_pack_id: None
            })
            .await
            .unwrap_err(),
        TransportError::Unavailable(_)
    ));
}
