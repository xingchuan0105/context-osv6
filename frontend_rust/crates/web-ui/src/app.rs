//! Root application component and shell layout

use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local;
use leptos_meta::provide_meta_context;
use leptos_router::components::{A, Redirect, Route, Router, Routes};
use leptos_router::path;

#[cfg(target_arch = "wasm32")]
use leptos_router::NavigateOptions;
#[cfg(target_arch = "wasm32")]
use leptos_router::hooks::{use_location, use_navigate};

use crate::i18n::choose;
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::{ConfirmResetPage, LoginPage, RegisterPage, ResetPasswordPage, VerifyResetPage};

#[cfg(target_arch = "wasm32")]
fn is_public_path(path: &str) -> bool {
    path == "/"
        || path == "/login"
        || path == "/register"
        || path.starts_with("/reset-password")
        || path.starts_with("/preview")
        || path.starts_with("/shared/kb/")
}

#[component]
fn AuthNavigationGuard() -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    {
        let auth = crate::state::auth::use_auth_state();
        let location = use_location();
        let navigate = use_navigate();
        let (bootstrap_done, set_bootstrap_done) = signal(false);

        Effect::new(move |_| {
            let path = location.pathname.get();
            let token = auth.token.get();

            if path.starts_with("/preview") {
                return;
            }

            if !bootstrap_done.get() {
                if let Some(token_value) = token.clone() {
                    set_bootstrap_done.set(true);
                    let auth = auth.clone();
                    let navigate = navigate.clone();
                    let path_for_async = path.clone();
                    spawn_local(async move {
                        let client =
                            web_sdk::ApiClient::new(String::new()).with_auth(token_value.clone());
                        match client.me().await {
                            Ok(resp) if resp.success => {
                                if let Some(data) = resp.data {
                                    auth.set_auth(token_value, data.user);
                                }
                            }
                            _ => {
                                auth.logout();
                                if !is_public_path(&path_for_async) {
                                    navigate("/login", NavigateOptions::default());
                                }
                            }
                        }
                    });
                } else {
                    set_bootstrap_done.set(true);
                }
            }

            if bootstrap_done.get() {
                let has_token = auth.token.get().is_some();
                if !has_token && !is_public_path(&path) {
                    navigate("/login", NavigateOptions::default());
                }
                if has_token && (path == "/login" || path == "/register") {
                    navigate("/dashboard", NavigateOptions::default());
                }
            }
        });
    }

    view! {}
}

/// Not found fallback page
#[component]
fn NotFound() -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    view! {
        <div class="app-auth-shell">
            <div class="app-surface-card w-full max-w-md text-center">
                <h1 class="text-4xl font-bold text-foreground mb-4">{"404"}</h1>
                <p class="text-muted-foreground mb-6">
                    {move || choose(locale.get(), "页面不存在", "Page not found")}
                </p>
                <A href="/dashboard" attr:class="app-button-primary">
                    {move || choose(locale.get(), "返回工作台", "Go to dashboard")}
                </A>
            </div>
        </div>
    }
}

