use crate::api_base::poc_api_base;
use crate::i18n::{UiLocale, UiTheme, use_i18n};
use crate::routes::dest;
use leptos::prelude::*;
use web_sdk::{clear_browser_auth, read_browser_auth, BrowserRestClient};

#[component]
pub fn AccountMenu() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    let trigger = NodeRef::<leptos::html::Button>::new();
    let show_admin = RwSignal::new(false);
    let flyout = RwSignal::new(None::<&'static str>);

    let probe_admin = move || {
        let tok = if token.get_untracked().is_empty() {
            read_browser_auth()
                .map(|auth| {
                    token.set(auth.token.clone());
                    auth.token
                })
                .unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() {
            show_admin.set(false);
            return;
        }
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            show_admin.set(client.probe_admin_access().await);
        });
    };

    view! {
        <div class="app-menu" on:keydown=move |event: leptos::ev::KeyboardEvent| {
            if event.key() == "Escape" && open.get_untracked() {
                event.prevent_default(); open.set(false);
                #[cfg(target_arch = "wasm32")]
                if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
            }
        }>
            <button
                type="button"
                class="app-top-bar-capsule"
                node_ref=trigger
                aria-expanded=move || open.get()
                aria-label=move || i18n.t("dashboardAccountLink")
                data-testid="dashboard-account-menu-trigger"
                on:click=move |_| {
                    let next = !open.get();
                    open.set(next);
                    flyout.set(None);
                    if next {
                        probe_admin();
                    }
                }
            >
                {move || i18n.t("dashboardAccountLink")}
            </button>
            <Show when=move || open.get()>
                <button
                    type="button"
                    class="app-menu-dismiss"
                    tabindex="-1"
                    aria-label=move || i18n.t("commonMenuClose")
                    on:click=move |_| {
                        open.set(false);
                        flyout.set(None);
                        request_animation_frame(move || {
                            #[cfg(target_arch = "wasm32")]
                            if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
                        });
                    }
                />
                <div class="app-menu-panel"  data-testid="dashboard-account-menu">
                    {move || {
                        let signed_in = !token.get().is_empty() || read_browser_auth().is_some();
                        let auth = read_browser_auth();
                        let fallback = i18n.t("dashboardAccountLink");
                        let name = auth
                            .as_ref()
                            .map(|a| {
                                let n = a.user.full_name.trim();
                                if n.is_empty() {
                                    a.user.email.split('@').next().unwrap_or(fallback.as_str()).to_string()
                                } else {
                                    n.to_string()
                                }
                            })
                            .unwrap_or_else(|| fallback.clone());
                        let email = auth.map(|a| a.user.email).unwrap_or_default();
                        let initial = name.chars().next().unwrap_or('U').to_uppercase().to_string();
                        view! {
                            <Show when=move || signed_in>
                                <div class="app-account-card" data-testid="account-user-card">
                                    <span class="app-account-avatar" aria-hidden="true">{initial.clone()}</span>
                                    <div class="app-account-meta">
                                        <strong class="app-account-name">{name.clone()}</strong>
                                        <span class="app-account-email">{email.clone()}</span>
                                    </div>
                                </div>
                            </Show>
                            <a
                                class="app-menu-item"
                                href=dest::PRICING

                                data-testid="account-membership-cta"
                                on:click=move |_| open.set(false)
                            >
                                {move || i18n.t("upgradeModal.title")}
                            </a>
                            <a
                                class="app-menu-item"
                                href=dest::SETTINGS

                                data-testid="account-settings-link"
                                on:click=move |_| open.set(false)
                            >
                                {move || i18n.t("appPrimaryNav.settings")}
                            </a>
                            <a
                                class="app-menu-item"
                                href=dest::HELP

                                data-testid="account-help-link"
                                on:click=move |_| open.set(false)
                            >
                                {move || i18n.t("accountMenu.help")}
                            </a>
                            <a class="app-menu-item" href=dest::LEGAL data-testid="account-legal-link" on:click=move |_| open.set(false)>
                                {move || i18n.t("productChrome.legalCenter")}
                            </a>
                            <Show when=move || show_admin.get()>
                                <a
                                    class="app-menu-item"
                                    href="/admin"

                                    data-testid="account-admin-link"
                                    on:click=move |_| open.set(false)
                                >
                                    {move || i18n.t("accountMenu.adminConsole")}
                                </a>
                            </Show>
                            <button
                                type="button"
                                class="app-menu-item"

                                data-testid="account-theme-toggle" aria-expanded=move || flyout.get() == Some("theme")
                                on:click=move |_| {
                                    flyout.update(|current| {
                                        *current = if *current == Some("theme") { None } else { Some("theme") };
                                    });
                                }
                            >
                                {move || format!("{} ▸", i18n.t("settings.appearance.themeLabel"))}
                            </button>
                            <Show when=move || flyout.get() == Some("theme")>
                                <div class="app-menu-flyout"  data-testid="account-theme-menu">
                                    <button
                                        type="button"
                                        class="app-menu-item"
                                        data-testid="account-theme-system"
                                        on:click=move |_| {
                                            i18n.set_theme(UiTheme::System);
                                            flyout.set(None);
                                            open.set(false);
                                            #[cfg(target_arch = "wasm32")]
                                            if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
                                        }
                                    >
                                        {move || theme_item(i18n, UiTheme::System, "settings.appearance.theme.system")}
                                    </button>
                                    <button
                                        type="button"
                                        class="app-menu-item"
                                        data-testid="account-theme-light"
                                        on:click=move |_| {
                                            i18n.set_theme(UiTheme::Light);
                                            flyout.set(None);
                                            open.set(false);
                                            #[cfg(target_arch = "wasm32")]
                                            if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
                                        }
                                    >
                                        {move || theme_item(i18n, UiTheme::Light, "settings.appearance.theme.light")}
                                    </button>
                                    <button
                                        type="button"
                                        class="app-menu-item"
                                        data-testid="account-theme-dark"
                                        on:click=move |_| {
                                            i18n.set_theme(UiTheme::Dark);
                                            flyout.set(None);
                                            open.set(false);
                                            #[cfg(target_arch = "wasm32")]
                                            if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
                                        }
                                    >
                                        {move || theme_item(i18n, UiTheme::Dark, "settings.appearance.theme.dark")}
                                    </button>
                                </div>
                            </Show>
                            <button
                                type="button"
                                class="app-menu-item"

                                data-testid="account-locale-toggle" aria-expanded=move || flyout.get() == Some("locale")
                                on:click=move |_| {
                                    flyout.update(|current| {
                                        *current = if *current == Some("locale") { None } else { Some("locale") };
                                    });
                                }
                            >
                                {move || format!("{} ▸", i18n.t("settings.appearance.localeLabel"))}
                            </button>
                            <Show when=move || flyout.get() == Some("locale")>
                                <div class="app-menu-flyout"  data-testid="account-locale-menu">
                                    <button
                                        type="button"
                                        class="app-menu-item"
                                        data-testid="account-locale-zh-CN"
                                        on:click=move |_| {
                                            i18n.set_locale(UiLocale::ZhCn);
                                            flyout.set(None);
                                            open.set(false);
                                            #[cfg(target_arch = "wasm32")]
                                            if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
                                        }
                                    >
                                        {move || locale_item(i18n, UiLocale::ZhCn, "workspaceLanguageChinese")}
                                    </button>
                                    <button
                                        type="button"
                                        class="app-menu-item"
                                        data-testid="account-locale-en"
                                        on:click=move |_| {
                                            i18n.set_locale(UiLocale::En);
                                            flyout.set(None);
                                            open.set(false);
                                            #[cfg(target_arch = "wasm32")]
                                            if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
                                        }
                                    >
                                        {move || locale_item(i18n, UiLocale::En, "workspaceLanguageEnglish")}
                                    </button>
                                </div>
                            </Show>
                            <Show when=move || signed_in>
                                <button
                                    type="button"
                                    class="app-menu-item"

                                    data-testid="account-logout"
                                    on:click=move |_| {
                                        clear_browser_auth();
                                        token.set(String::new());
                                        open.set(false);
                                        #[cfg(target_arch = "wasm32")]
                                        {
                                            let _ = web_sys::window()
                                                .and_then(|window| window.location().set_href("/login").ok());
                                        }
                                    }
                                >
                                    {move || i18n.t("dashboardLogout")}
                                </button>
                            </Show>
                            <Show when=move || !signed_in>
                                <a
                                    class="app-menu-item"
                                    href="/login"

                                    data-testid="account-login-link"
                                    on:click=move |_| open.set(false)
                                >
                                    {move || i18n.t("marketingChrome.login")}
                                </a>
                            </Show>
                        }
                    }}
                </div>
            </Show>
        </div>
    }
}

fn theme_item(i18n: crate::i18n::I18n, value: UiTheme, key: &'static str) -> String {
    let mark = if i18n.theme.get() == value { "✓ " } else { "" };
    format!("{mark}{}", i18n.t(key))
}

fn locale_item(i18n: crate::i18n::I18n, value: UiLocale, key: &'static str) -> String {
    let mark = if i18n.locale.get() == value { "✓ " } else { "" };
    format!("{mark}{}", i18n.t(key))
}
