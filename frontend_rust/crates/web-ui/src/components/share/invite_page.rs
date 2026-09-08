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
    let i18n = crate::i18n::use_i18n();
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<InviteParams>();
    let navigate = use_navigate();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let member_id = Signal::derive(move || params.read().as_ref().ok()
        .and_then(|p| p.member_id.clone()).unwrap_or_default());

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
            <main class="auth-card" aria-label=move || i18n.t("invite.title")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("invite.title")}</h1>
                    <p class="auth-subtitle">
                        {move || i18n.tf("invite.description", &[("id", &workspace_id.get())])}
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
                                    disabled=move || busy.get()
                                    on:click=move |_| {
                                        if busy.get_untracked() { return; }
                                        let wid = workspace_id.get_untracked();
                                        let mid = member_id.get_untracked();
                                        if wid.is_empty() || mid.is_empty() { return; }
                                        let tok = token.get_untracked();
                                        let navigate = navigate.clone();
                                        busy.set(true);
                                        error.set(None);
                                        leptos::task::spawn_local(async move {
                                            let client = web_sdk::BrowserRestClient::new(&crate::api_base::poc_api_base(), Some(tok));
                                            match client.accept_workspace_invite(&wid, &mid).await {
                                                Ok(()) => navigate(&format!("/dashboard/{wid}"), NavigateOptions::default()),
                                                Err(err) => error.set(Some(i18n.tf("invite.failed", &[("error", &err.to_string())]))),
                                            }
                                            busy.set(false);
                                        });
                                    }
                                >
                                    {move || i18n.t("invite.accept")}
                                </button>
                            }
                            .into_any()
                        } else {
                            let wid = workspace_id.get();
                            let mid = member_id.get();
                            let login_href = format!("/login?next=/invite/{wid}/{mid}");
                            view! {
                                <div class="invite-guest-prompt">
                                    <p>{move || i18n.t("invite.loginHint")}</p>
                                    <a href=login_href class="auth-submit-btn" data-testid="invite-login-btn">
                                        {move || i18n.t("invite.login")}
                                    </a>
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </div>
                {move || error.get().map(|message| view! { <p role="alert" class="settings-error" data-testid="invite-error">{message}</p> })}
            </main>
        </div>
    }
}
