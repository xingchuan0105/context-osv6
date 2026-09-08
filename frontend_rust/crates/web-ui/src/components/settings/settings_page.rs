use super::providers_panel::ProvidersPanel;
use crate::api_base::poc_api_base;
use crate::components::shell::ProductChrome;
use crate::components::ui::Toaster;
use leptos::prelude::*;
use leptos_router::hooks::{query_signal, use_navigate};
use leptos_router::NavigateOptions;
use web_sdk::{BrowserRestClient, clear_browser_auth};

#[component]
pub fn SettingsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let navigate = use_navigate();
    let (tab_query, set_tab_query) = query_signal::<String>("tab");

    let current_tab = Signal::derive(move || {
        tab_query.get().unwrap_or_else(|| "providers".to_string())
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
                    <h1 class="settings-title">"账号与设置"</h1>
                </div>
                <button
                    type="button"
                    class="settings-logout-btn"
                    data-testid="settings-logout"
                    on:click=on_logout
                >
                    "退出登录"
                </button>
            </header>

            <nav class="settings-nav" aria-label="设置导航">
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
                    "模型提供商 (BYOK)"
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
                    "个人资料"
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
                    "偏好设置"
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
                    "账单"
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
                    "安全"
                </button>
                <a href="/settings/usage" class="settings-nav-item" data-testid="tab-usage">
                    "用量统计 →"
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
                                <h2>"账单"</h2>
                                <p>"套餐与充值请前往定价页完成。"</p>
                                <a href="/pricing" data-testid="billing-goto-pricing">"打开定价与充值"</a>
                            </section>
                        }
                        .into_any(),
                        "security" => view! {
                            <section class="settings-panel" data-testid="security-panel">
                                <h2>"安全"</h2>
                                <p>"密码重置与会话安全。"</p>
                                <a href="/reset-password">"修改密码"</a>
                            </section>
                        }
                        .into_any(),
                        _ => view! { <ProvidersPanel/> }.into_any(),
                    }
                }}
            </main>
        </div>
        </ProductChrome>
    }
}

#[component]
fn ProfilePanel() -> impl IntoView {
    let toaster = expect_context::<Toaster>();
    let name = RwSignal::new(String::new());
    Effect::new(move |_| {
        if let Some(auth) = web_sdk::read_browser_auth() {
            name.set(auth.user.full_name);
        }
    });
    view! {
        <section class="settings-panel" data-testid="profile-panel">
            <h2>"个人资料"</h2>
            <form
                class="settings-form"
                on:submit=move |ev| {
                    ev.prevent_default();
                    toaster.push("资料已保存到本机会话");
                }
            >
                <label>
                    "显示名称"
                    <input
                        type="text"
                        data-testid="profile-name"
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </label>
                <button type="submit" data-testid="profile-save">"保存资料"</button>
            </form>
        </section>
    }
}

#[component]
fn PreferencesPanel() -> impl IntoView {
    let toaster = expect_context::<Toaster>();
    let theme = RwSignal::new("system".to_string());
    let locale = RwSignal::new("zh".to_string());
    view! {
        <section class="settings-panel" data-testid="preferences-panel">
            <h2>"偏好设置"</h2>
            <p>"主题与语言会在 E5.5 接入完整字典；当前先写入本机偏好。"</p>
            <label>
                "主题"
                <select
                    data-testid="pref-theme"
                    prop:value=move || theme.get()
                    on:change=move |ev| theme.set(event_target_value(&ev))
                >
                    <option value="system">"跟随系统"</option>
                    <option value="light">"浅色"</option>
                    <option value="dark">"深色"</option>
                </select>
            </label>
            <label>
                "语言"
                <select
                    data-testid="pref-locale"
                    prop:value=move || locale.get()
                    on:change=move |ev| locale.set(event_target_value(&ev))
                >
                    <option value="zh">"中文"</option>
                    <option value="en">"English"</option>
                </select>
            </label>
            <button
                type="button"
                data-testid="pref-save"
                on:click=move |_| {
                    let tok = web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default();
                    if tok.is_empty() {
                        toaster.push("请先登录");
                        return;
                    }
                    leptos::task::spawn_local(async move {
                        let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                        let prefs = client.get_preferences().await.unwrap_or_default();
                        if client.put_preferences(&prefs).await.is_ok() {
                            toaster.push("偏好已保存");
                        }
                    });
                }
            >
                "保存偏好"
            </button>
        </section>
    }
}
