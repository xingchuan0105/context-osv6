use super::providers_panel::ProvidersPanel;
use leptos::prelude::*;
use leptos_router::hooks::{query_signal, use_navigate};
use leptos_router::NavigateOptions;
use web_sdk::clear_browser_auth;

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
        <div class="settings-shell" data-testid="settings-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href="/chat" class="settings-back-link">"← 返回对话"</a>
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
                <a href="/settings/usage" class="settings-nav-item">
                    "用量统计 →"
                </a>
            </nav>

            <main class="settings-content">
                {move || {
                    match current_tab.get().as_str() {
                        "providers" => view! { <ProvidersPanel/> }.into_any(),
                        "profile" => view! {
                            <section class="settings-panel" data-testid="profile-panel">
                                <h2>"个人资料"</h2>
                                <p>"个人账户信息与安全设置。"</p>
                            </section>
                        }
                        .into_any(),
                        "preferences" => view! {
                            <section class="settings-panel" data-testid="preferences-panel">
                                <h2>"偏好设置"</h2>
                                <p>"系统外观、通知与交互行为偏好。"</p>
                            </section>
                        }
                        .into_any(),
                        _ => view! { <ProvidersPanel/> }.into_any(),
                    }
                }}
            </main>
        </div>
    }
}
