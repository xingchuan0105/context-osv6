use crate::core::subscription_snapshot_from_event;
use crate::stripe_client::StripeClient;
use crate::types::{BillingConfig, HmacSha256, PLAN_PRO, STATUS_ACTIVE};
use hmac::Mac;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn billing_config_prefers_v5_env_names() {
    let _guard = ENV_MUTEX.lock().unwrap();
    set_env("STRIPE_PRICE_PRO", "price_pro_v5");
    set_env("STRIPE_PRICE_ENTERPRISE", "price_plus_v5");
    set_env("BILLING_PRICE_LABEL_PRO", "$29/month");
    set_env("BILLING_PRICE_LABEL_ENTERPRISE", "Talk to sales");
    set_env("STRIPE_SECRET_KEY", "sk_test");
    remove_env("STRIPE_PRICE_PRO_MONTHLY");
    remove_env("STRIPE_PRICE_ID");

    let config = BillingConfig::from_env();

    assert_eq!(config.stripe_price_pro, "price_pro_v5");
    assert_eq!(config.stripe_price_plus, "price_plus_v5");
    assert_eq!(config.billing_price_label_pro, "$29/month");
    assert_eq!(config.billing_price_label_plus, "Talk to sales");
    assert!(config.checkout_available(PLAN_PRO));
}

#[test]
fn billing_config_falls_back_to_legacy_price_envs() {
    let _guard = ENV_MUTEX.lock().unwrap();
    remove_env("STRIPE_PRICE_PRO");
    remove_env("STRIPE_PRICE_ENTERPRISE");
    remove_env("BILLING_PRICE_LABEL_PRO");
    remove_env("BILLING_PRICE_LABEL_ENTERPRISE");
    set_env("STRIPE_SECRET_KEY", "sk_test");
    set_env("STRIPE_PRICE_PRO_MONTHLY", "price_pro_legacy");

    let config = BillingConfig::from_env();

    assert_eq!(config.stripe_price_pro, "price_pro_legacy");
    assert_eq!(
        config.price_label_for_plan(PLAN_PRO),
        "¥129 / 月 · $19 / 月"
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
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
    use rsa::RsaPrivateKey;
    use rand::thread_rng;

    let mut rng = thread_rng();
    let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let public_key = private_key.to_public_key();

    let private_key_pem = private_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap().to_string();
    let public_key_pem = public_key.to_public_key_pem(rsa::pkcs8::LineEnding::LF).unwrap();

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

    if std::env::var("ALIPAY_APP_ID").unwrap_or_default().is_empty() {
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
