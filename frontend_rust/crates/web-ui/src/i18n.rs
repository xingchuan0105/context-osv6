use leptos::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const LOCALE_COOKIE_NAME: &str = "avrag.ui.locale";
pub const LOCALE_STORAGE_KEY: &str = "avrag.ui.locale.v1";
pub const THEME_STORAGE_KEY: &str = "avrag.ui.theme.v1";

const CATALOG_JSON: &str = include_str!("../i18n/messages.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiLocale {
    ZhCn,
    En,
}

impl UiLocale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "en" | "en-US" | "en-GB" => Self::En,
            _ => Self::ZhCn,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTheme {
    System,
    Light,
    Dark,
}

impl UiTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }
}

#[derive(Clone, Copy)]
pub struct I18n {
    pub locale: RwSignal<UiLocale>,
    pub theme: RwSignal<UiTheme>,
}

impl I18n {
    pub fn t(self, key: &str) -> String {
        lookup(self.locale.get(), key)
    }

    pub fn tf(self, key: &str, values: &[(&str, &str)]) -> String {
        interpolate(&lookup(self.locale.get(), key), values)
    }

    pub fn set_locale(self, locale: UiLocale) {
        self.locale.set(locale);
    }

    pub fn set_theme(self, theme: UiTheme) {
        self.theme.set(theme);
    }
}

#[derive(Deserialize)]
struct Descriptor {
    zh: String,
    en: String,
}

fn catalog() -> &'static HashMap<String, Descriptor> {
    static CATALOG: OnceLock<HashMap<String, Descriptor>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(CATALOG_JSON).expect("i18n catalog JSON")
    })
}

pub fn lookup(locale: UiLocale, key: &str) -> String {
    match catalog().get(key) {
        Some(desc) => match locale {
            UiLocale::ZhCn => desc.zh.clone(),
            UiLocale::En => desc.en.clone(),
        },
        None => key.to_string(),
    }
}

pub fn interpolate(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in values {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

pub fn catalog_len() -> usize {
    catalog().len()
}

pub fn catalog_has(key: &str) -> bool {
    catalog().contains_key(key)
}

pub fn provide_i18n() {
    let locale = RwSignal::new(read_initial_locale());
    let theme = RwSignal::new(read_initial_theme());
    provide_context(I18n { locale, theme });
    Effect::new(move |_| {
        persist_preferences(locale.get(), theme.get());
    });
}

pub fn expect_i18n() -> I18n {
    expect_context::<I18n>()
}

pub fn use_i18n() -> I18n {
    use_context::<I18n>().unwrap_or_else(|| I18n {
        locale: RwSignal::new(UiLocale::ZhCn),
        theme: RwSignal::new(UiTheme::System),
    })
}

pub fn active_locale() -> UiLocale {
    use_context::<I18n>()
        .map(|i18n| i18n.locale.get_untracked())
        .unwrap_or(UiLocale::ZhCn)
}

pub fn t_now(key: &str) -> String {
    lookup(active_locale(), key)
}

pub fn tf_now(key: &str, values: &[(&str, &str)]) -> String {
    interpolate(&t_now(key), values)
}

#[component]
pub fn Tx(k: &'static str) -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t(k)
}

fn read_initial_locale() -> UiLocale {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(value) = storage_get(LOCALE_STORAGE_KEY) {
            return UiLocale::parse(&value);
        }
        if let Some(value) = cookie_get(LOCALE_COOKIE_NAME) {
            return UiLocale::parse(&value);
        }
    }
    UiLocale::ZhCn
}

fn read_initial_theme() -> UiTheme {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(value) = storage_get(THEME_STORAGE_KEY) {
            return UiTheme::parse(&value);
        }
    }
    UiTheme::System
}

fn persist_preferences(locale: UiLocale, theme: UiTheme) {
    #[cfg(target_arch = "wasm32")]
    {
        storage_set(LOCALE_STORAGE_KEY, locale.as_str());
        storage_set(THEME_STORAGE_KEY, theme.as_str());
        cookie_set(LOCALE_COOKIE_NAME, locale.as_str());
        apply_document(locale, theme);
    }
    let _ = (locale, theme);
}

#[cfg(target_arch = "wasm32")]
fn storage_get(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(key)
        .ok()
        .flatten()
}

#[cfg(target_arch = "wasm32")]
fn storage_set(key: &str, value: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item(key, value);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn cookie_get(name: &str) -> Option<String> {
    use wasm_bindgen::JsCast;
    let document = web_sys::window()?.document()?;
    let html_doc: web_sys::HtmlDocument = document.dyn_into().ok()?;
    let cookies = html_doc.cookie().ok()?;
    for part in cookies.split(';') {
        let part = part.trim();
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == name {
            return Some(js_unescape(value));
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn cookie_set(name: &str, value: &str) {
    use wasm_bindgen::JsCast;
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(html_doc) = document.dyn_into::<web_sys::HtmlDocument>() {
            let _ = html_doc.set_cookie(&format!(
                "{name}={value}; Path=/; Max-Age=31536000; SameSite=Lax"
            ));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn apply_document(locale: UiLocale, theme: UiTheme) {
    let Some(el) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    let _ = el.set_attribute("lang", locale.as_str());
    match theme {
        UiTheme::System => {
            let _ = el.remove_attribute("data-theme");
        }
        other => {
            let _ = el.set_attribute("data-theme", other.as_str());
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn js_unescape(value: &str) -> String {
    js_sys::decode_uri_component(value)
        .ok()
        .and_then(|decoded| decoded.as_string())
        .unwrap_or_else(|| value.to_string())
}
