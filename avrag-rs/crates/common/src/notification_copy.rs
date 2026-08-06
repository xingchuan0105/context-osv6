//! In-app notification title/body — loaded from `avrag-rs/notifications/*.txt`.
//!
//! These strings are persisted and shown in the product bell. Prefer editing the
//! text files, not call sites. Default product locale is Chinese.

/// Locale for notification copy (product default: Chinese).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyLocale {
    Zh,
    En,
}

impl NotifyLocale {
    pub fn product_default() -> Self {
        Self::Zh
    }

    /// Parse optional BCP-47-ish tags (`zh-CN`, `en`, …). Unknown → Chinese.
    pub fn from_lang_tag(raw: Option<&str>) -> Self {
        let Some(tag) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
            return Self::Zh;
        };
        let lower = tag.to_ascii_lowercase();
        if lower == "en" || lower.starts_with("en-") {
            Self::En
        } else {
            Self::Zh
        }
    }
}

/// Known in-app notification kinds (stable product events).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyKind {
    IngestionSuccess,
    IngestionFailed,
    FundsRequired,
    PasswordChanged,
    ShareEnabled,
    SubscriptionPaid,
    SubscriptionExpired,
    BillingUpdate,
    DegradeGeneral,
    DegradeSearch,
    DegradeRag,
}

impl NotifyKind {
    /// Map billing outbox `event_type` strings to copy kinds.
    pub fn from_billing_outbox(event_type: &str) -> Self {
        match event_type {
            "subscription.paid" => Self::SubscriptionPaid,
            "subscription.expired" => Self::SubscriptionExpired,
            _ => Self::BillingUpdate,
        }
    }

    /// Map chat agent mode to degrade notification copy.
    pub fn degrade_for_mode(mode: &str, is_direct_chat: bool) -> Self {
        if is_direct_chat {
            Self::DegradeGeneral
        } else if mode == "search" {
            Self::DegradeSearch
        } else {
            Self::DegradeRag
        }
    }
}

/// Rendered notification title + body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyCopy {
    pub title: String,
    pub body: String,
}

/// Replace `{name}` placeholders. Unknown keys are left as-is.
pub fn render_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        let needle = format!("{{{key}}}");
        out = out.replace(&needle, value);
    }
    out
}

fn asset<'a>(locale: NotifyLocale, zh: &'a str, en: &'a str) -> &'a str {
    match locale {
        NotifyLocale::Zh => zh,
        NotifyLocale::En => en,
    }
}

fn pair(
    locale: NotifyLocale,
    title_zh: &str,
    title_en: &str,
    body_zh: &str,
    body_en: &str,
    vars: &[(&str, &str)],
) -> NotifyCopy {
    let title = render_template(asset(locale, title_zh, title_en).trim(), vars);
    let body = render_template(asset(locale, body_zh, body_en).trim(), vars);
    NotifyCopy { title, body }
}

/// Render notification copy for `kind` in `locale`.
pub fn render(kind: NotifyKind, locale: NotifyLocale) -> NotifyCopy {
    render_with(kind, locale, &[])
}

