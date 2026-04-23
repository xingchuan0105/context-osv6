include!("workspace/setup_session_runtime.rs");
include!("workspace/setup_notes_runtime.rs");
include!("workspace/setup_source_runtime.rs");

pub(crate) struct WorkspacePageRuntime {
    pub auth: crate::state::auth::AuthState,
    pub locale: ReadSignal<crate::i18n::Locale>,
    pub workspace_id: Memo<String>,
    pub chat_for_desktop_history: crate::state::chat::ChatState,
    pub chat_for_mobile_history: crate::state::chat::ChatState,
    pub chat_for_desktop_sources: StoredValue<crate::state::chat::ChatState>,
    pub chat_for_mobile_sources: StoredValue<crate::state::chat::ChatState>,
    pub sessions: ReadSignal<Vec<ChatSession>>,
    pub set_sessions: WriteSignal<Vec<ChatSession>>,
    pub sessions_loading: ReadSignal<bool>,
    pub creating_session: ReadSignal<bool>,
    pub deleting_session_id: ReadSignal<Option<String>>,
    pub set_deleting_session_id: WriteSignal<Option<String>>,
    pub sources: ReadSignal<Vec<SourceRow>>,
    pub selected_source_ids: ReadSignal<Vec<String>>,
    pub set_selected_source_ids: WriteSignal<Vec<String>>,
    pub selected_document: ReadSignal<Option<SourceRow>>,
    pub set_selected_document: WriteSignal<Option<SourceRow>>,
    pub sources_loading: ReadSignal<bool>,
    pub status_polling: ReadSignal<bool>,
    pub url_source: ReadSignal<String>,
    pub set_url_source: WriteSignal<String>,
    pub adding_url_source: ReadSignal<bool>,
    pub show_upload_modal: ReadSignal<bool>,
    pub set_show_upload_modal: WriteSignal<bool>,
    pub left_rail_open: ReadSignal<bool>,
    pub set_left_rail_open: WriteSignal<bool>,
    pub right_rail_open: ReadSignal<bool>,
    pub set_right_rail_open: WriteSignal<bool>,
    pub workspace_name: ReadSignal<String>,
    pub set_workspace_name: WriteSignal<String>,
    pub workspace_error: ReadSignal<String>,
    pub set_workspace_error: WriteSignal<String>,
    pub pinned_source_ids: ReadSignal<Vec<String>>,
    pub notes: ReadSignal<Vec<NotebookNote>>,
    pub active_note_id: ReadSignal<Option<String>>,
    pub set_active_note_id: WriteSignal<Option<String>>,
    pub note_title: ReadSignal<String>,
    pub set_note_title: WriteSignal<String>,
    pub note_content: ReadSignal<String>,
    pub set_note_content: WriteSignal<String>,
    pub notes_loading: ReadSignal<bool>,
    pub note_sync_state: ReadSignal<DraftSyncState>,
    pub set_note_sync_revision: WriteSignal<u64>,
    pub handle_create_session: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    pub save_session_update:
        StoredValue<Arc<dyn Fn(String, UpdateChatSessionRequest) + Send + Sync>>,
    pub handle_add_url_source: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    pub handle_delete_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_reindex_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_toggle_source_pin: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_create_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_delete_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_promote_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub append_to_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub refresh_sources_after_upload: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    pub set_docscope_initialized: WriteSignal<bool>,
}

