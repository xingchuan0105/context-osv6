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
        let uid = user_id.get();
        if uid.is_empty() {
            return;
        }

        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), None);
            match client.get_public_user_shares(&uid).await {
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
            <Show
                when=move || is_enabled.get()
                fallback=move || {
                    view! {
                        <div class="shared-user-disabled" role="alert" data-testid="profile-disabled">
                            <h2>"主页未公开"</h2>
                            <p>"该用户尚未开启公开主页展示。"</p>
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
                                    .unwrap_or_else(|| "公开用户".to_string())
                            }}
                        </h1>
                        <p class="shared-user-bio">
                            {move || {
                                profile_data
                                    .get()
                                    .and_then(|d| d.get("bio").and_then(|v| v.as_str()).map(str::to_string))
                                    .unwrap_or_else(|| "暂无个人简介".to_string())
                            }}
                        </p>
                    </header>
                </main>
            </Show>
        </div>
    }
}
