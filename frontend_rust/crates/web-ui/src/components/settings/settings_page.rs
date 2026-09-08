use super::providers_panel::ProvidersPanel;
use crate::components::shell::ProductChrome;
use crate::components::ui::Toaster;
use crate::i18n::{UiLocale, UiTheme, t_now, use_i18n};
use leptos::prelude::*;
use leptos_router::hooks::{query_signal, use_navigate};
use leptos_router::NavigateOptions;
use web_sdk::clear_browser_auth;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let navigate = use_navigate();
    let (tab_query, set_tab_query) = query_signal::<String>("tab");

    let current_tab = Signal::derive(move || {
        tab_query.get().unwrap_or_else(|| "profile".to_string())
    });

    let on_logout = {
        let navigate = navigate.clone();
        move |_| {
            clear_browser_auth();
            token.set(String::new());
            navigate("/login", NavigateOptions::default());
        }
    };

    view! {
        <ProductChrome>
        <div class="settings-shell" data-testid="settings-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <h1 class="settings-title">{move || i18n.t("settings.pageHeading")}</h1>
                </div>
                <button
                    type="button"
                    class="settings-logout-btn"
                    data-testid="settings-logout"
                    on:click=on_logout
                >
                    {move || i18n.t("dashboardLogout")}
                </button>
            </header>

            <nav class="settings-nav" aria-label=move || i18n.t("settings.navAria")>
                <button
                    type="button"
                    class=move || {
                        if current_tab.get() == "providers" {
                            "settings-nav-item is-active"
                        } else {
                            "settings-nav-item"
                        }
                    }
                    data-testid="tab-providers"
                    on:click=move |_| set_tab_query.set(Some("providers".to_string()))
                >
                    {move || i18n.t("settings.tabs.providers")}
                </button>
                <button
                    type="button"
                    class=move || {
                        if current_tab.get() == "profile" {
                            "settings-nav-item is-active"
                        } else {
                            "settings-nav-item"
                        }
                    }
                    data-testid="tab-profile"
                    on:click=move |_| set_tab_query.set(Some("profile".to_string()))
                >
                    {move || i18n.t("settings.tabs.profile")}
                </button>
                <button
                    type="button"
                    class=move || {
                        if current_tab.get() == "preferences" {
                            "settings-nav-item is-active"
                        } else {
                            "settings-nav-item"
                        }
                    }
                    data-testid="tab-preferences"
                    on:click=move |_| set_tab_query.set(Some("preferences".to_string()))
                >
                    {move || i18n.t("settings.tabs.preferences")}
                </button>
                <button
                    type="button"
                    class=move || {
                        if current_tab.get() == "billing" {
                            "settings-nav-item is-active"
                        } else {
                            "settings-nav-item"
                        }
                    }
                    data-testid="tab-billing"
                    on:click=move |_| set_tab_query.set(Some("billing".to_string()))
                >
                    {move || i18n.t("settings.tabs.billing")}
                </button>
                <button
                    type="button"
                    class=move || {
                        if current_tab.get() == "security" {
                            "settings-nav-item is-active"
                        } else {
                            "settings-nav-item"
                        }
                    }
                    data-testid="tab-security"
                    on:click=move |_| set_tab_query.set(Some("security".to_string()))
                >
                    {move || i18n.t("settings.tabs.security")}
                </button>
                <a href="/settings/usage" class="settings-nav-item" data-testid="tab-usage">
                    {move || i18n.t("settings.usageLink")}
                </a>
            </nav>

            <main class="settings-content">
                {move || {
                    match current_tab.get().as_str() {
                        "providers" => view! { <ProvidersPanel/> }.into_any(),
                        "profile" => view! { <ProfilePanel/> }.into_any(),
                        "preferences" => view! { <PreferencesPanel/> }.into_any(),
                        "billing" => view! {
                            <section class="settings-panel" data-testid="billing-panel">
                                <h2>{move || i18n.t("settings.tabs.billing")}</h2>
                                <p>{move || i18n.t("settings.billingHint")}</p>
                                <a href="/pricing" data-testid="billing-goto-pricing">{move || i18n.t("settings.billingGoto")}</a>
                            </section>
                        }
                        .into_any(),
                        "security" => view! {
                            <section class="settings-panel" data-testid="security-panel">
                                <h2>{move || i18n.t("settings.tabs.security")}</h2>
                                <p>{move || i18n.t("settings.securityHint")}</p>
                                <a href="/reset-password">{move || i18n.t("settings.changePassword")}</a>
                            </section>
                        }
                        .into_any(),
                        _ => view! { <ProfilePanel/> }.into_any(),
                    }
                }}
            </main>
        </div>
        </ProductChrome>
    }
}

