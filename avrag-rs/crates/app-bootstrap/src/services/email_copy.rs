//! Product email copy — loaded from `avrag-rs/email/*.txt` (not LLM prompts).

/// Mail locale for product SMTP templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailLocale {
    Zh,
    En,
}

impl MailLocale {
    /// Parse optional BCP-47-ish tags from the API (`zh-CN`, `en`, …).
    /// Default is Chinese (product default).
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

    pub fn from_zh_flag(locale_zh: bool) -> Self {
        if locale_zh {
            Self::Zh
        } else {
            Self::En
        }
    }
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

fn asset<'a>(locale: MailLocale, zh: &'a str, en: &'a str) -> &'a str {
    match locale {
        MailLocale::Zh => zh,
        MailLocale::En => en,
    }
}

/// `(subject, body)` for password-reset codes.
pub fn password_reset(
    locale: MailLocale,
    code: &str,
    expires_at: &str,
) -> (String, String) {
    let subject = asset(
        locale,
        include_str!("../../../../email/password-reset.subject.zh.txt"),
        include_str!("../../../../email/password-reset.subject.en.txt"),
    )
    .trim()
    .to_string();
    let body_tmpl = asset(
        locale,
        include_str!("../../../../email/password-reset.body.zh.txt"),
        include_str!("../../../../email/password-reset.body.en.txt"),
    );
    let body = render_template(
        body_tmpl,
        &[("code", code), ("expires_at", expires_at)],
    );
    (subject, body)
}

/// `(subject, body)` for workspace collaboration invites.
pub fn workspace_invite(
    locale: MailLocale,
    inviter: &str,
    workspace_title: &str,
    accept_url: &str,
) -> (String, String) {
    let subject_tmpl = asset(
        locale,
        include_str!("../../../../email/workspace-invite.subject.zh.txt"),
        include_str!("../../../../email/workspace-invite.subject.en.txt"),
    );
    let body_tmpl = asset(
        locale,
        include_str!("../../../../email/workspace-invite.body.zh.txt"),
        include_str!("../../../../email/workspace-invite.body.en.txt"),
    );
    let vars = [
        ("inviter", inviter),
        ("workspace_title", workspace_title),
        ("accept_url", accept_url),
    ];
    (
        render_template(subject_tmpl, &vars).trim().to_string(),
        render_template(body_tmpl, &vars),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_tag_defaults_to_zh() {
        assert_eq!(MailLocale::from_lang_tag(None), MailLocale::Zh);
        assert_eq!(MailLocale::from_lang_tag(Some("")), MailLocale::Zh);
        assert_eq!(MailLocale::from_lang_tag(Some("zh-CN")), MailLocale::Zh);
        assert_eq!(MailLocale::from_lang_tag(Some("en")), MailLocale::En);
        assert_eq!(MailLocale::from_lang_tag(Some("en-US")), MailLocale::En);
    }

    #[test]
    fn password_reset_zh_contains_code() {
        let (subject, body) = password_reset(MailLocale::Zh, "123456", "2026-08-07T12:00:00Z");
        assert!(subject.contains("密码"));
        assert!(body.contains("123456"));
        assert!(body.contains("2026-08-07T12:00:00Z"));
        assert!(!body.contains("{code}"));
    }

    #[test]
    fn password_reset_en_contains_code() {
        let (subject, body) = password_reset(MailLocale::En, "654321", "soon");
        assert!(subject.to_ascii_lowercase().contains("password"));
        assert!(body.contains("654321"));
        assert!(body.contains("soon"));
    }

    #[test]
    fn workspace_invite_substitutes_all_fields() {
        let (subject, body) = workspace_invite(
            MailLocale::Zh,
            "alice@example.com",
            "研究库",
            "https://app.example/invite/w/m",
        );
        assert!(subject.contains("研究库"));
        assert!(body.contains("alice@example.com"));
        assert!(body.contains("https://app.example/invite/w/m"));
        assert!(!body.contains("{accept_url}"));
    }
}
