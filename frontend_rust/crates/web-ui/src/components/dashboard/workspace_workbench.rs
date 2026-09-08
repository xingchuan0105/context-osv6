use crate::api_base::poc_api_base;
use crate::components::chat::chat_page::ChatPage;
use crate::components::notes::NoteEditor;
use crate::components::shell::ProductChrome;
use crate::components::ui::{AppDialog, Toaster};
use crate::i18n::{t_now, use_i18n};
#[cfg(target_arch = "wasm32")]
use crate::i18n::tf_now;
use contracts::documents::Document;
use contracts::workspaces::{ChatSession, Workspace, WorkspaceNote};
use leptos::prelude::*;
use leptos_router::hooks::{query_signal, use_navigate, use_params};
use leptos_router::params::Params;
use leptos_router::NavigateOptions;
use web_sdk::{BrowserRestClient, SESSION_FILE_ACCEPT};

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
    let sessions = RwSignal::new(Vec::<ChatSession>::new());
    let show_upload = RwSignal::new(false);
    let upload_tab = RwSignal::new("file".to_string());
    let source_url = RwSignal::new(String::new());
    let paste_title = RwSignal::new(String::new());
    let paste_body = RwSignal::new(String::new());
    let selected_docs = RwSignal::new(std::collections::HashSet::<String>::new());
    let toaster = expect_context::<Toaster>();
    let i18n = use_i18n();
    let navigate = use_navigate();
    let (_session_query, set_session_query) = query_signal::<String>("session");
    let file_input = NodeRef::<leptos::html::Input>::new();

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
            if let Ok(list) = client.list_sessions().await {
                sessions.set(
                    list.sessions
                        .into_iter()
                        .filter(|session| session.workspace_id.as_deref() == Some(wid_clone.as_str()))
                        .collect(),
                );
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
        <ProductChrome footer=false>
        <div class="workspace-workbench-shell" data-testid="workspace-workbench">
            <header class="workbench-top-bar">
                <div class="workbench-top-left">
                    <a href="/dashboard" class="workbench-back">{move || i18n.t("workbench.backAll")}</a>
                    <h2 class="workbench-title">
                        {move || {
                            current_ws
                                .get()
                                .map(|ws| ws.name)
                                .unwrap_or_else(|| {
                                    let id = workspace_id.get();
                                    i18n.tf("workbench.workspaceFallback", &[("id", id.as_str())])
                                })
                        }}
                    </h2>
                </div>
                <div class="workbench-top-actions">
                    <a
                        href=move || format!("/dashboard/{}/share", workspace_id.get())
                        class="workbench-analyze-link"
                        data-testid="goto-analyze"
                    >
                        {move || i18n.t("workbench.shareCenter")}
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
                            {move || i18n.t("workbench.sourcesTab")}
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
                            {move || i18n.t("workbench.notesTab")}
                        </button>
                    </div>

                    <div class="rail-content">
                        <Show when=move || active_tab.get() == "sources">
                            <section class="rail-section" data-testid="sources-section">
                                <div class="rail-section-header">
                                    <span class="rail-section-title">{move || i18n.t("workbench.fileList")}</span>
                                    <span class="rail-section-count">
                                        {move || {
                                            let count = documents.get().len().to_string();
                                            i18n.tf("workbench.docCount", &[("count", count.as_str())])
                                        }}
                                    </span>
                                    <button
                                        type="button"
                                        class="rail-btn-new-note"
                                        data-testid="open-upload"
                                        on:click=move |_| show_upload.set(true)
                                    >
                                        {move || i18n.t("workbench.upload")}
                                    </button>
                                </div>
                                <p class="page-status-empty" data-testid="sources-empty" hidden=move || !documents.get().is_empty()>
                                    {move || i18n.t("workbench.sourcesEmpty")}
                                </p>
                                <ul class="rail-doc-list">
                                    <For
                                        each=move || documents.get()
                                        key=|doc| doc.id.clone()
                                        children=move |doc| {
                                            let doc_id = doc.id.clone();
                                            let select_id = doc.id.clone();
                                            let checked_id = doc.id.clone();
                                            view! {
                                                <li class="rail-doc-item" data-testid="workspace-doc-item" data-status=doc.status.clone()>
                                                    <label class="rail-doc-select">
                                                        <input
                                                            type="checkbox"
                                                            data-testid="doc-select"
                                                            prop:checked=move || selected_docs.get().contains(&checked_id)
                                                            on:change=move |_| {
                                                                selected_docs.update(|set| {
                                                                    if !set.insert(select_id.clone()) {
                                                                        set.remove(&select_id);
                                                                    }
                                                                });
                                                            }
                                                        />
                                                    </label>
                                                    <div class="rail-doc-info">
                                                        <span class="rail-doc-name">{doc.file_name}</span>
                                                        <span class="rail-doc-status">{doc.status.clone()}</span>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="rail-doc-delete"
                                                        data-testid="delete-doc-btn"
                                                        on:click=move |_| on_delete_doc(doc_id.clone())
                                                    >
                                                        {move || i18n.t("dashboardActionDelete")}
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
                                    <span class="rail-section-title">{move || i18n.t("workbench.notesTab")}</span>
                                    <button
                                        type="button"
                                        class="rail-btn-new-note"
                                        data-testid="btn-new-note"
                                        on:click=move |_| is_creating_note.set(true)
                                    >
                                        {move || i18n.t("workbench.newNote")}
                                    </button>
                                </div>

                                <Show when=move || is_creating_note.get()>
                                    <form on:submit=on_create_note class="rail-note-form">
                                        <input
                                            type="text"
                                            placeholder=move || i18n.t("workbench.noteTitlePlaceholder")
                                            data-testid="note-title-input"
                                            prop:value=move || new_note_title.get()
                                            on:input=move |ev| new_note_title.set(event_target_value(&ev))
                                            required
                                        />
                                        <NoteEditor value=new_note_content/>
                                        <div class="rail-note-form-actions">
                                            <button
                                                type="button"
                                                class="dashboard-btn-cancel"
                                                on:click=move |_| is_creating_note.set(false)
                                            >
                                                {move || i18n.t("commonCancel")}
                                            </button>
                                            <button
                                                type="submit"
                                                class="dashboard-btn-confirm"
                                                data-testid="submit-note-btn"
                                            >
                                                {move || i18n.t("workbench.saveNote")}
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
                                                        {move || i18n.t("dashboardActionDelete")}
                                                    </button>
                                                </li>
                                            }
                                        }
                                    />
                                </ul>
                            </section>
                        </Show>
                        <section class="rail-section" data-testid="workspace-sessions">
                            <div class="rail-section-header">
                                <span class="rail-section-title">{move || i18n.t("workbench.sessions")}</span>
                            </div>
                            <p class="page-status-empty" hidden=move || !sessions.get().is_empty()>{move || i18n.t("workbench.sessionsEmpty")}</p>
                            <ul class="rail-note-list">
                                <For
                                    each=move || sessions.get()
                                    key=|session| session.id.clone()
                                    children=move |session| {
                                        let sid = session.id.clone();
                                        let sid_pin = sid.clone();
                                        let sid_del = sid.clone();
                                        let title = session.title.clone().unwrap_or_else(|| i18n.t("chat.untitled"));
                                        let pinned = session.pinned;
                                        view! {
                                            <li class="rail-note-item" data-testid="workspace-session-item">
                                                <button
                                                    type="button"
                                                    class="rail-session-open"
                                                    data-testid="open-workspace-session"
                                                    on:click={
                                                        let navigate = navigate.clone();
                                                        let sid = sid.clone();
                                                        move |_| {
                                                            set_session_query.set(Some(sid.clone()));
                                                            navigate(
                                                                &format!("/dashboard/{}?session={sid}", workspace_id.get_untracked()),
                                                                NavigateOptions { replace: true, ..Default::default() },
                                                            );
                                                        }
                                                    }
                                                >
                                                    {if pinned { "📌 " } else { "" }}{title}
                                                </button>
                                                <button type="button" data-testid="pin-session" on:click=move |_| {
                                                    let tok = current_wb_token(token);
                                                    let sid = sid_pin.clone();
                                                    leptos::task::spawn_local(async move {
                                                        let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                                                        let _ = client.update_session(&sid, None, Some(!pinned)).await;
                                                    });
                                                    refresh_gen.update(|n| *n += 1);
                                                }>{if pinned { i18n.t("workspaceUnpinSessionAction") } else { i18n.t("workspacePinSessionAction") }}</button>
                                                <button type="button" data-testid="delete-session" on:click=move |_| {
                                                    let tok = current_wb_token(token);
                                                    let sid = sid_del.clone();
                                                    leptos::task::spawn_local(async move {
                                                        let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                                                        let _ = client.delete_session(&sid).await;
                                                    });
                                                    refresh_gen.update(|n| *n += 1);
                                                }>{move || i18n.t("dashboardActionDelete")}</button>
                                            </li>
                                        }
                                    }
                                />
                            </ul>
                        </section>
                    </div>
                </aside>
            </div>
            <AppDialog open=Signal::derive(move || show_upload.get()) title_key="workbench.uploadTitle" test_id="upload-dialog" on_close=Callback::new(move |_| show_upload.set(false))>
                <div class="upload-tabs">
                    <button type="button" data-testid="upload-tab-file" on:click=move |_| upload_tab.set("file".into())>{move || i18n.t("workbench.uploadFile")}</button>
                    <button type="button" data-testid="upload-tab-url" on:click=move |_| upload_tab.set("url".into())>{move || i18n.t("workbench.uploadUrl")}</button>
                    <button type="button" data-testid="upload-tab-paste" on:click=move |_| upload_tab.set("paste".into())>{move || i18n.t("workbench.uploadPaste")}</button>
                </div>
                <div hidden=move || upload_tab.get() != "file">
                    <input type="file" data-testid="workspace-file-input" accept=SESSION_FILE_ACCEPT node_ref=file_input/>
                    <button type="button" data-testid="workspace-file-submit" on:click=move |_| {
                        start_workspace_file_upload(token, workspace_id.get_untracked(), file_input, toaster, refresh_gen);
                        show_upload.set(false);
                    }>{move || i18n.t("workbench.startUpload")}</button>
                </div>
                <div hidden=move || upload_tab.get() != "url">
                    <input type="url" data-testid="workspace-url-input" prop:value=move || source_url.get() on:input=move |ev| source_url.set(event_target_value(&ev))/>
                    <button type="button" data-testid="workspace-url-submit" on:click=move |_| {
                        let tok = current_wb_token(token);
                        let wid = workspace_id.get_untracked();
                        let url = source_url.get();
                        leptos::task::spawn_local(async move {
                            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                            if client.add_workspace_source_url(&wid, &url).await.is_ok() {
                                toaster.push(t_now("workbench.linkSubmitted"));
                            }
                        });
                        show_upload.set(false);
                        refresh_gen.update(|n| *n += 1);
                    }>{move || i18n.t("workbench.addLink")}</button>
                </div>
                <div hidden=move || upload_tab.get() != "paste">
                    <input data-testid="workspace-paste-title" prop:value=move || paste_title.get() on:input=move |ev| paste_title.set(event_target_value(&ev))/>
                    <textarea data-testid="workspace-paste-body" prop:value=move || paste_body.get() on:input=move |ev| paste_body.set(event_target_value(&ev))></textarea>
                    <button type="button" data-testid="workspace-paste-submit" on:click=move |_| {
                        let tok = current_wb_token(token);
                        let wid = workspace_id.get_untracked();
                        let title = paste_title.get();
                        let content = paste_body.get();
                        leptos::task::spawn_local(async move {
                            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                            if client.add_workspace_source_paste(&wid, &title, &content).await.is_ok() {
                                toaster.push(t_now("workbench.textSubmitted"));
                            }
                        });
                        show_upload.set(false);
                        refresh_gen.update(|n| *n += 1);
                    }>{move || i18n.t("workbench.addText")}</button>
                </div>
            </AppDialog>
        </div>
        </ProductChrome>
    }
}

