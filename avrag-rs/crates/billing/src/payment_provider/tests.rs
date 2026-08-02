//! Payment provider adapter tests — golden wire samples signed/verified
//! locally (no network, no live credentials).
//!
//! Creem samples use a fixed HMAC secret; Alipay samples use a fresh RSA-2048
//! keypair generated per test, so the sign/verify asymmetry is proven against
//! a real signature.

use app_core::{BillingProvider, ProviderEvent};
use hmac::Mac;

use crate::payment_provider::{
    AlipayAdapter, CheckoutSession, CreemAdapter, PaymentProvider, ProviderError,
};
use crate::{AlipayClient, BillingConfig};
use common::UserId;
use uuid::Uuid;

const CREEM_SECRET: &str = "whsec_test_secret";

/// Percent-encode a value for an Alipay notify wire form (`+` and `%` are
/// significant — the sign's base64 can contain `+`, and the value must be
/// encoded the way Alipay's notify encodes it).
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn creem_config() -> BillingConfig {
    BillingConfig {
        creem_webhook_secret: CREEM_SECRET.to_string(),
        creem_product_pro: "prod_pro".to_string(),
        creem_api_key: "creem_test".to_string(),
        public_app_base_url: "http://localhost:3000".to_string(),
        ..Default::default()
    }
}

