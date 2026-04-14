pub(crate) struct WorkspaceSourceRuntime {
    pub handle_add_url_source: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    pub handle_delete_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub handle_reindex_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    pub refresh_sources_after_upload: StoredValue<Arc<dyn Fn() + Send + Sync>>,
}

pub(crate) fn setup_workspace_source_runtime(
    auth: crate::state::auth::AuthState,
    locale: ReadSignal<crate::i18n::Locale>,
    workspace_id: Memo<String>,
    loaded_sources_key: ReadSignal<String>,
    set_loaded_sources_key: WriteSignal<String>,
    sources: ReadSignal<Vec<SourceRow>>,
    set_sources: WriteSignal<Vec<SourceRow>>,
    _selected_source_ids: ReadSignal<Vec<String>>,
    set_selected_source_ids: WriteSignal<Vec<String>>,
    set_selected_document: WriteSignal<Option<SourceRow>>,
    _sources_loading: ReadSignal<bool>,
    set_sources_loading: WriteSignal<bool>,
    status_polling: ReadSignal<bool>,
    set_status_polling: WriteSignal<bool>,
    status_poll_loop_active: ReadSignal<bool>,
    set_status_poll_loop_active: WriteSignal<bool>,
    url_source: ReadSignal<String>,
    set_url_source: WriteSignal<String>,
    set_adding_url_source: WriteSignal<bool>,
    set_workspace_error: WriteSignal<String>,
    docscope_initialized: ReadSignal<bool>,
    set_docscope_initialized: WriteSignal<bool>,
) -> WorkspaceSourceRuntime {
    let apply_sources_update = move |next_sources: Vec<SourceRow>| {
        let previous_sources = sources.get_untracked();
        let initialize_docscope = !docscope_initialized.get_untracked();
        let newly_eligible_ids = next_sources
            .iter()
            .filter(|source| source_status_docscope_eligible(&source.status))
            .filter(|source| {
                previous_sources
                    .iter()
                    .find(|previous| previous.id == source.id)
                    .map(|previous| !source_status_docscope_eligible(&previous.status))
                    .unwrap_or(true)
            })
            .map(|source| source.id.clone())
            .collect::<Vec<_>>();

        set_sources.set(next_sources.clone());
        set_selected_source_ids.update(|selected| {
            selected.retain(|id| {
                next_sources.iter().any(|source| {
                    source.id == *id && source_status_docscope_eligible(&source.status)
                })
            });

            if initialize_docscope && selected.is_empty() {
                selected.extend(
                    next_sources
                        .iter()
                        .filter(|source| source_status_docscope_eligible(&source.status))
                        .map(|source| source.id.clone()),
                );
            } else {
                for source_id in newly_eligible_ids {
                    if !selected.iter().any(|id| id == &source_id) {
                        selected.push(source_id);
                    }
                }
            }
        });

        if initialize_docscope {
            set_docscope_initialized.set(true);
        }
    };

    let fetch_sources = move || {
        let workspace_id_value = workspace_id.get_untracked();
        if workspace_id_value.is_empty() {
            return;
        }
        let token = auth.token.get_untracked();
        if token.is_none() {
            return;
        }

        set_sources_loading.set(true);
        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
        let apply_sources_update = apply_sources_update.clone();
        spawn(async move {
            match client.list_sources(&workspace_id_value).await {
                Ok(resp) => {
                    let should_poll = resp
                        .sources
                        .iter()
                        .any(|source| !source_status_terminal(&source.status));
                    apply_sources_update(resp.sources);
                    if should_poll {
                        set_status_polling.set(true);
                    }
                }
                Err(error) => {
                    set_workspace_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载资料列表失败",
                            "Failed to load sources"
                        ),
                        error
                    ));
                }
            }
            set_sources_loading.set(false);
        });
    };

    let auth_for_sources = auth.clone();
    let fetch_sources_on_mount = fetch_sources.clone();
    run_once_after_hydration(
        move || {
            auth_for_sources
                .token
                .get()
                .map(|value| format!("{}:{}", value, workspace_id.get()))
                .unwrap_or_default()
        },
        loaded_sources_key,
        set_loaded_sources_key,
        move || fetch_sources_on_mount(),
    );

    let auth_for_status_poll = auth.clone();
    let workspace_id_for_status_poll = workspace_id;
    Effect::new(move |_| {
        if !status_polling.get() || status_poll_loop_active.get() {
            return;
        }
        set_status_poll_loop_active.set(true);
        let auth = auth_for_status_poll.clone();
        spawn_local(async move {
            loop {
                TimeoutFuture::new(2000).await;
                let Some(token) = auth.token.get() else {
                    set_status_polling.set(false);
                    set_status_poll_loop_active.set(false);
                    break;
                };
                let workspace_id_value = workspace_id_for_status_poll.get();
                if workspace_id_value.is_empty() {
                    set_status_polling.set(false);
                    set_status_poll_loop_active.set(false);
                    break;
                }
                let client = ApiClient::new(api_base_url()).with_auth(token);
                let apply_sources_update = apply_sources_update.clone();
                match client.list_sources(&workspace_id_value).await {
                    Ok(resp) => {
                        let should_continue = resp
                            .sources
                            .iter()
                            .any(|source| !source_status_terminal(&source.status));
                        apply_sources_update(resp.sources);
                        if !should_continue {
                            set_status_polling.set(false);
                            set_status_poll_loop_active.set(false);
                            break;
                        }
                    }
                    Err(_) => {
                        set_status_polling.set(false);
                        set_status_poll_loop_active.set(false);
                        break;
                    }
                }
            }
        });
    });

    let handle_delete_document = {
        let auth = auth.clone();
        let fetch_sources = fetch_sources.clone();
        move |document_id: String| {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.delete_document(&document_id).await {
                    Ok(_) => {
                        set_selected_document.set(None);
                        fetch_sources();
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "删除资料失败",
                                "Failed to delete source"
                            ),
                            error
                        ));
                    }
                }
            });
        }
    };
    let handle_delete_document = StoredValue::new(
        Arc::new(handle_delete_document) as Arc<dyn Fn(String) + Send + Sync>,
    );

    let handle_reindex_document = {
        let auth = auth.clone();
        let fetch_sources = fetch_sources.clone();
        move |document_id: String| {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.reindex_document(&document_id).await {
                    Ok(_) => {
                        fetch_sources();
                        set_status_polling.set(true);
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "重新索引资料失败",
                                "Failed to reindex source"
                            ),
                            error
                        ));
                    }
                }
            });
        }
    };
    let handle_reindex_document = StoredValue::new(
        Arc::new(handle_reindex_document) as Arc<dyn Fn(String) + Send + Sync>,
    );

    let handle_add_url_source = {
        let auth = auth.clone();
        let fetch_sources = fetch_sources.clone();
        move || {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let workspace_id_value = workspace_id.get_untracked();
            let url = url_source.get_untracked().trim().to_string();
            if workspace_id_value.is_empty() || url.is_empty() {
                return;
            }
            set_adding_url_source.set(true);
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                let result = client.add_url_source(&workspace_id_value, &url).await;
                set_adding_url_source.set(false);
                match result {
                    Ok(_) => {
                        set_url_source.set(String::new());
                        fetch_sources();
                        set_status_polling.set(true);
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "添加链接资料失败",
                                "Failed to add URL source"
                            ),
                            error
                        ));
                    }
                }
            });
        }
    };
    let handle_add_url_source =
        StoredValue::new(Arc::new(handle_add_url_source) as Arc<dyn Fn() + Send + Sync>);

    let refresh_sources_after_upload = {
        let fetch_sources = fetch_sources.clone();
        StoredValue::new(Arc::new(move || fetch_sources()) as Arc<dyn Fn() + Send + Sync>)
    };

    WorkspaceSourceRuntime {
        handle_add_url_source,
        handle_delete_document,
        handle_reindex_document,
        refresh_sources_after_upload,
    }
}
