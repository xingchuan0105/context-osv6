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

    // Defaults from from_env when Alipay/Creem price envs empty:
    assert_eq!(
        config.price_label_for_plan(PLAN_PRO),
        "¥39.00 / 月 · $5.99 / 月"
    );
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