#[component]
pub fn Shell() -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <script type="module">
                    {r#"
                        const knownMarkers = [
                            '/preview/live',
                            '/preview',
                            '/dashboard',
                            '/settings',
                            '/help',
                            '/login',
                            '/register',
                            '/reset-password',
                            '/shared/kb',
                            '/invite',
                            '/admin'
                        ];
                        const pathname = window.location.pathname || '/';
                        let pathPrefix = '';
                        for (const marker of knownMarkers) {
                            const idx = pathname.indexOf(marker);
                            if (idx > 0) {
                                pathPrefix = pathname.slice(0, idx).replace(/\/+$/, '');
                                break;
                            }
                            if (idx === 0) {
                                pathPrefix = '';
                                break;
                            }
                        }
                        const withPrefix = (assetPath) => `${pathPrefix}${assetPath}`;

                        if (pathname.endsWith('/') && pathname !== '/') {
                            const trimmed = pathname.replace(/\/+$/, '');
                            if (trimmed === '/preview/live' || trimmed.startsWith('/preview/live/')) {
                                window.history.replaceState({}, '', `${trimmed}${window.location.search}${window.location.hash}`);
                            }
                        }

                        const cssLink = document.createElement('link');
                        cssLink.rel = 'stylesheet';
                        cssLink.href = withPrefix('/pkg/index.css');
                        document.head.appendChild(cssLink);

                        const { default: init, hydrate } = await import(withPrefix('/pkg/web_ui.js'));
                        await init({ module_or_path: withPrefix('/pkg/web_ui_bg.wasm') });
                        hydrate();
                    "#}
                </script>
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

/// Root `<App/>` component — provides the top-level shell with navigation.
/// This is the main app structure that includes the router and all routes.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    let _ui_prefs = crate::state::ui_prefs::provide_ui_prefs_state();
    let _auth = crate::state::auth::provide_auth_state();

    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        if let Some(document) = web_sys::window().and_then(|window| window.document())
            && let Some(root) = document.document_element()
        {
            let _ = root.set_attribute("data-hydrated", "true");
        }
    });

    view! {
        <Router>
            <AuthNavigationGuard />
            <Routes fallback={|| view! { <NotFound /> }}>
                <Route path={path!("/")} view={crate::HomePage} />
                <Route path={path!("/login")} view={LoginPage} />
                <Route path={path!("/register")} view={RegisterPage} />
                <Route path={path!("/reset-password")} view={ResetPasswordPage} />
                <Route path={path!("/reset-password/verify")} view={VerifyResetPage} />
                <Route path={path!("/reset-password/confirm")} view={ConfirmResetPage} />
                <Route path={path!("/preview")} view={crate::PreviewEntryPage} />
                <Route path={path!("/preview/login")} view={crate::PreviewLoginPage} />
                <Route path={path!("/preview/dashboard")} view={crate::PreviewDashboardPage} />
                <Route path={path!("/preview/workspace")} view={crate::PreviewWorkspacePage} />
                <Route path={path!("/preview/account")} view={crate::PreviewAccountPage} />
                <Route path={path!("/preview/settings")} view={crate::PreviewSettingsPage} />
                <Route path={path!("/preview/help")} view={crate::PreviewHelpPage} />
                <Route path={path!("/preview/live")} view=|| view! { <Redirect path="/preview/live/dashboard" /> } />
                <Route path={path!("/preview/live/login")} view={crate::LoginPage} />
                <Route path={path!("/preview/live/dashboard")} view={crate::DashboardListPage} />
                <Route path={path!("/preview/live/help")} view={crate::HelpPage} />
                <Route path={path!("/preview/live/settings")} view={crate::SettingsPage} />
                <Route
                    path={path!("/preview/live/workspace/:notebook_id")}
                    view={crate::WorkspacePage}
                />
                <Route
                    path={path!("/preview/live/workspace/:notebook_id/api-access")}
                    view={crate::ApiAccessPage}
                />
                <Route
                    path={path!("/preview/live/workspace/:notebook_id/share")}
                    view={crate::ShareCenterPage}
                />

                <Route path={path!("/dashboard")} view={crate::DashboardListPage} />
                <Route path={path!("/dashboard/search")} view={crate::SearchPage} />
                <Route
                    path={path!("/dashboard/:notebook_id")}
                    view={crate::WorkspacePage}
                />
                <Route
                    path={path!("/dashboard/:notebook_id/api-access")}
                    view={crate::ApiAccessPage}
                />
                <Route path={path!("/help")} view={crate::HelpPage} />

                <Route path={path!("/shared/kb/:token")} view={crate::SharedKbPage} />
                <Route
                    path={path!("/invite/:notebook_id/:member_id")}
                    view={crate::InvitePage}
                />
                <Route path={path!("/settings")} view={crate::SettingsPage} />

                <Route
                    path={path!("/dashboard/:notebook_id/share")}
                    view={crate::ShareCenterPage}
                />
                <Route
                    path={path!("/dashboard/:notebook_id/share/analytics")}
                    view={crate::ShareCenterPage}
                />
                <Route
                    path={path!("/dashboard/:notebook_id/share/access-logs")}
                    view={crate::ShareCenterPage}
                />
                <Route
                    path={path!("/notebooks/:notebook_id/share")}
                    view={crate::ShareCenterPage}
                />
                <Route
                    path={path!("/notebooks/:notebook_id/share/analytics")}
                    view={crate::ShareCenterPage}
                />
                <Route
                    path={path!("/notebooks/:notebook_id/share/access-logs")}
                    view={crate::ShareCenterPage}
                />

                <Route path={path!("/admin")} view=|| view! { <crate::AdminShell><crate::OrganizationsPage /></crate::AdminShell> } />
                <Route path={path!("/admin/organizations")} view=|| view! { <crate::AdminShell><crate::OrganizationsPage /></crate::AdminShell> } />
                <Route path={path!("/admin/users")} view=|| view! { <crate::AdminShell><crate::UsersPage /></crate::AdminShell> } />
                <Route path={path!("/admin/usage")} view=|| view! { <crate::AdminShell><crate::UsagePage /></crate::AdminShell> } />
                <Route path={path!("/admin/billing")} view=|| view! { <crate::AdminShell><crate::BillingPage /></crate::AdminShell> } />
                <Route path={path!("/admin/health")} view=|| view! { <crate::AdminShell><crate::HealthPage /></crate::AdminShell> } />
                <Route path={path!("/admin/rag-health")} view=|| view! { <crate::AdminShell><crate::RagHealthPage /></crate::AdminShell> } />
                <Route path={path!("/admin/feature-flags")} view=|| view! { <crate::AdminShell><crate::FeatureFlagsPage /></crate::AdminShell> } />
                <Route path={path!("/admin/system/workers")} view=|| view! { <crate::AdminShell><crate::WorkerStatusPage /></crate::AdminShell> } />
                <Route path={path!("/admin/system/degradation")} view=|| view! { <crate::AdminShell><crate::DegradationPage /></crate::AdminShell> } />
                <Route path={path!("/admin/audit-logs")} view=|| view! { <crate::AdminShell><crate::AuditLogsPage /></crate::AdminShell> } />
                <Route path={path!("/admin/organizations/:org_id")} view=|| view! { <crate::AdminShell><crate::OrgDetailPage /></crate::AdminShell> } />
                <Route path={path!("/admin/orgs/:org_id")} view=|| view! { <crate::AdminShell><crate::OrgDetailPage /></crate::AdminShell> } />
            </Routes>
        </Router>
    }
}
