//! UI preference state such as locale and theme.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::i18n::Locale;

#[cfg(target_arch = "wasm32")]
const UI_PREFS_STORAGE_KEY: &str = "avrag.ui-prefs.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedUiPrefs {
    locale: Locale,
    theme: Theme,
}

#[cfg(target_arch = "wasm32")]
fn read_persisted_prefs() -> Option<PersistedUiPrefs> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    let raw = storage.get(UI_PREFS_STORAGE_KEY).ok().flatten()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_persisted_prefs() -> Option<PersistedUiPrefs> {
    None
}

#[cfg(target_arch = "wasm32")]
fn persist_prefs(locale: Locale, theme: Theme) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let payload = PersistedUiPrefs { locale, theme };
    if let Ok(raw) = serde_json::to_string(&payload) {
        let _ = storage.set(UI_PREFS_STORAGE_KEY, &raw);
    }
}

#[cfg(target_arch = "wasm32")]
fn apply_document_prefs(locale: Locale, theme: Theme) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.document_element() else {
        return;
    };

    let _ = root.set_attribute(
        "lang",
        match locale {
            Locale::ZhCn => "zh-CN",
            Locale::En => "en",
        },
    );

    match theme {
        Theme::System => {
            let _ = root.remove_attribute("data-theme");
        }
        Theme::Light => {
            let _ = root.set_attribute("data-theme", "light");
        }
        Theme::Dark => {
            let _ = root.set_attribute("data-theme", "dark");
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn persist_prefs(_locale: Locale, _theme: Theme) {}

#[cfg(not(target_arch = "wasm32"))]
fn apply_document_prefs(_locale: Locale, _theme: Theme) {}

#[derive(Clone)]
pub struct UiPrefsState {
    pub locale: ReadSignal<Locale>,
    pub set_locale: WriteSignal<Locale>,
    pub theme: ReadSignal<Theme>,
    pub set_theme: WriteSignal<Theme>,
}

pub fn provide_ui_prefs_state() -> UiPrefsState {
    let initial = read_persisted_prefs().unwrap_or_default();
    let (locale, set_locale) = signal(initial.locale);
    let (theme, set_theme) = signal(initial.theme);

    Effect::new(move |_| {
        let current_locale = locale.get();
        let current_theme = theme.get();
        persist_prefs(current_locale, current_theme);
        apply_document_prefs(current_locale, current_theme);
    });

    let state = UiPrefsState {
        locale,
        set_locale,
        theme,
        set_theme,
    };
    provide_context(state.clone());
    state
}

pub fn use_ui_prefs_state() -> UiPrefsState {
    use_context().expect("UiPrefsState not provided - did you call provide_ui_prefs_state()?")
}

#[cfg(test)]
mod tests {
    use super::{PersistedUiPrefs, Theme};
    use crate::i18n::Locale;

    #[test]
    fn persisted_ui_prefs_round_trip_keeps_locale_and_theme() {
        let original = PersistedUiPrefs {
            locale: Locale::En,
            theme: Theme::Dark,
        };
        let json = serde_json::to_string(&original).expect("serialize ui prefs");
        let decoded: PersistedUiPrefs = serde_json::from_str(&json).expect("deserialize ui prefs");

        assert_eq!(decoded.locale, Locale::En);
        assert_eq!(decoded.theme, Theme::Dark);
    }
}