/// Render with optional `{placeholder}` substitutions.
pub fn render_with(kind: NotifyKind, locale: NotifyLocale, vars: &[(&str, &str)]) -> NotifyCopy {
    // Paths are relative to this source file: crates/common/src → avrag-rs/notifications
    match kind {
        NotifyKind::IngestionSuccess => pair(
            locale,
            include_str!("../../../notifications/ingestion-success.title.zh.txt"),
            include_str!("../../../notifications/ingestion-success.title.en.txt"),
            include_str!("../../../notifications/ingestion-success.body.zh.txt"),
            include_str!("../../../notifications/ingestion-success.body.en.txt"),
            vars,
        ),
        NotifyKind::IngestionFailed => pair(
            locale,
            include_str!("../../../notifications/ingestion-failed.title.zh.txt"),
            include_str!("../../../notifications/ingestion-failed.title.en.txt"),
            include_str!("../../../notifications/ingestion-failed.body.zh.txt"),
            include_str!("../../../notifications/ingestion-failed.body.en.txt"),
            vars,
        ),
        NotifyKind::FundsRequired => pair(
            locale,
            include_str!("../../../notifications/funds-required.title.zh.txt"),
            include_str!("../../../notifications/funds-required.title.en.txt"),
            include_str!("../../../notifications/funds-required.body.zh.txt"),
            include_str!("../../../notifications/funds-required.body.en.txt"),
            vars,
        ),
        NotifyKind::PasswordChanged => pair(
            locale,
            include_str!("../../../notifications/password-changed.title.zh.txt"),
            include_str!("../../../notifications/password-changed.title.en.txt"),
            include_str!("../../../notifications/password-changed.body.zh.txt"),
            include_str!("../../../notifications/password-changed.body.en.txt"),
            vars,
        ),
        NotifyKind::ShareEnabled => pair(
            locale,
            include_str!("../../../notifications/share-enabled.title.zh.txt"),
            include_str!("../../../notifications/share-enabled.title.en.txt"),
            include_str!("../../../notifications/share-enabled.body.zh.txt"),
            include_str!("../../../notifications/share-enabled.body.en.txt"),
            vars,
        ),
        NotifyKind::SubscriptionPaid => pair(
            locale,
            include_str!("../../../notifications/subscription-paid.title.zh.txt"),
            include_str!("../../../notifications/subscription-paid.title.en.txt"),
            include_str!("../../../notifications/subscription-paid.body.zh.txt"),
            include_str!("../../../notifications/subscription-paid.body.en.txt"),
            vars,
        ),
        NotifyKind::SubscriptionExpired => pair(
            locale,
            include_str!("../../../notifications/subscription-expired.title.zh.txt"),
            include_str!("../../../notifications/subscription-expired.title.en.txt"),
            include_str!("../../../notifications/subscription-expired.body.zh.txt"),
            include_str!("../../../notifications/subscription-expired.body.en.txt"),
            vars,
        ),
        NotifyKind::BillingUpdate => pair(
            locale,
            include_str!("../../../notifications/billing-update.title.zh.txt"),
            include_str!("../../../notifications/billing-update.title.en.txt"),
            include_str!("../../../notifications/billing-update.body.zh.txt"),
            include_str!("../../../notifications/billing-update.body.en.txt"),
            vars,
        ),
        NotifyKind::DegradeGeneral => pair(
            locale,
            include_str!("../../../notifications/degrade-general.title.zh.txt"),
            include_str!("../../../notifications/degrade-general.title.en.txt"),
            include_str!("../../../notifications/degrade-general.body.zh.txt"),
            include_str!("../../../notifications/degrade-general.body.en.txt"),
            vars,
        ),
        NotifyKind::DegradeSearch => pair(
            locale,
            include_str!("../../../notifications/degrade-search.title.zh.txt"),
            include_str!("../../../notifications/degrade-search.title.en.txt"),
            include_str!("../../../notifications/degrade-search.body.zh.txt"),
            include_str!("../../../notifications/degrade-search.body.en.txt"),
            vars,
        ),
        NotifyKind::DegradeRag => pair(
            locale,
            include_str!("../../../notifications/degrade-rag.title.zh.txt"),
            include_str!("../../../notifications/degrade-rag.title.en.txt"),
            include_str!("../../../notifications/degrade-rag.body.zh.txt"),
            include_str!("../../../notifications/degrade-rag.body.en.txt"),
            vars,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[NotifyKind] = &[
        NotifyKind::IngestionSuccess,
        NotifyKind::IngestionFailed,
        NotifyKind::FundsRequired,
        NotifyKind::PasswordChanged,
        NotifyKind::ShareEnabled,
        NotifyKind::SubscriptionPaid,
        NotifyKind::SubscriptionExpired,
        NotifyKind::BillingUpdate,
        NotifyKind::DegradeGeneral,
        NotifyKind::DegradeSearch,
        NotifyKind::DegradeRag,
    ];

    #[test]
    fn all_kinds_nonempty_both_locales() {
        for kind in ALL {
            for locale in [NotifyLocale::Zh, NotifyLocale::En] {
                let copy = render(*kind, locale);
                assert!(
                    !copy.title.is_empty() && !copy.body.is_empty(),
                    "{kind:?} {locale:?} empty"
                );
                assert!(
                    !copy.title.contains('{') && !copy.body.contains('{'),
                    "{kind:?} leftover placeholder"
                );
            }
        }
    }

    #[test]
    fn funds_zh_mentions_balance() {
        let copy = render(NotifyKind::FundsRequired, NotifyLocale::Zh);
        assert!(copy.title.contains("余额") || copy.body.contains("余额"));
    }

    #[test]
    fn billing_outbox_mapping() {
        assert_eq!(
            NotifyKind::from_billing_outbox("subscription.paid"),
            NotifyKind::SubscriptionPaid
        );
        assert_eq!(
            NotifyKind::from_billing_outbox("subscription.expired"),
            NotifyKind::SubscriptionExpired
        );
        assert_eq!(
            NotifyKind::from_billing_outbox("other"),
            NotifyKind::BillingUpdate
        );
    }
}