#[component]
fn ProfilePanel() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let toaster = expect_context::<Toaster>();
    let i18n = use_i18n();
    let name = RwSignal::new(String::new());
    let initialized = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if let Some(auth) = web_sdk::read_browser_auth() {
            name.set(auth.user.full_name);
        }
        initialized.set(true);
    });
    view! {
        <section class="settings-panel" data-testid="profile-panel">
            <h2>{move || i18n.t("settings.profile.sectionTitle")}</h2>
            <Show when=move || initialized.get()>
            <form
                class="settings-form"
                on:submit=move |ev| {
                    ev.prevent_default();
                    if busy.get_untracked() { return; }
                    let tok = token.get_untracked();
                    let submitted_name = name.get_untracked().trim().to_string();
                    busy.set(true);
                    error.set(None);
                    leptos::task::spawn_local(async move {
                        let client = web_sdk::BrowserRestClient::new(&crate::api_base::poc_api_base(), Some(tok.clone()));
                        // The endpoint replaces profile fields; read current values so a name edit
                        // does not erase the user's biography or public-profile preferences.
                        let result = async {
                            let mut profile = client.get_profile().await?;
                            profile.full_name = submitted_name;
                            client.update_profile(&profile).await
                        }.await;
                        match result {
                            Ok(user) => {
                                name.set(user.full_name.clone());
                                web_sdk::write_browser_auth(&web_sdk::PersistedAuth { token: tok, user });
                                toaster.push(t_now("settings.profileSaved"));
                            }
                            Err(err) => error.set(Some(i18n.tf("settings.profileSaveFailed", &[("error", &err.to_string())]))),
                        }
                        busy.set(false);
                    });
                }
            >
                <label>
                    {move || i18n.t("settings.profile.nameLabel")}
                    <input
                        type="text"
                        maxlength="120"
                        disabled=move || busy.get()
                        data-testid="profile-name"
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </label>
                <button type="submit" disabled=move || busy.get() data-testid="profile-save">{move || i18n.t("settings.profile.saveAction")}</button>
                {move || error.get().map(|message| view! { <p role="alert" class="settings-error">{message}</p> })}
            </form>
            </Show>
        </section>
    }
}

#[component]
fn PreferencesPanel() -> impl IntoView {
    let toaster = expect_context::<Toaster>();
    let i18n = use_i18n();
    view! {
        <section class="settings-panel" data-testid="preferences-panel">
            <h2>{move || i18n.t("settings.tabs.preferences")}</h2>
            <p>{move || i18n.t("settings.prefsSubtitle")}</p>
            <label>
                {move || i18n.t("settings.appearance.themeLabel")}
                <select
                    data-testid="pref-theme"
                    prop:value=move || i18n.theme.get().as_str().to_string()
                    on:change=move |ev| i18n.set_theme(UiTheme::parse(&event_target_value(&ev)))
                >
                    <option value="system">{move || i18n.t("settings.appearance.theme.system")}</option>
                    <option value="light">{move || i18n.t("settings.appearance.theme.light")}</option>
                    <option value="dark">{move || i18n.t("settings.appearance.theme.dark")}</option>
                </select>
            </label>
            <label>
                {move || i18n.t("settings.appearance.localeLabel")}
                <select
                    data-testid="pref-locale"
                    prop:value=move || i18n.locale.get().as_str().to_string()
                    on:change=move |ev| i18n.set_locale(UiLocale::parse(&event_target_value(&ev)))
                >
                    <option value="zh-CN">{move || i18n.t("workspaceLanguageChinese")}</option>
                    <option value="en">{move || i18n.t("workspaceLanguageEnglish")}</option>
                </select>
            </label>
            <button
                type="button"
                data-testid="pref-save"
                on:click=move |_| {
                    toaster.push(t_now("settings.prefsSaved"));
                }
            >
                {move || i18n.t("settings.savePrefs")}
            </button>
        </section>
    }
}
