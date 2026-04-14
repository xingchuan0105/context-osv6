pub(crate) struct WorkspaceNotesRuntime {
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
    pub handle_create_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_delete_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_promote_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub append_to_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
}

pub(crate) fn setup_workspace_notes_runtime(
    auth: crate::state::auth::AuthState,
    locale: ReadSignal<crate::i18n::Locale>,
    workspace_id: Memo<String>,
    loaded_notes_key: ReadSignal<String>,
    set_loaded_notes_key: WriteSignal<String>,
    set_workspace_error: WriteSignal<String>,
    refresh_sources_after_upload: StoredValue<Arc<dyn Fn() + Send + Sync>>,
) -> WorkspaceNotesRuntime {
    let (notes, set_notes) = signal(Vec::<NotebookNote>::new());
    let (active_note_id, set_active_note_id) = signal(Option::<String>::None);
    let (note_title, set_note_title) = signal(String::new());
    let (note_content, set_note_content) = signal(String::new());
    let (notes_loading, set_notes_loading) = signal(false);
    let (notes_loaded, set_notes_loaded) = signal(false);
    let (note_sync_state, set_note_sync_state) = signal(DraftSyncState::Idle);
    let (note_sync_revision, set_note_sync_revision) = signal(0_u64);

    Effect::new(move |_| {
        let _ = workspace_id.get();
        set_notes.set(Vec::new());
        set_active_note_id.set(None);
        set_note_title.set(String::new());
        set_note_content.set(String::new());
        set_notes_loading.set(false);
        set_notes_loaded.set(false);
        set_note_sync_state.set(DraftSyncState::Idle);
        set_note_sync_revision.set(0);
    });

    let fetch_notes = move || {
        let Some(token) = auth.token.get_untracked() else {
            return;
        };
        let notebook_id = workspace_id.get_untracked();
        if notebook_id.is_empty() {
            return;
        }
        set_notes_loading.set(true);
        let client = ApiClient::new(api_base_url()).with_auth(token);
        spawn(async move {
            match client.list_notebook_notes(&notebook_id).await {
                Ok(response) => {
                    let sorted = sort_workspace_notes(&response.notes);
                    let current_active = active_note_id.get_untracked();
                    let next_active = current_active
                        .filter(|note_id| sorted.iter().any(|note| note.id == *note_id));
                    set_notes.set(sorted.clone());
                    set_active_note_id.set(next_active.clone());
                    if let Some(note_id) = next_active {
                        if let Some(note) = sorted.iter().find(|note| note.id == note_id) {
                            set_note_title.set(note.title.clone());
                            set_note_content.set(note.content.clone());
                        }
                    } else {
                        set_note_title.set(String::new());
                        set_note_content.set(String::new());
                    }
                    set_note_sync_state.set(DraftSyncState::Synced);
                }
                Err(error) => {
                    set_workspace_error.set(format!(
                        "{}: {}",
                        choose(locale.get_untracked(), "加载笔记失败", "Failed to load notes"),
                        error
                    ));
                    set_note_sync_state.set(DraftSyncState::Error);
                }
            }
            set_notes_loading.set(false);
            set_notes_loaded.set(true);
        });
    };

    let auth_for_notes = auth.clone();
    let fetch_notes_on_mount = fetch_notes.clone();
    run_once_after_hydration(
        move || {
            auth_for_notes
                .token
                .get()
                .map(|value| format!("{}:{}", value, workspace_id.get()))
                .unwrap_or_default()
        },
        loaded_notes_key,
        set_loaded_notes_key,
        move || fetch_notes_on_mount(),
    );

    Effect::new(move |_| {
        let Some(note_id) = active_note_id.get() else {
            return;
        };
        if let Some(note) = notes.get().into_iter().find(|item| item.id == note_id) {
            set_note_title.set(note.title);
            set_note_content.set(note.content);
        }
    });

    Effect::new(move |_| {
        let revision = note_sync_revision.get();
        if revision == 0 || !notes_loaded.get() {
            return;
        }
        let Some(note_id) = active_note_id.get() else {
            return;
        };
        let Some(token) = auth.token.get_untracked() else {
            return;
        };
        let notebook_id = workspace_id.get_untracked();
        if notebook_id.is_empty() {
            return;
        }
        let current_title = note_title.get();
        let current_content = note_content.get();
        let current_note = notes
            .get_untracked()
            .into_iter()
            .find(|note| note.id == note_id);
        if current_note
            .as_ref()
            .map(|note| note.title == current_title && note.content == current_content)
            .unwrap_or(false)
        {
            return;
        }

        set_note_sync_state.set(DraftSyncState::Syncing);
        let client = ApiClient::new(api_base_url()).with_auth(token);
        spawn_local(async move {
            TimeoutFuture::new(700).await;
            if note_sync_revision.get_untracked() != revision {
                return;
            }

            match client
                .update_notebook_note(
                    &notebook_id,
                    &note_id,
                    &UpdateNotebookNoteRequest {
                        title: Some(current_title.clone()),
                        content: Some(current_content.clone()),
                    },
                )
                .await
            {
                Ok(response) => {
                    set_notes.update(|items| upsert_workspace_note(items, response.note.clone()));
                    set_note_title.set(response.note.title);
                    set_note_content.set(response.note.content);
                    set_note_sync_state.set(DraftSyncState::Synced);
                }
                Err(error) => {
                    set_workspace_error.set(format!(
                        "{}: {}",
                        choose(locale.get_untracked(), "同步笔记失败", "Failed to sync note"),
                        error
                    ));
                    set_note_sync_state.set(DraftSyncState::Error);
                }
            }
        });
    });

    let create_note_impl: Arc<dyn Fn(String) + Send + Sync> = Arc::new({
        let auth = auth.clone();
        move |seed_content: String| {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let notebook_id = workspace_id.get_untracked();
            if notebook_id.is_empty() {
                return;
            }
            set_workspace_error.set(String::new());
            set_notes_loading.set(true);
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client
                    .create_notebook_note(
                        &notebook_id,
                        &CreateNotebookNoteRequest {
                            title: None,
                            content: (!seed_content.is_empty()).then_some(seed_content.clone()),
                        },
                    )
                    .await
                {
                    Ok(response) => {
                        set_notes.update(|items| upsert_workspace_note(items, response.note.clone()));
                        set_active_note_id.set(Some(response.note.id.clone()));
                        set_note_title.set(response.note.title);
                        set_note_content.set(response.note.content);
                        set_note_sync_revision.set(0);
                        set_note_sync_state.set(DraftSyncState::Synced);
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(locale.get_untracked(), "创建笔记失败", "Failed to create note"),
                            error
                        ));
                    }
                }
                set_notes_loading.set(false);
            });
        }
    });
    let handle_create_note = StoredValue::new(create_note_impl.clone());

    let handle_delete_note = StoredValue::new(Arc::new({
        let auth = auth.clone();
        move |note_id: String| {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let notebook_id = workspace_id.get_untracked();
            if notebook_id.is_empty() {
                return;
            }
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.delete_notebook_note(&notebook_id, &note_id).await {
                    Ok(_) => {
                        set_notes.update(|items| items.retain(|note| note.id != note_id));
                        if active_note_id.get_untracked().as_deref() == Some(note_id.as_str()) {
                            set_active_note_id.set(None);
                            set_note_title.set(String::new());
                            set_note_content.set(String::new());
                            set_note_sync_revision.set(0);
                        }
                        set_note_sync_state.set(DraftSyncState::Idle);
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(locale.get_untracked(), "删除笔记失败", "Failed to delete note"),
                            error
                        ));
                    }
                }
            });
        }
    }) as Arc<dyn Fn(String) + Send + Sync>);

    let handle_promote_note = StoredValue::new(Arc::new({
        let auth = auth.clone();
        move |note_id: String| {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let notebook_id = workspace_id.get_untracked();
            if notebook_id.is_empty() {
                return;
            }
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.promote_notebook_note(&notebook_id, &note_id).await {
                    Ok(response) => {
                        set_notes.update(|items| upsert_workspace_note(items, response.note.clone()));
                        refresh_sources_after_upload.with_value(|callback| callback());
                        set_note_sync_state.set(DraftSyncState::Synced);
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "转换为内容源失败",
                                "Failed to promote note to source",
                            ),
                            error
                        ));
                    }
                }
            });
        }
    }) as Arc<dyn Fn(String) + Send + Sync>);

    let append_to_note = StoredValue::new(Arc::new({
        let create_note_impl = create_note_impl.clone();
        move |content: String| {
            if content.trim().is_empty() {
                return;
            }
            if active_note_id.get_untracked().is_some() {
                set_note_content.update(|current| {
                    if !current.trim().is_empty() {
                        current.push_str("\n\n");
                    }
                    current.push_str(&content);
                });
                set_note_sync_revision.update(|value| *value += 1);
                return;
            }
            create_note_impl(content);
        }
    }) as Arc<dyn Fn(String) + Send + Sync>);

    WorkspaceNotesRuntime {
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
    }
}
