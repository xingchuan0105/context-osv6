use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::params::Params;
use leptos_router::NavigateOptions;

#[derive(Params, PartialEq, Clone, Debug)]
struct InviteParams {
    workspace_id: Option<String>,
    member_id: Option<String>,
}

#[component]
pub fn InvitePage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<InviteParams>();
    let navigate = use_navigate();

    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    view! {
        <div class="auth-page-container" data-testid="invite-page">
            <main class="auth-card" aria-label="工作区成员邀请">
                <header class="auth-header">
                    <h1 class="auth-title">"工作区加入邀请"</h1>
                    <p class="auth-subtitle">
                        {move || format!("您受邀加入工作区 {}", workspace_id.get())}
                    </p>
                </header>
                <div class="invite-action-box">
                    {move || {
                        if !token.get().is_empty() {
                            let navigate = navigate.clone();
                            view! {
                                <button
                                    type="button"
                                    class="auth-submit-btn"
                                    data-testid="accept-invite-btn"
                                    on:click=move |_| {
                                        let wid = workspace_id.get();
                                        navigate(&format!("/dashboard/{wid}"), NavigateOptions::default());
                                    }
                                >
                                    "立即接受并进入工作台"
                                </button>
                            }
                            .into_any()
                        } else {
                            let wid = workspace_id.get();
                            let login_href = format!("/login?next=/dashboard/{wid}");
                            view! {
                                <div class="invite-guest-prompt">
                                    <p>"您当前尚未登录，请先登录后接受邀请。"</p>
                                    <a href=login_href class="auth-submit-btn" data-testid="invite-login-btn">
                                        "登录并接受邀请"
                                    </a>
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </div>
            </main>
        </div>
    }
}