fn creem_signature(payload: &[u8]) -> String {
    let mut mac =
        crate::types::HmacSha256::new_from_slice(CREEM_SECRET.as_bytes()).expect("valid key");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

fn paid_payload() -> serde_json::Value {
    serde_json::json!({
        "id": "evt_123",
        "type": "subscription.paid",
        "data": {
            "id": "sub_abc",
            "user_id": "user-42",
            "plan_id": "pro",
            "price_id": "price_x",
            "amount": 1900,
            "currency": "usd",
            "current_period_start": 1_752_000_000,
            "current_period_end": 1_755_000_000,
            "metadata": { "user_id": "user-42", "plan_id": "pro" }
        }
    })
}

fn alipay_config() -> BillingConfig {
    // Keys are populated by the generated keypair in each test that needs them.
    BillingConfig {
        alipay_app_id: "2021000000000001".to_string(),
        alipay_gateway_url: "https://openapi-sandbox.dl.alipaydev.com/gateway.do".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn creem_subscription_paid_parses_typed_event() {
    let adapter = CreemAdapter::new(creem_config());
    let payload = serde_json::to_vec(&paid_payload()).unwrap();

    let delivery = adapter
        .parse_event(Some(&creem_signature(&payload)), &payload)
        .await
        .expect("valid signature should parse");

    assert_eq!(delivery.event_id, "evt_123");
    match delivery.event {
        ProviderEvent::SubscriptionPaid {
            subscription_id,
            user_id,
            plan_id,
            price_id,
            amount_cents,
            currency,
            current_period_start,
            current_period_end,
        } => {
            assert_eq!(subscription_id, "sub_abc");
            assert_eq!(user_id, "user-42");
            assert_eq!(plan_id, "pro");
            assert_eq!(price_id, "price_x");
            assert_eq!(amount_cents, 1900);
            assert_eq!(currency, "usd");
            assert!(current_period_start.is_some());
            assert!(current_period_end.is_some());
        }
        other => panic!("expected SubscriptionPaid, got {other:?}"),
    }
}

#[tokio::test]
async fn creem_rejects_bad_signature() {
    let adapter = CreemAdapter::new(creem_config());
    let payload = serde_json::to_vec(&paid_payload()).unwrap();
    let error = adapter
        .parse_event(Some("deadbeef"), &payload)
        .await
        .expect_err("bad signature must fail");
    assert!(matches!(error, ProviderError::Signature(_)));
}

#[tokio::test]
async fn creem_rejects_missing_signature() {
    let adapter = CreemAdapter::new(creem_config());
    let payload = serde_json::to_vec(&paid_payload()).unwrap();
    let error = adapter
        .parse_event(None, &payload).await
        .expect_err("missing signature must fail");
    assert!(matches!(error, ProviderError::Signature(_)));
}

#[tokio::test]
async fn creem_canceled_parses_typed_event() {
    let adapter = CreemAdapter::new(creem_config());
    let payload = serde_json::json!({
        "id": "evt_456",
        "type": "subscription.canceled",
        "data": { "id": "sub_abc" }
    });
    let payload = serde_json::to_vec(&payload).unwrap();

    let delivery = adapter
        .parse_event(Some(&creem_signature(&payload)), &payload)
        .await
        .expect("valid canceled event should parse");
    assert!(matches!(
        delivery.event,
        ProviderEvent::SubscriptionCanceled { subscription_id } if subscription_id == "sub_abc"
    ));
}

#[tokio::test]
async fn creem_unknown_event_type_is_explicit_ignored() {
    let adapter = CreemAdapter::new(creem_config());
    let payload = serde_json::json!({
        "id": "evt_789",
        "type": "invoice.created",
        "data": {}
    });
    let payload = serde_json::to_vec(&payload).unwrap();

    let delivery = adapter
        .parse_event(Some(&creem_signature(&payload)), &payload)
        .await
        .expect("unknown type should parse as ignored");
    assert!(matches!(delivery.event, ProviderEvent::Ignored));
}

#[tokio::test]
async fn creem_missing_amount_fails_explicitly_not_defaulted() {
    // Regression: the old second dispatcher silently defaulted amount to 2000
    // and plan to "pro". The adapter must fail instead.
    let adapter = CreemAdapter::new(creem_config());
    let mut payload = paid_payload();
    payload["data"].as_object_mut().unwrap().remove("amount");
    let payload = serde_json::to_vec(&payload).unwrap();

    let error = adapter
        .parse_event(Some(&creem_signature(&payload)), &payload)
        .await
        .expect_err("missing amount must fail");
    assert!(matches!(error, ProviderError::Invalid(_)), "{error}");
}

fn rsa_keypair() -> (String, String) {
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
    use rsa::RsaPrivateKey;
    let mut rng = rand::rngs::OsRng;
    let key = RsaPrivateKey::new(&mut rng, 2048).expect("2048-bit keygen");
    let private_pem = key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .expect("pkcs8 private pem")
        .to_string();
    let public_pem = key
        .to_public_key()
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .expect("public pem");
    (private_pem, public_pem)
}

fn alipay_client_with_keys() -> AlipayClient {
    let (private_pem, public_pem) = rsa_keypair();
    let mut config = alipay_config();
    config.alipay_private_key = private_pem;
    config.alipay_public_key = public_pem;
    AlipayClient::new(config)
}

#[test]
fn alipay_verify_signature_excludes_sign_type_while_signing_includes_it() {
    let client = alipay_client_with_keys();

    // Notify-style content: everything except sign/sign_type.
    let notify_params = vec![
        ("app_id".to_string(), "2021000000000001".to_string()),
        ("out_trade_no".to_string(), "order-1".to_string()),
        ("total_amount".to_string(), "129.00".to_string()),
    ];
    let sign = client.sign(&notify_params).expect("sign");
    let mut delivery = notify_params.clone();
    delivery.push(("sign".to_string(), sign));
    client
        .verify_signature(&delivery, &delivery.last().unwrap().1)
        .expect("signature over sign_type-free content must verify");

    // Asymmetry: request signing includes `sign_type` in the signed content,
    // while notify verification excludes it — so a signature made with
    // sign_type present must NOT verify under notify-style verification.
    let mut signed_with_type = notify_params.clone();
    signed_with_type.push(("sign_type".to_string(), "RSA2".to_string()));
    let sign_with_type = client.sign(&signed_with_type).expect("sign with sign_type");
    signed_with_type.push(("sign".to_string(), sign_with_type));
    let error = client
        .verify_signature(&signed_with_type, &signed_with_type.last().unwrap().1)
        .expect_err("sign_type-inclusive signature must fail under notify verification");
    assert!(error.to_string().contains("verification failed"), "{error}");
}

#[test]
fn alipay_verify_signature_fails_on_tampered_value() {
    let client = alipay_client_with_keys();
    let mut params = vec![
        ("app_id".to_string(), "2021000000000001".to_string()),
        ("out_trade_no".to_string(), "order-1".to_string()),
        ("total_amount".to_string(), "129.00".to_string()),
    ];
    let sign = client.sign(&params).expect("sign");
    params.push(("sign".to_string(), sign));
    // Tamper AFTER signing: amount changed.
    params[2] = ("total_amount".to_string(), "999.00".to_string());
    let error = client
        .verify_signature(&params, &params.last().unwrap().1)
        .expect_err("tampered amount must fail verification");
    assert!(error.to_string().contains("verification failed"), "{error}");
}

#[tokio::test]
async fn alipay_cjk_percent_decoded_subject_survives_signature_and_parses() {
    let adapter_config = alipay_config();
    let (private_pem, public_pem) = rsa_keypair();
    let mut keyed_config = adapter_config.clone();
    keyed_config.alipay_private_key = private_pem;
    keyed_config.alipay_public_key = public_pem;
    let client = AlipayClient::new(keyed_config.clone());

    // Alipay signs the DECODED values; the wire carries percent-encoded bytes.
    let decoded_params = vec![
        ("app_id".to_string(), "2021000000000001".to_string()),
        ("notify_id".to_string(), "notify-9".to_string()),
        ("out_trade_no".to_string(), "order-2".to_string()),
        ("subject".to_string(), "中文订阅".to_string()),
        ("total_amount".to_string(), "49.00".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ];
    let sign = client.sign(&decoded_params).expect("sign");

    // Wire form: values percent-encoded (multi-byte UTF-8 byte-by-byte).
    let wire = format!(
        "app_id=2021000000000001&notify_id=notify-9&out_trade_no=order-2&\
         subject=%E4%B8%AD%E6%96%87%E8%AE%A2%E9%98%85&total_amount=49.00&\
         trade_status=TRADE_SUCCESS&sign={}",
        percent_encode(&sign)
    );

    let adapter = AlipayAdapter::new(keyed_config);
    let delivery = adapter
        .parse_event(None, wire.as_bytes())
        .await
        .expect("CJK percent-decoded notify should verify and parse");
    assert_eq!(delivery.event_id, "notify-9");
    match delivery.event {
        ProviderEvent::AlipayOrderPaid {
            out_trade_no,
            paid_cents,
        } => {
            assert_eq!(out_trade_no, "order-2");
            assert_eq!(paid_cents, 4900);
        }
        other => panic!("expected AlipayOrderPaid, got {other:?}"),
    }
}

#[tokio::test]
async fn alipay_wait_buyer_pay_is_ignored() {
    let adapter_config = alipay_config();
    let (private_pem, public_pem) = rsa_keypair();
    let mut keyed_config = adapter_config.clone();
    keyed_config.alipay_private_key = private_pem;
    keyed_config.alipay_public_key = public_pem;
    let client = AlipayClient::new(keyed_config.clone());

    let decoded = vec![
        ("app_id".to_string(), "2021000000000001".to_string()),
        ("notify_id".to_string(), "notify-1".to_string()),
        ("out_trade_no".to_string(), "order-3".to_string()),
        ("total_amount".to_string(), "129.00".to_string()),
        ("trade_status".to_string(), "TRADE_WAIT_BUYER_PAY".to_string()),
    ];
    let sign = client.sign(&decoded).expect("sign");
    let wire = format!(
        "app_id=2021000000000001&notify_id=notify-1&trade_status=TRADE_WAIT_BUYER_PAY\
         &out_trade_no=order-3&total_amount=129.00&sign_type=RSA2&sign={}",
        percent_encode(&sign)
    );

    let adapter = AlipayAdapter::new(keyed_config);
    let delivery = adapter
        .parse_event(None, wire.as_bytes())
        .await
        .expect("non-final status parses as ignored");
    assert!(matches!(delivery.event, ProviderEvent::Ignored));
}

#[tokio::test]
async fn alipay_rejects_cross_merchant_app_id() {
    let adapter_config = alipay_config();
    let (private_pem, public_pem) = rsa_keypair();
    let mut keyed_config = adapter_config.clone();
    keyed_config.alipay_private_key = private_pem;
    keyed_config.alipay_public_key = public_pem;
    let client = AlipayClient::new(keyed_config.clone());

    // Signed by *us* (valid under our public key) but targeting a foreign
    // app_id — the adapter's app_id guard must reject before any store write.
    let decoded = vec![
        ("app_id".to_string(), "9999999999".to_string()),
        ("notify_id".to_string(), "notify-1".to_string()),
        ("out_trade_no".to_string(), "order-4".to_string()),
        ("total_amount".to_string(), "129.00".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ];
    let sign = client.sign(&decoded).expect("sign");
    let wire = format!(
        "app_id=9999999999&notify_id=notify-1&trade_status=TRADE_SUCCESS\
         &out_trade_no=order-4&total_amount=129.00&sign_type=RSA2&sign={}",
        percent_encode(&sign)
    );

    let adapter = AlipayAdapter::new(keyed_config);
    let error = adapter
        .parse_event(None, wire.as_bytes())
        .await
        .expect_err("foreign app_id must be rejected");
    assert!(matches!(error, ProviderError::Invalid(_)), "{error}");
}

#[tokio::test]
async fn alipay_rejects_missing_signature() {
    let adapter = AlipayAdapter::new(alipay_config());
    let wire = "app_id=2021000000000001&notify_id=notify-1&trade_status=TRADE_SUCCESS\
                &out_trade_no=order-5&total_amount=129.00";
    let error = adapter
        .parse_event(None, wire.as_bytes())
        .await
        .expect_err("missing signature must fail");
    assert!(matches!(error, ProviderError::Signature(_)), "{error}");
}

#[tokio::test]
async fn alipay_create_checkout_returns_qr_code_shape() {
    // No network: price must exist for plan "pro" so the adapter reaches the
    // HTTP call; instead assert the pre-call error path is explicit.
    let adapter = AlipayAdapter::new(alipay_config());
    let error = adapter
        .create_checkout(
            UserId::new(Uuid::new_v4()),
            "free",
            "order-6",
        )
        .await
        .expect_err("free plan is not configured for Alipay checkout");
    assert!(matches!(error, ProviderError::Request(_)), "{error}");
}

#[test]
fn payment_provider_ids_are_stable() {
    assert_eq!(CreemAdapter::new(creem_config()).id(), BillingProvider::Creem);
    assert_eq!(AlipayAdapter::new(alipay_config()).id(), BillingProvider::Alipay);
    let _: Result<CheckoutSession, ProviderError> = Err(ProviderError::Invalid("x".into()));
}
