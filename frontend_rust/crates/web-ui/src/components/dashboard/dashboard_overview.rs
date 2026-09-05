use crate::api_base::poc_api_base;
use contracts::workspaces::Workspace;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use web_sdk::BrowserRestClient;

#[component]
pub fn DashboardOverviewPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let navigate = use_navigate();

    let workspaces = RwSignal::new(Vec::<Workspace>::new());
    let loading = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let show_create = RwSignal::new(false);
    let new_name = RwSignal::new(String::new());
    let new_desc = RwSignal::new(String::new());
    let create_loading = RwSignal::new(false);

    let load_workspaces = move || {
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() {
            return;
        }

        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.list_workspaces().await {
                Ok(resp) => {
                    workspaces.set(resp.workspaces);
                }
                Err(err) => {
                    error.set(Some(format!("加载工作区失败：{err}")));
                }
            }
            loading.set(false);
        });
    };

    Effect::new(move |_| {
        load_workspaces();
    });

    let on_create = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let name_val = new_name.get().trim().to_string();
            let desc_val = new_desc.get().trim().to_string();
            if name_val.is_empty() {
                return;
            }
            let tok = if token.get_untracked().is_empty() {
                web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
            } else {
                token.get_untracked()
            };
            if tok.is_empty() || create_loading.get() {
                return;
            }

            create_loading.set(true);
            let navigate = navigate.clone();
            leptos::task::spawn_local(async move {
                let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                match client.create_workspace(&name_val, &desc_val).await {
                    Ok(resp) => {
                        let wid = resp.workspace.id;
                        navigate(&format!("/dashboard/{wid}"), NavigateOptions::default());
                    }
                    Err(err) => {
                        error.set(Some(format!("创建工作区失败：{err}")));
                    }
                }
                create_loading.set(false);
            });
        }
    };

    view! {
        <div class="dashboard-shell" data-testid="dashboard-overview">
            <header class="dashboard-header">
                <div class="dashboard-header-left">
                    <a href="/chat" class="dashboard-chat-link">"← 返回个人对话"</a>
                    <h1 class="dashboard-title">"工作区与持久知识库"</h1>
                </div>
                <div class="dashboard-header-actions">
                    <a href="/dashboard/analytics" class="dashboard-header-btn">"分享流量分析"</a>
                    <button
                        type="button"
                        class="dashboard-create-btn"
                        data-testid="create-workspace-btn"
                        on:click=move |_| show_create.set(true)
                    >
                        "+ 新建工作区"
                    </button>
                </div>
            </header>

            <div
                class="dashboard-modal-backdrop"
                style=move || if show_create.get() { "" } else { "display: none;" }
            >
                <div class="dashboard-modal" role="dialog" aria-label="新建工作区">
                    <header class="dashboard-modal-header">
                        <h2>"新建工作区"</h2>
                        <button
                            type="button"
                            class="dashboard-modal-close"
                            on:click=move |_| show_create.set(false)
                        >
                            "×"
                        </button>
                    </header>
                    <form on:submit=on_create class="dashboard-modal-form">
                        <div class="auth-field">
                            <label for="ws-name">"工作区名称"</label>
                            <input
                                id="ws-name"
                                type="text"
                                data-testid="new-workspace-name"
                                placeholder="例如：材料研发知识库"
                                prop:value=move || new_name.get()
                                on:input=move |ev| new_name.set(event_target_value(&ev))
                                required
                            />
                        </div>
                        <div class="auth-field">
                            <label for="ws-desc">"描述 (可选)"</label>
                            <input
                                id="ws-desc"
                                type="text"
                                data-testid="new-workspace-desc"
                                placeholder="说明知识库范畴"
                                prop:value=move || new_desc.get()
                                on:input=move |ev| new_desc.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="dashboard-modal-actions">
                            <button
                                type="button"
                                class="dashboard-btn-cancel"
                                on:click=move |_| show_create.set(false)
                            >
                                "取消"
                            </button>
                            <button
                                type="submit"
                                class="dashboard-btn-confirm"
                                data-testid="submit-create-workspace"
                                disabled=move || create_loading.get()
                            >
                                {move || if create_loading.get() { "创建中…" } else { "确认创建" }}
                            </button>
                        </div>
                    </form>
                </div>
            </div>

            <main class="dashboard-content">
                {move || {
                    error.get().map(|msg| {
                        view! {
                            <p class="dashboard-error" role="alert">
                                {msg}
                            </p>
                        }
                    })
                }}
                <Show when=move || loading.get()>
                    <p class="dashboard-loading">"正在加载工作区列表…"</p>
                </Show>
                <div class="dashboard-workspace-grid" data-testid="workspace-grid">
                    <For
                        each=move || workspaces.get()
                        key=|ws| ws.id.clone()
                        children=move |ws| {
                            let wid = ws.id.clone();
                            let href = format!("/dashboard/{wid}");
                            let desc = if ws.description.is_empty() {
                                "暂无描述".to_string()
                            } else {
                                ws.description.clone()
                            };
                            view! {
                                <a href=href class="dashboard-workspace-card" data-testid="workspace-card">
                                    <div class="dashboard-card-top">
                                        <h3 class="dashboard-card-title">{ws.name}</h3>
                                        <span class="dashboard-card-docs">
                                            {format!("{} 篇资料", ws.document_count)}
                                        </span>
                                    </div>
                                    <p class="dashboard-card-desc">
                                        {desc}
                                    </p>
                                </a>
                            }
                        }
                    />
                </div>
            </main>
        </div>
    }
}
