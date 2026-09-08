use crate::api_base::poc_api_base;
use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use web_sdk::BrowserRestClient;

#[derive(Params, PartialEq, Clone, Debug)]
struct SharedUserParams {
    user_id: Option<String>,
}

#[component]
pub fn SharedUserPage() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let retry = RwSignal::new(0_u64);
    let params = use_params::<SharedUserParams>();
    let user_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.user_id.clone())
            .unwrap_or_default()
    });

    let profile_data = RwSignal::new(None::<serde_json::Value>);
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    Effect::new(move |_| {
        let _ = retry.get();
        let uid = user_id.get();
        if uid.is_empty() {
            return;
        }

        profile_data.set(None);
        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), None);
            let result = client.get_public_user_shares(&uid).await;
            if user_id.try_get_untracked().as_deref() != Some(uid.as_str()) { return; }
            match result {
                Ok(data) => {
                    profile_data.set(Some(data));
                }
                Err(err) => {
                    error.set(Some(format!("{err}")));
                }
            }
            loading.set(false);
        });
    });

    let is_enabled = Signal::derive(move || {
        profile_data.with(|data| {
            data.as_ref()
                .and_then(|d| d.get("profile_enabled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
        })
    });

    view! {
        <div class="shared-user-shell" data-testid="shared-user-page">
            <p role="status" hidden=move || !loading.get()>{move || i18n.t("common.loading")}</p>
            <div hidden=move || error.get().is_none()><p role="alert">{move || error.get()}</p><button type="button" on:click=move |_| retry.update(|n| *n += 1)>{move || i18n.t("common.retry")}</button></div>
            <Show when=move || profile_data.get().is_some() && error.get().is_none()>
            <Show
                when=move || is_enabled.get()
                fallback=move || {
                    view! {
                        <div class="shared-user-disabled" role="alert" data-testid="profile-disabled">
                            <h2>{move || i18n.t("share.profilePrivate")}</h2>
                            <p>{move || i18n.t("share.profilePrivateBody")}</p>
                        </div>
                    }
                }
            >
                <main class="shared-user-card" data-testid="user-profile-card">
                    <header class="shared-user-header">
                        <h1 class="shared-user-name">
                            {move || {
                                profile_data
                                    .get()
                                    .and_then(|d| d.get("display_name").and_then(|v| v.as_str()).map(str::to_string))
                                    .unwrap_or_else(|| i18n.t("share.publicUser"))
                            }}
                        </h1>
                        <p class="shared-user-bio">
                            {move || {
                                profile_data
                                    .get()
                                    .and_then(|d| d.get("bio").and_then(|v| v.as_str()).map(str::to_string))
                                    .unwrap_or_else(|| i18n.t("share.noBio"))
                            }}
                        </p>
                    </header>
                </main>
            </Show>
            </Show>
        </div>
    }
}
