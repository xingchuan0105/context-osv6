use crate::api_base::poc_api_base;
use crate::routes::dest;
use leptos::prelude::*;
use web_sdk::{clear_browser_auth, read_browser_auth, BrowserRestClient};

#[component]
pub fn AccountMenu() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let open = RwSignal::new(false);
    let show_admin = RwSignal::new(false);

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
        <div class="app-menu">
            <button
                type="button"
                class="app-top-bar-capsule"
                aria-haspopup="menu"
                aria-expanded=move || open.get()
                aria-label="账户"
                data-testid="dashboard-account-menu-trigger"
                on:click=move |_| {
                    let next = !open.get();
                    open.set(next);
                    if next {
                        probe_admin();
                    }
                }
            >
                "账户"
            </button>
            <Show when=move || open.get()>
                <button
                    type="button"
                    class="app-menu-dismiss"
                    aria-label="关闭菜单"
                    on:click=move |_| open.set(false)
                />
                <div class="app-menu-panel" role="menu" data-testid="dashboard-account-menu">
                    {move || {
                        let signed_in = !token.get().is_empty() || read_browser_auth().is_some();
                        let auth = read_browser_auth();
                        let name = auth
                            .as_ref()
                            .map(|a| {
                                let n = a.user.full_name.trim();
                                if n.is_empty() {
                                    a.user.email.split('@').next().unwrap_or("账户").to_string()
                                } else {
                                    n.to_string()
                                }
                            })
                            .unwrap_or_else(|| "账户".to_string());
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
                                role="menuitem"
                                data-testid="account-membership-cta"
                                on:click=move |_| open.set(false)
                            >
                                "会员与充值"
                            </a>
                            <a
                                class="app-menu-item"
                                href=dest::SETTINGS
                                role="menuitem"
                                data-testid="account-settings-link"
                                on:click=move |_| open.set(false)
                            >
                                "设置"
                            </a>
                            <a
                                class="app-menu-item"
                                href=dest::HELP
                                role="menuitem"
                                data-testid="account-help-link"
                                on:click=move |_| open.set(false)
                            >
                                "帮助"
                            </a>
                            <Show when=move || show_admin.get()>
                                <a
                                    class="app-menu-item"
                                    href="/admin"
                                    role="menuitem"
                                    data-testid="account-admin-link"
                                    on:click=move |_| open.set(false)
                                >
                                    "管理台"
                                </a>
                            </Show>
                            <Show when=move || signed_in>
                                <button
                                    type="button"
                                    class="app-menu-item"
                                    role="menuitem"
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
                                    "退出登录"
                                </button>
                            </Show>
                            <Show when=move || !signed_in>
                                <a
                                    class="app-menu-item"
                                    href="/login"
                                    role="menuitem"
                                    data-testid="account-login-link"
                                    on:click=move |_| open.set(false)
                                >
                                    "登录"
                                </a>
                            </Show>
                        }
                    }}
                </div>
            </Show>
        </div>
    }
}
