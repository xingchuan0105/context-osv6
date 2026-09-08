use web_ui::i18n::{UiLocale, catalog_has, catalog_len, interpolate, lookup};

#[test]
fn catalog_is_full_and_covers_product_keys() {
    assert!(catalog_len() > 1500, "expected Next+extras catalog, got {}", catalog_len());
    for key in [
        "pricingTitle",
        "authLoginTitle",
        "chat.newConversation",
        "workspaceSend",
        "dashboardAccountLink",
        "settings.appearance.localeLabel",
        "workspaceLanguageChinese",
        "workspaceLanguageEnglish",
        "notFound.title",
        "chat.modeLineNoSessionFiles",
    ] {
        assert!(catalog_has(key), "missing {key}");
    }
}

#[test]
fn lookup_switches_locale_and_interpolates() {
    assert_eq!(lookup(UiLocale::ZhCn, "workspaceSend"), "发送");
    assert_eq!(lookup(UiLocale::En, "workspaceSend"), "Send");
    assert_eq!(
        interpolate(&lookup(UiLocale::En, "pricingWalletBalance"), &[("balance", "¥12")]),
        "Balance ¥12"
    );
    assert_eq!(lookup(UiLocale::En, "missing.key"), "missing.key");
}

#[test]
fn locale_parse_normalizes_zh_default() {
    assert_eq!(UiLocale::parse("en").as_str(), "en");
    assert_eq!(UiLocale::parse("zh-CN").as_str(), "zh-CN");
    assert_eq!(UiLocale::parse("zh").as_str(), "zh-CN");
}
