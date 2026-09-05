use crate::api_base::poc_api_base;
use crate::components::chat::chat_page::ChatPage;
use contracts::documents::Document;
use contracts::workspaces::{Workspace, WorkspaceNote};
use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use web_sdk::BrowserRestClient;

#[derive(Params, PartialEq, Clone, Debug)]
struct WorkbenchParams {
    workspace_id: Option<String>,
}

#[component]
pub fn WorkspaceWorkbenchPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<WorkbenchParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    let current_ws = RwSignal::new(None::<Workspace>);
    let documents = RwSignal::new(Vec::<Document>::new());
    let notes = RwSignal::new(Vec::<WorkspaceNote>::new());
    let active_tab = RwSignal::new("sources"); // "sources" | "notes"
    let new_note_title = RwSignal::new(String::new());
    let new_note_content = RwSignal::new(String::new());
    let is_creating_note = RwSignal::new(false);
    let refresh_gen = RwSignal::new(0_u64);

    Effect::new(move |_| {
        let wid = workspace_id.get();
        if wid.is_empty() {
            return;
        }
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() {
            return;
        }

        let _ = refresh_gen.get();
        let wid_clone = wid.clone();
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if let Ok(resp) = client.get_workspace(&wid_clone).await {
                current_ws.set(Some(resp.workspace));
            }
            if let Ok(resp) = client.list_workspace_documents(&wid_clone).await {
                documents.set(resp.documents);
            }
            if let Ok(resp) = client.list_workspace_notes(&wid_clone).await {
                notes.set(resp.notes);
            }
        });
    });

    let on_delete_doc = move |doc_id: String| {
        let wid = workspace_id.get_untracked();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() {
            return;
        }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if client.delete_workspace_document(&wid, &doc_id).await.is_ok() {
                documents.update(|list| list.retain(|d| d.id != doc_id));
            }
        });
    };

    let on_create_note = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let title = new_note_title.get().trim().to_string();
        let content = new_note_content.get().trim().to_string();
        if title.is_empty() {
            return;
        }
        let wid = workspace_id.get_untracked();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() {
            return;
        }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if let Ok(resp) = client.create_workspace_note(&wid, &title, &content).await {
                notes.update(|list| list.insert(0, resp.note));
                new_note_title.set(String::new());
                new_note_content.set(String::new());
                is_creating_note.set(false);
            }
        });
    };

    let on_delete_note = move |note_id: String| {
        let wid = workspace_id.get_untracked();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() {
            return;
        }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if client.delete_workspace_note(&wid, &note_id).await.is_ok() {
                notes.update(|list| list.retain(|n| n.id != note_id));
            }
        });
    };

    view! {
        <div class="workspace-workbench-shell" data-testid="workspace-workbench">
            <header class="workbench-top-bar">
                <div class="workbench-top-left">
                    <a href="/dashboard" class="workbench-back">"← 全部工作区"</a>
                    <h2 class="workbench-title">
                        {move || {
                            current_ws
                                .get()
                                .map(|ws| ws.name)
                                .unwrap_or_else(|| format!("工作区：{}", workspace_id.get()))
                        }}
                    </h2>
                </div>
                <div class="workbench-top-actions">
                    <a
                        href=move || format!("/dashboard/{}/analyze", workspace_id.get())
                        class="workbench-analyze-link"
                        data-testid="goto-analyze"
                    >
                        "资料分析与切片 →"
                    </a>
                </div>
            </header>

            <div class="workbench-main-body">
                <div class="workbench-chat-column">
                    <ChatPage/>
                </div>

                <aside class="workbench-side-rail" data-testid="workspace-side-rail">
                    <div class="rail-tabs">
                        <button
                            type="button"
                            class=move || {
                                if active_tab.get() == "sources" {
                                    "rail-tab is-active"
                                } else {
                                    "rail-tab"
                                }
                            }
                            data-testid="tab-sources"
                            on:click=move |_| active_tab.set("sources")
                        >
                            "持久资料库"
                        </button>
                        <button
                            type="button"
                            class=move || {
                                if active_tab.get() == "notes" {
                                    "rail-tab is-active"
                                } else {
                                    "rail-tab"
                                }
                            }
                            data-testid="tab-notes"
                            on:click=move |_| active_tab.set("notes")
                        >
                            "工作区笔记"
                        </button>
                    </div>

                    <div class="rail-content">
                        <Show when=move || active_tab.get() == "sources">
                            <section class="rail-section" data-testid="sources-section">
                                <div class="rail-section-header">
                                    <span class="rail-section-title">"持久文件列表"</span>
                                    <span class="rail-section-count">
                                        {move || format!("{} 篇", documents.get().len())}
                                    </span>
                                </div>
                                <ul class="rail-doc-list">
                                    <For
                                        each=move || documents.get()
                                        key=|doc| doc.id.clone()
                                        children=move |doc| {
                                            let doc_id = doc.id.clone();
                                            view! {
                                                <li class="rail-doc-item" data-testid="workspace-doc-item">
                                                    <div class="rail-doc-info">
                                                        <span class="rail-doc-name">{doc.file_name}</span>
                                                        <span class="rail-doc-status">{doc.status}</span>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="rail-doc-delete"
                                                        data-testid="delete-doc-btn"
                                                        on:click=move |_| on_delete_doc(doc_id.clone())
                                                    >
                                                        "删除"
                                                    </button>
                                                </li>
                                            }
                                        }
                                    />
                                </ul>
                            </section>
                        </Show>

                        <Show when=move || active_tab.get() == "notes">
                            <section class="rail-section" data-testid="notes-section">
                                <div class="rail-section-header">
                                    <span class="rail-section-title">"工作区笔记"</span>
                                    <button
                                        type="button"
                                        class="rail-btn-new-note"
                                        data-testid="btn-new-note"
                                        on:click=move |_| is_creating_note.set(true)
                                    >
                                        "+ 新笔记"
                                    </button>
                                </div>

                                <Show when=move || is_creating_note.get()>
                                    <form on:submit=on_create_note class="rail-note-form">
                                        <input
                                            type="text"
                                            placeholder="笔记标题"
                                            data-testid="note-title-input"
                                            prop:value=move || new_note_title.get()
                                            on:input=move |ev| new_note_title.set(event_target_value(&ev))
                                            required
                                        />
                                        <textarea
                                            placeholder="输入 Markdown 笔记内容…"
                                            rows=4
                                            data-testid="note-content-input"
                                            prop:value=move || new_note_content.get()
                                            on:input=move |ev| new_note_content.set(event_target_value(&ev))
                                        ></textarea>
                                        <div class="rail-note-form-actions">
                                            <button
                                                type="button"
                                                class="dashboard-btn-cancel"
                                                on:click=move |_| is_creating_note.set(false)
                                            >
                                                "取消"
                                            </button>
                                            <button
                                                type="submit"
                                                class="dashboard-btn-confirm"
                                                data-testid="submit-note-btn"
                                            >
                                                "保存笔记"
                                            </button>
                                        </div>
                                    </form>
                                </Show>

                                <ul class="rail-note-list">
                                    <For
                                        each=move || notes.get()
                                        key=|note| note.id.clone()
                                        children=move |note| {
                                            let note_id = note.id.clone();
                                            view! {
                                                <li class="rail-note-item" data-testid="workspace-note-item">
                                                    <div class="rail-note-info">
                                                        <span class="rail-note-title">{note.title}</span>
                                                        <p class="rail-note-preview">{note.preview}</p>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="rail-note-delete"
                                                        data-testid="delete-note-btn"
                                                        on:click=move |_| on_delete_note(note_id.clone())
                                                    >
                                                        "删除"
                                                    </button>
                                                </li>
                                            }
                                        }
                                    />
                                </ul>
                            </section>
                        </Show>
                    </div>
                </aside>
            </div>
        </div>
    }
}
