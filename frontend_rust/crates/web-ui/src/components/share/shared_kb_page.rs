use crate::api_base::poc_api_base;
use crate::components::chat::chat_page::ChatPage;
use contracts::share::SharedWorkspacePayload;
use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use web_sdk::BrowserRestClient;

#[derive(Params, PartialEq, Clone, Debug)]
struct SharedKbParams {
    token: Option<String>,
}

#[component]
pub fn SharedKbPage() -> impl IntoView {
    let params = use_params::<SharedKbParams>();
    let token = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.token.clone())
            .unwrap_or_default()
    });

    let payload = RwSignal::new(None::<SharedWorkspacePayload>);
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    Effect::new(move |_| {
        let tok = token.get();
        if tok.is_empty() {
            return;
        }

        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), None);
            match client.get_shared_workspace(&tok).await {
                Ok(data) => {
                    payload.set(Some(data));
                }
                Err(err) => {
                    error.set(Some(format!("{err}")));
                }
            }
            loading.set(false);
        });
    });

    view! {
        <div class="shared-kb-shell" data-testid="shared-kb-page">
            <Show when=move || error.get().is_some()>
                <div class="shared-kb-error-box" role="alert" data-testid="share-expired">
                    <h2>"分享不可用"</h2>
                    <p>"该知识库分享链接已失效或不存在。"</p>
                    <a href="/login" class="dashboard-chat-link">"返回首页登录"</a>
                </div>
            </Show>

            <Show when=move || payload.get().is_some()>
                {move || {
                    payload.get().map(|data| {
                        let kb = data.knowledge_base;
                        let owner = data.owner;
                        let sources = data.sources;
                        view! {
                            <header class="shared-kb-header" data-testid="shared-kb-header">
                                <div class="shared-kb-meta">
                                    <span class="shared-kb-badge">"公开知识库"</span>
                                    <h1 class="shared-kb-title">{kb.title}</h1>
                                    <p class="shared-kb-desc">{kb.description.unwrap_or_default()}</p>
                                </div>
                                {owner.map(|o| {
                                    view! {
                                        <div class="shared-owner-card" data-testid="owner-card">
                                            <span class="shared-owner-label">"分享者"</span>
                                            <span class="shared-owner-name">{o.display_name}</span>
                                            {o.bio.map(|bio| view! { <p class="shared-owner-bio">{bio}</p> })}
                                        </div>
                                    }
                                })}
                            </header>

                            <div class="shared-kb-body">
                                <aside class="shared-kb-sources">
                                    <h3>"包含资料 (只读)"</h3>
                                    <ul class="shared-source-list">
                                        {sources.into_iter().map(|s| {
                                            view! {
                                                <li class="shared-source-item" data-testid="shared-source-item">
                                                    <span class="shared-source-name">{s.file_name}</span>
                                                </li>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </ul>
                                </aside>

                                <div class="shared-kb-chat">
                                    <ChatPage/>
                                </div>
                            </div>
                        }
                    })
                }}
            </Show>
        </div>
    }
}
