use crate::core::build_plan_payloads;
use crate::types::{BillingConfig, PLAN_PRO};
use std::collections::HashMap;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn billing_config_marks_checkout_available_for_creem_and_alipay() {
    let _guard = ENV_MUTEX.lock().unwrap();
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
fn billing_config_price_labels_without_stripe() {
    let _guard = ENV_MUTEX.lock().unwrap();
    remove_env("BILLING_PRICE_LABEL_PRO");
    remove_env("BILLING_PRICE_LABEL_PLUS");
    remove_env("ALIPAY_PRICE_PRO");
    remove_env("ALIPAY_PRICE_PLUS");
    remove_env("CREEM_PRICE_PRO");
    remove_env("CREEM_PRICE_PLUS");

    let config = BillingConfig::from_env();

    // Defaults from from_env when Alipay/Creem price envs empty (frozen pricing: Pro ¥129 / $19).
    assert_eq!(
        config.price_label_for_plan(PLAN_PRO),
        "¥129.00 / 月 · $19.00 / 月"
    );
}

#[test]
fn percent_decode_preserves_multibyte_utf8() {
    use crate::service::percent_decode;

    // "中" percent-encoded (as Alipay notify form params arrive).
    assert_eq!(percent_decode("%E4%B8%AD%E6%96%87"), "中文");
    assert_eq!(percent_decode("a+b%20c"), "a b c");
    // Invalid sequences pass through literally.
    assert_eq!(percent_decode("100%zz%"), "100%zz%");
    assert_eq!(percent_decode("plain"), "plain");
}

#[test]
fn alipay_amount_compare_uses_cents() {
    // process.rs compares notify total_amount against billing_orders.amount_cents.
    assert_eq!(BillingConfig::decimal_price_to_cents("129.00"), 12900);
    assert_eq!(BillingConfig::decimal_price_to_cents("49.00"), 4900);
    assert_eq!(BillingConfig::decimal_price_to_cents("0.01"), 1);
    assert_eq!(BillingConfig::decimal_price_to_cents(""), 0);
}

#[test]
fn topup_pack_fen_matches_alipay_decimal_and_cents() {
    for pack in app_core::DEFAULT_TOPUP_PACKS {
        let decimal = app_core::fen_to_decimal_amount(pack.amount_fen);
        assert_eq!(
            BillingConfig::decimal_price_to_cents(&decimal),
            pack.amount_fen,
            "pack {} fen/cents round-trip",
            pack.id
        );
        assert_eq!(decimal, format!("{}.00", pack.amount_yuan));
    }
}

#[test]
fn create_checkout_request_defaults_subscription_without_kind() {
    let raw = r#"{"plan_id":"pro","provider":"alipay"}"#;
    let parsed: crate::CreateCheckoutRequest =
        serde_json::from_str(raw).expect("legacy subscription body still deserializes");
    assert_eq!(parsed.plan_id.as_deref(), Some("pro"));
    assert!(parsed.kind.is_none());
    assert!(parsed.topup_pack_id.is_none());
}

#[test]
fn create_checkout_request_wallet_topup_fields() {
    let raw = r#"{"kind":"wallet_topup","topup_pack_id":"topup_50","provider":"creem"}"#;
    let parsed: crate::CreateCheckoutRequest =
        serde_json::from_str(raw).expect("wallet topup body deserializes");
    assert_eq!(parsed.kind.as_deref(), Some("wallet_topup"));
    assert_eq!(parsed.topup_pack_id.as_deref(), Some("topup_50"));
}

#[test]
fn build_plan_payloads_does_not_require_stripe() {
    let config = BillingConfig {
        billing_price_label_pro: "$20/month".to_string(),
        billing_price_label_plus: "Contact sales".to_string(),
        public_app_base_url: "http://localhost:3000".to_string(),
        creem_api_key: "k".to_string(),
        creem_product_pro: "prod_pro".to_string(),
        creem_product_plus: "prod_plus".to_string(),
        ..Default::default()
    };
    let quotas: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    let plans = build_plan_payloads(&config, PLAN_PRO, &quotas);
    assert!(!plans.is_empty());
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