fn current_wb_token(token: RwSignal<String>) -> String {
    if token.get_untracked().is_empty() {
        web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
    } else {
        token.get_untracked()
    }
}

fn start_workspace_file_upload(
    token: RwSignal<String>,
    workspace_id: String,
    file_input: NodeRef<leptos::html::Input>,
    toaster: Toaster,
    refresh_gen: RwSignal<u64>,
) {
    let tok = current_wb_token(token);
    if tok.is_empty() || workspace_id.is_empty() {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let Some(input) = file_input.get() else {
            return;
        };
        let Some(files) = input.files() else {
            return;
        };
        let Some(file) = files.item(0) else {
            return;
        };
        let name = file.name();
        let size = file.size() as u64;
        let mime = if file.type_().is_empty() {
            "application/octet-stream".to_string()
        } else {
            file.type_()
        };
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client
                .create_workspace_document_upload(&workspace_id, &name, size, &mime)
                .await
            {
                Ok(resp) => {
                    let bytes = gloo_file_bytes(&file).await;
                    if let Ok(bytes) = bytes {
                        let _ = client.put_upload_bytes(&resp.upload_url, &bytes, &mime).await;
                        let _ = client.complete_upload(&resp.document_id).await;
                        toaster.push(t_now("workbench.uploaded"));
                    }
                }
                Err(err) => toaster.push(tf_now("workbench.uploadFailed", &[("error", &err.to_string())])),
            }
            refresh_gen.update(|n| *n += 1);
        });
    }
    let _ = (file_input, toaster, refresh_gen);
}

#[cfg(target_arch = "wasm32")]
async fn gloo_file_bytes(file: &web_sys::File) -> Result<Vec<u8>, ()> {
    use wasm_bindgen_futures::JsFuture;
    let promise = file.array_buffer();
    let buffer = JsFuture::from(promise).await.map_err(|_| ())?;
    let array = js_sys::Uint8Array::new(&buffer);
    Ok(array.to_vec())
}