pub(crate) fn setup_workspace_page() -> WorkspacePageRuntime {
    let params = use_params_map();
    let workspace_id = Memo::new(move |_| params.get().get("notebook_id").unwrap_or_default());

    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let chat = provide_chat_state();
    let chat_for_desktop_history = chat.clone();
    let chat_for_mobile_history = chat.clone();
    let chat_for_desktop_sources = StoredValue::new(chat.clone());
    let chat_for_mobile_sources = StoredValue::new(chat.clone());

    let (sessions, set_sessions) = signal(Vec::<ChatSession>::new());
    let (sources, set_sources) = signal(Vec::<SourceRow>::new());
    let (sessions_loading, set_sessions_loading) = signal(false);
    let (sources_loading, set_sources_loading) = signal(false);
    let (creating_session, set_creating_session) = signal(false);
    let (deleting_session_id, set_deleting_session_id) = signal(Option::<String>::None);
    let (selected_source_ids, set_selected_source_ids) = signal(Vec::<String>::new());
    let (selected_document, set_selected_document) = signal(Option::<SourceRow>::None);
    let (show_upload_modal, set_show_upload_modal) = signal(false);
    let (loaded_session_key, set_loaded_session_key) = signal(String::new());
    let (loaded_sources_key, set_loaded_sources_key) = signal(String::new());
    let (loaded_notes_key, set_loaded_notes_key) = signal(String::new());
    let (loaded_workspace_prefs_key, set_loaded_workspace_prefs_key) = signal(String::new());
    let (workspace_name, set_workspace_name) = signal(String::new());
    let (status_polling, set_status_polling) = signal(false);
    let (status_poll_loop_active, set_status_poll_loop_active) = signal(false);
    let (workspace_error, set_workspace_error) = signal(String::new());
    let (last_chat_status, set_last_chat_status) = signal(ChatStatus::Idle);
    let (url_source, set_url_source) = signal(String::new());
    let (adding_url_source, set_adding_url_source) = signal(false);
    let (handled_focus_request, set_handled_focus_request) = signal(0_u64);
    let (docscope_initialized, set_docscope_initialized) = signal(false);
    let (left_rail_open, set_left_rail_open) = signal(false);
    let (right_rail_open, set_right_rail_open) = signal(false);
    let (pinned_source_ids, set_pinned_source_ids) = signal(Vec::<String>::new());

    let workspace_state = provide_workspace_state(
        sources,
        set_sources,
        selected_source_ids,
        set_selected_source_ids,
        selected_document,
        set_selected_document,
    );

    Effect::new(move |_| {
        let _ = workspace_id.get();
        set_pinned_source_ids.set(Vec::new());
    });

    Effect::new(move |_| {
        let focus_request = workspace_state.focus_request.get();
        if focus_request == handled_focus_request.get() {
            return;
        }
        set_handled_focus_request.set(focus_request);
        if let Some(focus) = workspace_state.citation_focus.get()
            && let Some(source) = workspace_state
                .sources
                .get()
                .into_iter()
                .find(|source| source.id == focus.doc_id)
        {
            workspace_state.set_selected_document.set(Some(source));
        }
    });

    let query_params = use_query_map();
    let (session_query_loaded, set_session_query_loaded) = signal(false);
    let chat_for_query_session = chat.clone();
    Effect::new(move |_| {
        if session_query_loaded.get() {
            return;
        }
        let session_id = query_params.get().get("session").unwrap_or_default();
        if session_id.is_empty() || sessions_loading.get() || sessions.get().is_empty() {
            return;
        }
        let matches = sessions.get().iter().any(|session| session.id == session_id);
        if !matches {
            return;
        }
        set_session_query_loaded.set(true);
        let Some(token) = auth.token.get() else {
            return;
        };
        let client = ApiClient::new(api_base_url()).with_auth(token);
        let chat_clone = chat_for_query_session.clone();
        let session_id_clone = session_id.to_string();
        spawn(async move {
            if let Ok(resp) = client.get_chat_messages(&session_id_clone).await {
                let messages = resp
                    .messages
                    .into_iter()
                    .map(|message| {
                        (
                            message.id,
                            message.role,
                            message.content,
                            message.answer_blocks,
                            message.citations,
                        )
                    })
                    .collect();
                chat_clone.load_session_messages(session_id_clone, messages);
            }
        });
    });

    let WorkspaceSessionRuntime {
        handle_create_session,
        save_session_update,
    } = setup_workspace_session_runtime(
        auth.clone(),
        locale,
        workspace_id,
        set_workspace_name,
        set_workspace_error,
        loaded_session_key,
        set_loaded_session_key,
        set_sessions,
        sessions_loading,
        set_sessions_loading,
        set_creating_session,
        chat.clone(),
        last_chat_status,
        set_last_chat_status,
    );

    let WorkspaceSourceRuntime {
        handle_add_url_source,
        handle_delete_document,
        handle_reindex_document,
        refresh_sources_after_upload,
    } = setup_workspace_source_runtime(
        auth.clone(),
        locale,
        workspace_id,
        loaded_sources_key,
        set_loaded_sources_key,
        sources,
        set_sources,
        selected_source_ids,
        set_selected_source_ids,
        set_selected_document,
        sources_loading,
        set_sources_loading,
        status_polling,
        set_status_polling,
        status_poll_loop_active,
        set_status_poll_loop_active,
        url_source,
        set_url_source,
        set_adding_url_source,
        set_workspace_error,
        docscope_initialized,
        set_docscope_initialized,
    );

    let WorkspaceNotesRuntime {
        notes,
        active_note_id,
        set_active_note_id,
        note_title,
        set_note_title,
        note_content,
        set_note_content,
        notes_loading,
        note_sync_state,
        set_note_sync_revision,
        handle_create_note,
        handle_delete_note,
        handle_promote_note,
        append_to_note,
    } = setup_workspace_notes_runtime(
        auth.clone(),
        locale,
        workspace_id,
        loaded_notes_key,
        set_loaded_notes_key,
        set_workspace_error,
        refresh_sources_after_upload,
    );

    run_once_after_hydration(
        move || {
            auth.token
                .get()
                .map(|value| format!("{}:{}", value, workspace_id.get()))
                .unwrap_or_default()
        },
        loaded_workspace_prefs_key,
        set_loaded_workspace_prefs_key,
        move || {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let notebook_id = workspace_id.get_untracked();
            if notebook_id.is_empty() {
                return;
            }
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.get_user_preferences().await {
                    Ok(preferences) => {
                        set_pinned_source_ids
                            .set(workspace_pinned_source_ids(&preferences.dashboard, &notebook_id));
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "加载工作台偏好失败",
                                "Failed to load workspace preferences",
                            ),
                            error
                        ));
                    }
                }
            });
        },
    );

    let handle_toggle_source_pin = StoredValue::new(Arc::new({
        let auth = auth.clone();
        move |source_id: String| {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let notebook_id = workspace_id.get_untracked();
            if notebook_id.is_empty() {
                return;
            }

            let next_pins = {
                let mut pins = pinned_source_ids.get_untracked();
                if let Some(index) = pins.iter().position(|id| id == &source_id) {
                    pins.remove(index);
                } else {
                    pins.push(source_id.clone());
                }
                pins
            };
            set_pinned_source_ids.set(next_pins.clone());

            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.get_user_preferences().await {
                    Ok(mut preferences) => {
                        upsert_workspace_pinned_sources(
                            &mut preferences.dashboard.workspace_preferences,
                            &notebook_id,
                            next_pins,
                        );
                        if let Err(error) = client.update_user_preferences(&preferences).await {
                            set_workspace_error.set(format!(
                                "{}: {}",
                                choose(
                                    locale.get_untracked(),
                                    "同步固定资料失败",
                                    "Failed to sync pinned sources",
                                ),
                                error
                            ));
                        }
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "加载工作台偏好失败",
                                "Failed to load workspace preferences",
                            ),
                            error
                        ));
                    }
                }
            });
        }
    }) as Arc<dyn Fn(String) + Send + Sync>);

    WorkspacePageRuntime {
        auth,
        locale,
        workspace_id,
        chat_for_desktop_history,
        chat_for_mobile_history,
        chat_for_desktop_sources,
        chat_for_mobile_sources,
        sessions,
        set_sessions,
        sessions_loading,
        creating_session,
        deleting_session_id,
        set_deleting_session_id,
        sources,
        selected_source_ids,
        set_selected_source_ids,
        selected_document,
        set_selected_document,
        sources_loading,
        status_polling,
        url_source,
        set_url_source,
        adding_url_source,
        show_upload_modal,
        set_show_upload_modal,
        left_rail_open,
        set_left_rail_open,
        right_rail_open,
        set_right_rail_open,
        workspace_name,
        set_workspace_name,
        workspace_error,
        set_workspace_error,
        pinned_source_ids,
        notes,
        active_note_id,
        set_active_note_id,
        note_title,
        set_note_title,
        note_content,
        set_note_content,
        notes_loading,
        note_sync_state,
        set_note_sync_revision,
        handle_create_session,
        save_session_update,
        handle_add_url_source,
        handle_delete_document,
        handle_reindex_document,
        handle_toggle_source_pin,
        handle_create_note,
        handle_delete_note,
        handle_promote_note,
        append_to_note,
        refresh_sources_after_upload,
        set_docscope_initialized,
    }
}
