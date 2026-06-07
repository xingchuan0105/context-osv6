use avrag_billing::BillingConfig;

/// Build a `BillingConfig` with the price-related env vars cleared, so we observe
/// the hard-coded fallbacks in `BillingConfig::from_env`.
fn config_with_clean_pricing_env() -> BillingConfig {
    for key in [
        "BILLING_PRICE_LABEL_PRO",
        "BILLING_PRICE_LABEL_PLUS",
        "ALIPAY_PRICE_PRO",
        "ALIPAY_PRICE_PLUS",
    ] {
        // SAFETY: each `tests/*.rs` file is its own integration-test binary (separate
        // process), so env mutations here cannot race with `module_surface.rs` or
        // `test_migration_0037.rs`. The two `#[test]` fns in this file do run in
        // parallel within the same process, but both clear the same env vars before
        // reading them, so the race is benign.
        unsafe {
            std::env::remove_var(key);
        }
    }
    BillingConfig::from_env()
}

#[test]
fn billing_config_default_alipay_prices_use_new_pricing_revamp() {
    let config = config_with_clean_pricing_env();
    assert_eq!(config.alipay_price_plus(), "49.00");
    assert_eq!(config.alipay_price_pro(), "129.00");
}

#[test]
fn billing_config_price_label_uses_dual_currency() {
    let config = config_with_clean_pricing_env();
    let plus_label = config.price_label_for_plan("plus");
    let pro_label = config.price_label_for_plan("pro");

    assert!(
        plus_label.contains("¥49"),
        "plus price label should contain ¥49, got: {plus_label}"
    );
    assert!(
        plus_label.contains("$9"),
        "plus price label should contain $9, got: {plus_label}"
    );
    assert!(
        pro_label.contains("¥129"),
        "pro price label should contain ¥129, got: {pro_label}"
    );
    assert!(
        pro_label.contains("$19"),
        "pro price label should contain $19, got: {pro_label}"
    );
}
