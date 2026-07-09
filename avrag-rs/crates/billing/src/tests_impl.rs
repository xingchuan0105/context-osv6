use crate::core::build_plan_payloads;
use crate::stripe_client::StripeClient;
use crate::types::{BillingConfig, HmacSha256, PLAN_PRO, STATUS_ACTIVE};
use crate::webhook_parse::subscription_snapshot_from_event;
use hmac::Mac;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn billing_config_marks_checkout_available_for_creem_and_alipay() {
    let _guard = ENV_MUTEX.lock().unwrap();
    remove_env("STRIPE_SECRET_KEY");
    set_env("CREEM_API_KEY", "creem_test");
    set_env("CREEM_PRODUCT_PRO", "prod_pro");
    set_env("CREEM_PRODUCT_PLUS", "prod_plus");
    set_env("ALIPAY_APP_ID", "alipay_test");
    set_env("ALIPAY_PRICE_PRO", "39.00");
    set_env("ALIPAY_PRICE_PLUS", "19.00");
    set_env("CREEM_PRICE_PRO", "5.99");
    set_env("CREEM_PRICE_PLUS", "3.19");

    let config = BillingConfig::from_env();

    assert!(config.checkout_available(PLAN_PRO));
    assert_eq!(config.price_label_cny_for_plan("plus"), "¥19.00 / 月");
    assert_eq!(config.price_label_usd_for_plan("plus"), "$3.19 / 月");
    assert_eq!(BillingConfig::decimal_price_to_cents("19.00"), 1900);
}

#[test]
fn billing_config_falls_back_to_legacy_price_envs() {
    let _guard = ENV_MUTEX.lock().unwrap();
    remove_env("STRIPE_PRICE_PRO");
    remove_env("STRIPE_PRICE_ENTERPRISE");
    remove_env("BILLING_PRICE_LABEL_PRO");
    remove_env("BILLING_PRICE_LABEL_PLUS");
    remove_env("ALIPAY_PRICE_PRO");
    remove_env("ALIPAY_PRICE_PLUS");
    remove_env("CREEM_PRICE_PRO");
    remove_env("CREEM_PRICE_PLUS");
    set_env("STRIPE_SECRET_KEY", "sk_test");
    set_env("STRIPE_PRICE_PRO_MONTHLY", "price_pro_legacy");

    let config = BillingConfig::from_env();

    assert_eq!(config.stripe_price_pro, "price_pro_legacy");
    assert_eq!(
        config.price_label_for_plan(PLAN_PRO),
        "¥39.00 / 月 · $5.99 / 月"
    );
}

#[test]
fn subscription_snapshot_uses_metadata_and_price_mapping() {
    let config = BillingConfig {
        stripe_secret_key: "sk".to_string(),
        stripe_webhook_secret: "whsec".to_string(),
        stripe_price_pro: "price_pro".to_string(),
        stripe_price_plus: "price_plus".to_string(),
        billing_price_label_pro: "$20/month".to_string(),
        billing_price_label_plus: "Contact sales".to_string(),
        public_app_base_url: "http://localhost:3000".to_string(),
        ..Default::default()
    };

    let payload = serde_json::json!({
        "id": "evt_1",
        "type": "customer.subscription.updated",
        "data": {
            "object": {
                "id": "sub_123",
                "customer": {"id": "cus_123"},
                "metadata": {
                    "user_id": "11111111-1111-1111-1111-111111111111"
                },
                "status": "active",
                "current_period_start": 1_700_000_000_i64,
                "current_period_end": 1_700_086_400_i64,
                "cancel_at_period_end": false,
                "items": {
                    "data": [
                        {
                            "price": {"id": "price_pro"}
                        }
                    ]
                }
            }
        }
    });

    let snapshot = subscription_snapshot_from_event(&payload, &config).unwrap();

    assert_eq!(snapshot.user_id, "11111111-1111-1111-1111-111111111111");
    assert_eq!(snapshot.stripe_customer_id, "cus_123");
    assert_eq!(snapshot.stripe_subscription_id, "sub_123");
    assert_eq!(snapshot.plan_id, PLAN_PRO);
    assert_eq!(snapshot.status, STATUS_ACTIVE);
    assert!(snapshot.current_period_start.is_some());
    assert!(snapshot.current_period_end.is_some());
}

