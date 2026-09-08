use crate::api_base::poc_api_base;
use crate::components::chat::chat_page::ChatPage;
use crate::components::notes::NoteEditor;
use crate::components::shell::ProductChrome;
use crate::components::ui::{AppDialog, Toaster};
use crate::i18n::{t_now, use_i18n};
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

    let busy = RwSignal::new(false);
    let loading = RwSignal::new(true);
    let load_error = RwSignal::new(None::<String>);
    let action_error = RwSignal::new(None::<String>);
    let previous_workspace = RwSignal::new(String::new());
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
    let (session_query, set_session_query) = query_signal::<String>("session");
    let (source_query, set_source_query) = query_signal::<String>("source");
    let preview = RwSignal::new(None::<String>);
    let preview_error = RwSignal::new(None::<String>);
    let preview_loading = RwSignal::new(false);
    let preview_refresh = RwSignal::new(0_u64);
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
            loading.set(false);
            load_error.set(Some(i18n.t("pricing.loginRequired")));
            return;
        }

        if previous_workspace.get_untracked() != wid {
            refresh_gen.update(|n| *n += 1);
            previous_workspace.set(wid.clone());
            current_ws.set(None);
            documents.set(Vec::new()); notes.set(Vec::new()); sessions.set(Vec::new());
            selected_docs.set(Default::default());
            busy.set(false); action_error.set(None); show_upload.set(false);
        }
        let generation = refresh_gen.get();
        loading.set(true); load_error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let result = async {
                let workspace = client.get_workspace(&wid).await?.workspace;
                let docs = client.list_workspace_documents(&wid).await?.documents;
                let ws_notes = client.list_workspace_notes(&wid).await?.notes;
                let ws_sessions = client.list_sessions().await?.sessions.into_iter()
                    .filter(|session| session.workspace_id.as_deref() == Some(wid.as_str())).collect::<Vec<_>>();
                Ok::<_, web_sdk::TransportError>((workspace, docs, ws_notes, ws_sessions))
            }.await;
            if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str())
                || refresh_gen.try_get_untracked() != Some(generation) { return; }
            match result {
                Ok((workspace, docs, ws_notes, ws_sessions)) => {
                    selected_docs.update(|ids| ids.retain(|id| docs.iter().any(|doc| &doc.id == id && doc.status == "completed")));
                    current_ws.set(Some(workspace)); documents.set(docs);
                    notes.set(ws_notes); sessions.set(ws_sessions);
                }
                Err(err) => load_error.set(Some(err.to_string())),
            }
            loading.set(false);
        });
    });

    Effect::new(move |_| {
        let wid = workspace_id.get();
        let source = source_query.get();
        let generation = preview_refresh.get();
        let docs = documents.get();
        if loading.get() { return; }
        preview.set(None); preview_error.set(None); preview_loading.set(false);
        let Some(id) = source else { return; };
        if !docs.iter().any(|doc| doc.id == id) {
            preview_error.set(Some(i18n.t("workbench.sourceUnavailable"))); return;
        }
        preview_loading.set(true);
        let tok = current_wb_token(token);
        leptos::task::spawn_local(async move {
            let result = BrowserRestClient::new(&poc_api_base(), Some(tok)).get_document_content(&id).await;
            if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str())
                || source_query.try_get_untracked().flatten().as_deref() != Some(id.as_str())
                || preview_refresh.try_get_untracked() != Some(generation) { return; }
            match result {
                Ok(data) => preview.set(Some(data.content)),
                Err(err) => preview_error.set(Some(err.to_string())),
            }
            preview_loading.set(false);
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

        run_workbench_action(workspace_id, busy, action_error, refresh_gen, async move {
            BrowserRestClient::new(&poc_api_base(), Some(tok)).delete_workspace_document(&wid, &doc_id).await
        }, Callback::new(move |_| ()));

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

        run_workbench_action(workspace_id, busy, action_error, refresh_gen, async move {
            BrowserRestClient::new(&poc_api_base(), Some(tok)).create_workspace_note(&wid, &title, &content).await.map(|_| ())
        }, Callback::new(move |_| {
            new_note_title.set(String::new()); new_note_content.set(String::new()); is_creating_note.set(false);
        }));

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

        run_workbench_action(workspace_id, busy, action_error, refresh_gen, async move {
            BrowserRestClient::new(&poc_api_base(), Some(tok)).delete_workspace_note(&wid, &note_id).await
        }, Callback::new(move |_| ()));

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

            <p role="status" hidden=move || !loading.get()>{move || i18n.t("common.loading")}</p>
            <div hidden=move || load_error.get().is_none() data-testid="workbench-load-error">
                <p role="alert">{move || load_error.get()}</p>
                <button type="button" disabled=move || busy.get() on:click=move |_| refresh_gen.update(|n| *n += 1)>{move || i18n.t("common.retry")}</button>
            </div>
            <p role="alert" data-testid="workbench-action-error">{move || action_error.get()}</p>
            <p role="status" hidden=move || !busy.get()>{move || i18n.t("common.saving")}</p>
            <div class="workbench-main-body">
                <div class="workbench-chat-column">
                    <ChatPage
                        knowledge_scope=Signal::derive(move || documents.get().into_iter().filter(|doc| doc.status == "completed").map(|doc| doc.id).collect::<Vec<_>>())
                        selected_scope=Signal::derive(move || { let mut ids: Vec<_> = selected_docs.get().into_iter().collect(); ids.sort(); ids })
                    />
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
                                    <button type="button" disabled=move || loading.get() || busy.get() on:click=move |_| refresh_gen.update(|n| *n += 1)>{move || i18n.t("common.retry")}</button>
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
                                <p class="page-status-empty" data-testid="sources-empty" hidden=move || loading.get() || load_error.get().is_some() || !documents.get().is_empty()>
                                    {move || i18n.t("workbench.sourcesEmpty")}
                                </p>
                                <ul class="rail-doc-list">
                                    <For
                                        each=move || documents.get()
                                        key=|doc| (doc.id.clone(), doc.status.clone(), doc.file_name.clone())
                                        children=move |doc| {
                                            let doc_id = doc.id.clone();
                                            let select_id = doc.id.clone();
                                            let checked_id = doc.id.clone();
                                            let preview_id = doc.id.clone();
                                            let status_key = match doc.status.as_str() { "completed" => "dashboardStatusReady", "failed" | "upload_invalid" => "dashboardStatusFailed", _ => "dashboardStatusProcessing" };
                                            view! {
                                                <li class="rail-doc-item" data-testid="workspace-doc-item" data-status=doc.status.clone()>
                                                    <label class="rail-doc-select">
                                                        <input
                                                            type="checkbox"
                                                            data-testid="doc-select"
                                                            disabled=doc.status != "completed"
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
                                                        <button type="button" class="rail-doc-name" data-testid="preview-doc" on:click=move |_| set_source_query.set(Some(preview_id.clone()))>{doc.file_name}</button>
                                                        <span class="rail-doc-status">{move || i18n.t(status_key)}</span>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="rail-doc-delete"
                                                        data-testid="delete-doc-btn" disabled=move || busy.get()
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
                                                data-testid="submit-note-btn" disabled=move || busy.get()
                                            >
                                                {move || i18n.t("workbench.saveNote")}
                                            </button>
                                        </div>
                                    </form>
                                </Show>

                                <ul class="rail-note-list">
                                    <For
                                        each=move || notes.get()
                                        key=|note| (note.id.clone(), note.title.clone(), note.preview.clone())
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
                                                        data-testid="delete-note-btn" disabled=move || busy.get()
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
                                    key=|session| (session.id.clone(), session.pinned, session.title.clone())
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
                                                <button type="button" data-testid="pin-session" disabled=move || busy.get() on:click=move |_| {
                                                    let tok = current_wb_token(token);
                                                    let sid = sid_pin.clone();
                                                    run_workbench_action(workspace_id, busy, action_error, refresh_gen, async move {
                                                        BrowserRestClient::new(&poc_api_base(), Some(tok)).update_session(&sid, None, Some(!pinned)).await.map(|_| ())
                                                    }, Callback::new(move |_| ()));
                                                }>{if pinned { i18n.t("workspaceUnpinSessionAction") } else { i18n.t("workspacePinSessionAction") }}</button>
                                                <button type="button" data-testid="delete-session" disabled=move || busy.get() on:click=move |_| {
                                                    let tok = current_wb_token(token);
                                                    let sid = sid_del.clone();
                                                    let deleted = sid.clone();
                                                    run_workbench_action(workspace_id, busy, action_error, refresh_gen, async move {
                                                        BrowserRestClient::new(&poc_api_base(), Some(tok)).delete_session(&sid).await
                                                    }, Callback::new(move |_| { if session_query.get_untracked().as_ref() == Some(&deleted) { set_session_query.set(None); } }));
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
            <AppDialog open=Signal::derive(move || source_query.get().is_some()) title_key="workbench.previewTitle" test_id="source-preview" on_close=Callback::new(move |_| set_source_query.set(None))>
                <p role="status" hidden=move || !preview_loading.get()>{move || i18n.t("common.loading")}</p>
                <p role="alert">{move || preview_error.get()}</p>
                <button type="button" hidden=move || preview_error.get().is_none() on:click=move |_| preview_refresh.update(|n| *n += 1)>{move || i18n.t("common.retry")}</button>
                <pre class="source-preview-text">{move || preview.get()}</pre>
            </AppDialog>
            <AppDialog open=Signal::derive(move || show_upload.get()) title_key="workbench.uploadTitle" test_id="upload-dialog" on_close=Callback::new(move |_| show_upload.set(false))>
                <p role="alert">{move || action_error.get()}</p>
                <div class="upload-tabs">
                    <button type="button" data-testid="upload-tab-file" on:click=move |_| upload_tab.set("file".into())>{move || i18n.t("workbench.uploadFile")}</button>
                    <button type="button" data-testid="upload-tab-url" on:click=move |_| upload_tab.set("url".into())>{move || i18n.t("workbench.uploadUrl")}</button>
                    <button type="button" data-testid="upload-tab-paste" on:click=move |_| upload_tab.set("paste".into())>{move || i18n.t("workbench.uploadPaste")}</button>
                </div>
                <div hidden=move || upload_tab.get() != "file">
                    <input type="file" data-testid="workspace-file-input" accept=SESSION_FILE_ACCEPT node_ref=file_input/>
                    <button type="button" data-testid="workspace-file-submit" disabled=move || busy.get() on:click=move |_| {
                        start_workspace_file_upload(token, workspace_id, file_input, toaster, refresh_gen, busy, action_error, show_upload);
                    }>{move || i18n.t("workbench.startUpload")}</button>
                </div>
                <div hidden=move || upload_tab.get() != "url">
                    <input type="url" data-testid="workspace-url-input" prop:value=move || source_url.get() on:input=move |ev| source_url.set(event_target_value(&ev))/>
                    <button type="button" data-testid="workspace-url-submit" disabled=move || busy.get() on:click=move |_| {
                        let tok = current_wb_token(token);
                        let wid = workspace_id.get_untracked();
                        let url = source_url.get();
                        run_workbench_action(workspace_id, busy, action_error, refresh_gen, async move {
                            BrowserRestClient::new(&poc_api_base(), Some(tok)).add_workspace_source_url(&wid, &url).await.map(|_| ())
                        }, Callback::new(move |_| {
                            toaster.push(t_now("workbench.linkSubmitted")); source_url.set(String::new()); show_upload.set(false);
                        }));
                    }>{move || i18n.t("workbench.addLink")}</button>
                </div>
                <div hidden=move || upload_tab.get() != "paste">
                    <input data-testid="workspace-paste-title" prop:value=move || paste_title.get() on:input=move |ev| paste_title.set(event_target_value(&ev))/>
                    <textarea data-testid="workspace-paste-body" prop:value=move || paste_body.get() on:input=move |ev| paste_body.set(event_target_value(&ev))></textarea>
                    <button type="button" data-testid="workspace-paste-submit" disabled=move || busy.get() on:click=move |_| {
                        let tok = current_wb_token(token);
                        let wid = workspace_id.get_untracked();
                        let title = paste_title.get();
                        let content = paste_body.get();
                        run_workbench_action(workspace_id, busy, action_error, refresh_gen, async move {
                            BrowserRestClient::new(&poc_api_base(), Some(tok)).add_workspace_source_paste(&wid, &title, &content).await.map(|_| ())
                        }, Callback::new(move |_| {
                            toaster.push(t_now("workbench.textSubmitted")); paste_title.set(String::new()); paste_body.set(String::new()); show_upload.set(false);
                        }));
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

fn run_workbench_action(
    workspace_id: Signal<String>, busy: RwSignal<bool>, error: RwSignal<Option<String>>,
    refresh: RwSignal<u64>, task: impl std::future::Future<Output = Result<(), web_sdk::TransportError>> + 'static,
    success: Callback<()>,
) {
    if busy.get_untracked() { return; }
    let wid = workspace_id.get_untracked();
    let generation = refresh.get_untracked();
    busy.set(true); error.set(None);
    leptos::task::spawn_local(async move {
        let result = task.await;
        if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str()) || refresh.try_get_untracked() != Some(generation) { return; }
        busy.set(false);
        match result {
            Ok(()) => { success.run(()); refresh.update(|n| *n += 1); }
            Err(err) => error.set(Some(err.to_string())),
        }
    });
}

fn start_workspace_file_upload(
    token: RwSignal<String>, workspace_id: Signal<String>, file_input: NodeRef<leptos::html::Input>,
    toaster: Toaster, refresh: RwSignal<u64>, busy: RwSignal<bool>,
    error: RwSignal<Option<String>>, show_upload: RwSignal<bool>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(file) = file_input.get().and_then(|i| i.files()).and_then(|f| f.item(0)) else {
            error.set(Some(t_now("workbench.chooseFile"))); return;
        };
        let tok = current_wb_token(token);
        let wid = workspace_id.get_untracked();
        run_workbench_action(workspace_id, busy, error, refresh, async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let mime = if file.type_().is_empty() { "application/octet-stream".to_string() } else { file.type_() };
            let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await
                .map_err(|_| web_sdk::TransportError::Unavailable(t_now("workbench.readFailed")))?;
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            let upload = client.create_workspace_document_upload(&wid, &file.name(), file.size() as u64, &mime).await?;
            client.put_upload_bytes(&upload.upload_url, &bytes, &mime).await?;
            client.complete_upload(&upload.document_id).await
        }, Callback::new(move |_| {
            toaster.push(t_now("workbench.uploaded")); show_upload.set(false);
            if let Some(input) = file_input.get_untracked() { input.set_value(""); }
        }));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (token, workspace_id, file_input, toaster, refresh, busy, error, show_upload);
}
