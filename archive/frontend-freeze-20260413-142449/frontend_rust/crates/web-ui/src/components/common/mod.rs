//! Common/shared UI components

pub mod unavailable_feature_card;

use leptos::prelude::*;

use crate::i18n::{Locale, MessageKey, t};
use crate::state::ui_prefs::use_ui_prefs_state;

pub use unavailable_feature_card::UnavailableFeatureCard;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeTone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

fn notice_tone_classes(tone: NoticeTone) -> &'static str {
    match tone {
        NoticeTone::Neutral => "border-border bg-card text-foreground",
        NoticeTone::Info => "border-sky-200 bg-sky-50 text-sky-900",
        NoticeTone::Success => "border-emerald-200 bg-emerald-50 text-emerald-900",
        NoticeTone::Warning => "border-amber-200 bg-amber-50 text-amber-900",
        NoticeTone::Danger => "border-red-200 bg-red-50 text-red-800",
    }
}

#[component]
pub fn ErrorText(message: String) -> impl IntoView {
    view! {
        <div class="text-sm text-red-600">{message}</div>
    }
}

#[component]
pub fn ErrorBanner(message: String) -> impl IntoView {
    view! {
        <NoticeBanner message=message tone=NoticeTone::Danger />
    }
}

#[component]
pub fn LoadingMessage(message: String) -> impl IntoView {
    view! {
        <span class="text-sm text-muted-foreground">{message}</span>
    }
}

#[component]
pub fn EmptyMessage(message: String) -> impl IntoView {
    view! {
        <div class="text-sm text-muted-foreground">{message}</div>
    }
}

#[component]
pub fn SectionCard(children: Children) -> impl IntoView {
    view! {
        <div class="app-surface-card">
            {children()}
        </div>
    }
}

#[component]
pub fn PageHeading(title: String, #[prop(optional)] subtitle: Option<String>) -> impl IntoView {
    view! {
        <div class="app-page-heading">
            <h1 class="app-page-title">{title}</h1>
            {subtitle
                .filter(|text| !text.is_empty())
                .map(|text| view! { <p class="app-page-subtitle">{text}</p> })}
        </div>
    }
}

#[component]
pub fn FieldLabel(for_id: String, label: String) -> impl IntoView {
    view! {
        <label class="app-form-label" for=for_id>
            {label}
        </label>
    }
}

#[component]
pub fn NoticeBanner(message: String, tone: NoticeTone) -> impl IntoView {
    let classes = notice_tone_classes(tone);
    view! {
        <div class={format!("app-notice-banner {}", classes)}>
            {message}
        </div>
    }
}

#[component]
pub fn StatusBadge(label: String, tone: NoticeTone) -> impl IntoView {
    let classes = match tone {
        NoticeTone::Neutral => "bg-muted text-muted-foreground",
        NoticeTone::Info => "bg-sky-100 text-sky-800",
        NoticeTone::Success => "bg-emerald-100 text-emerald-800",
        NoticeTone::Warning => "bg-amber-100 text-amber-800",
        NoticeTone::Danger => "bg-red-100 text-red-800",
    };
    view! {
        <span class={format!("app-status-badge {}", classes)}>
            {label}
        </span>
    }
}

#[component]
pub fn LocaleToggle() -> impl IntoView {
    let prefs = use_ui_prefs_state();
    let current = prefs.locale;

    view! {
        <div class="inline-flex items-center gap-1 rounded-full border border-border bg-card p-1 shadow-sm">
            <button
                type="button"
                class="app-locale-button"
                class=("app-locale-button-active", move || current.get() == Locale::ZhCn)
                on:click=move |_| prefs.set_locale.set(Locale::ZhCn)
                title=t(Locale::ZhCn, MessageKey::LanguageChinese)
            >
                {Locale::ZhCn.short_label()}
            </button>
            <button
                type="button"
                class="app-locale-button"
                class=("app-locale-button-active", move || current.get() == Locale::En)
                on:click=move |_| prefs.set_locale.set(Locale::En)
                title=t(Locale::En, MessageKey::LanguageEnglish)
            >
                {Locale::En.short_label()}
            </button>
        </div>
    }
}