#[test]
fn webhook_signature_verification_accepts_valid_signature() {
    let config = BillingConfig {
        stripe_secret_key: "sk".to_string(),
        stripe_webhook_secret: "whsec_test_secret".to_string(),
        stripe_price_pro: String::new(),
        stripe_price_plus: String::new(),
        billing_price_label_pro: "$20/month".to_string(),
        billing_price_label_plus: "Contact sales".to_string(),
        public_app_base_url: "http://localhost:3000".to_string(),
        ..Default::default()
    };
    let client = StripeClient::new(config);
    let payload = br#"{"id":"evt_123"}"#;
    let timestamp = "1700000000";
    let signed_payload = format!("{timestamp}.{}", String::from_utf8_lossy(payload));
    let mut mac = HmacSha256::new_from_slice(b"whsec_test_secret").unwrap();
    mac.update(signed_payload.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    client
        .verify_webhook_signature(payload, &format!("t={timestamp},v1={signature}"))
        .unwrap();
}

fn set_env(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}

fn remove_env(key: &str) {
    unsafe {
        std::env::remove_var(key);
    }
}

#[test]
fn alipay_client_signature_verify_works() {
    use crate::AlipayClient;
    use rand::thread_rng;
    use rsa::RsaPrivateKey;
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};

    let mut rng = thread_rng();
    let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let public_key = private_key.to_public_key();

    let private_key_pem = private_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap()
        .to_string();
    let public_key_pem = public_key
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap();

    let config = BillingConfig {
        alipay_app_id: "test_app_id".to_string(),
        alipay_private_key: private_key_pem,
        alipay_public_key: public_key_pem,
        alipay_gateway_url: "".to_string(),
        alipay_notify_url: None,
        alipay_price_pro: "20.00".to_string(),
        alipay_price_plus: "100.00".to_string(),
        ..Default::default()
    };

    let client = AlipayClient::new(config);
    let params = vec![
        ("app_id".to_string(), "test_app_id".to_string()),
        ("method".to_string(), "alipay.trade.precreate".to_string()),
        ("biz_content".to_string(), "{}".to_string()),
    ];

    let sign = client.sign(&params).unwrap();
    assert!(!sign.is_empty());

    let mut verify_params = params.clone();
    verify_params.push(("sign".to_string(), sign.clone()));

    assert!(client.verify_signature(&verify_params, &sign).is_ok());

    // 篡改数据以确认验证失败
    verify_params[0].1 = "modified_app_id".to_string();
    assert!(client.verify_signature(&verify_params, &sign).is_err());
}

#[test]
fn alipay_real_key_loads_and_signs() {
    use crate::AlipayClient;

    let _guard = ENV_MUTEX.lock().unwrap();

    if std::env::var("ALIPAY_APP_ID")
        .unwrap_or_default()
        .is_empty()
    {
        return;
    }

    let config = BillingConfig::from_env();
    if config.alipay_app_id.is_empty() {
        return;
    }

    let client = AlipayClient::new(config);
    let params = vec![
        ("app_id".to_string(), "test_app_id".to_string()),
        ("method".to_string(), "alipay.trade.precreate".to_string()),
        ("biz_content".to_string(), "{}".to_string()),
    ];

    let sign = client.sign(&params).unwrap();
    assert!(!sign.is_empty());

    // Also verify the signature with the configured public key
    let mut verify_params = params.clone();
    verify_params.push(("sign".to_string(), sign.clone()));
    assert!(client.verify_signature(&verify_params, &sign).is_ok());
}

// =====================================================================
// Task 3: dual-currency price labels on the /plans endpoint payload.
// =====================================================================

#[test]
fn plans_endpoint_emits_dual_currency_price_labels_for_plus_and_pro() {
    let _guard = ENV_MUTEX.lock().unwrap();
    set_env("ALIPAY_PRICE_PLUS", "19.00");
    set_env("ALIPAY_PRICE_PRO", "39.00");
    set_env("CREEM_PRICE_PLUS", "3.19");
    set_env("CREEM_PRICE_PRO", "5.99");
    let config = BillingConfig::from_env();

    let plans = build_plan_payloads(&config, "free", &Default::default());

    assert_eq!(
        plans.len(),
        3,
        "expected 3 tiers (free/pro/plus), got {}",
        plans.len()
    );

    let plus = plans
        .iter()
        .find(|p| p.get("plan_id").and_then(|v| v.as_str()) == Some("plus"))
        .expect("plus plan present");
    assert_eq!(
        plus.get("price_label_cny").and_then(|v| v.as_str()),
        Some("¥19.00 / 月"),
        "plus plan should carry the CNY price label from ALIPAY_PRICE_PLUS"
    );
    assert_eq!(
        plus.get("price_label_usd").and_then(|v| v.as_str()),
        Some("$3.19 / 月"),
        "plus plan should carry the USD price label from CREEM_PRICE_PLUS"
    );

    let pro = plans
        .iter()
        .find(|p| p.get("plan_id").and_then(|v| v.as_str()) == Some("pro"))
        .expect("pro plan present");
    assert_eq!(
        pro.get("price_label_cny").and_then(|v| v.as_str()),
        Some("¥39.00 / 月"),
    );
    assert_eq!(
        pro.get("price_label_usd").and_then(|v| v.as_str()),
        Some("$5.99 / 月"),
    );
}

#[test]
fn plans_endpoint_marks_current_user_plan() {
    let _guard = ENV_MUTEX.lock().unwrap();
    remove_env("BILLING_PRICE_LABEL_PRO");
    remove_env("BILLING_PRICE_LABEL_PLUS");
    let config = BillingConfig::from_env();

    let plans = build_plan_payloads(&config, "plus", &Default::default());

    let plus_current = plans
        .iter()
        .find(|p| p.get("plan_id").and_then(|v| v.as_str()) == Some("plus"))
        .and_then(|p| p.get("current"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        plus_current,
        "plus plan should be marked current when current_plan_id == \"plus\""
    );

    let free_current = plans
        .iter()
        .find(|p| p.get("plan_id").and_then(|v| v.as_str()) == Some("free"))
        .and_then(|p| p.get("current"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    assert!(
        !free_current,
        "free plan must not be current when user is on plus"
    );
}
