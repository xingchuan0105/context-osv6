pub(crate) struct WorkspaceSessionRuntime {
    pub handle_create_session: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    pub save_session_update:
        StoredValue<Arc<dyn Fn(String, UpdateChatSessionRequest) + Send + Sync>>,
}

pub(crate) fn setup_workspace_session_runtime(
    auth: crate::state::auth::AuthState,
    locale: ReadSignal<crate::i18n::Locale>,
    workspace_id: Memo<String>,
    set_workspace_name: WriteSignal<String>,
    set_workspace_error: WriteSignal<String>,
    loaded_session_key: ReadSignal<String>,
    set_loaded_session_key: WriteSignal<String>,
    set_sessions: WriteSignal<Vec<ChatSession>>,
    _sessions_loading: ReadSignal<bool>,
    set_sessions_loading: WriteSignal<bool>,
    set_creating_session: WriteSignal<bool>,
    chat: crate::state::chat::ChatState,
    last_chat_status: ReadSignal<ChatStatus>,
    set_last_chat_status: WriteSignal<ChatStatus>,
) -> WorkspaceSessionRuntime {
    let auth_for_sessions = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_sessions
                .token
                .get()
                .map(|value| format!("{}:{}", value, workspace_id.get()))
                .unwrap_or_default()
        },
        loaded_session_key,
        set_loaded_session_key,
        move || {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let workspace_id_value = workspace_id.get_untracked();
            let workspace_id_for_notebook = workspace_id_value.clone();

            refresh_workspace_sessions(
                auth.token,
                workspace_id_value,
                set_sessions,
                set_sessions_loading,
                true,
            );

            let set_workspace_name_clone = set_workspace_name.clone();
            spawn(async move {
                if let Ok(resp) = client.get_notebook(&workspace_id_for_notebook).await {
                    set_workspace_name_clone.set(resp.notebook.title);
                }
            });
        },
    );

    let workspace_id_for_session_updates = workspace_id;
    let save_session_update = {
        let auth = auth.clone();
        move |session_id: String, request: UpdateChatSessionRequest| {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let current_locale = locale.get_untracked();
            let workspace_id_value = workspace_id_for_session_updates.get_untracked();
            spawn(async move {
                match client.update_chat_session(&session_id, &request).await {
                    Ok(updated) => {
                        set_sessions.update(|items| {
                            if let Some(existing) =
                                items.iter_mut().find(|item| item.id == updated.id)
                            {
                                *existing = updated;
                            }
                        });
                    }
                    Err(error) => {
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(current_locale, "更新会话失败", "Failed to update session"),
                            error
                        ));
                        refresh_workspace_sessions(
                            auth.token,
                            workspace_id_value,
                            set_sessions,
                            set_sessions_loading,
                            false,
                        );
                    }
                }
            });
        }
    };
    let save_session_update = StoredValue::new(
        Arc::new(save_session_update)
            as Arc<dyn Fn(String, UpdateChatSessionRequest) + Send + Sync>,
    );

    let auth_for_session_sync = auth.clone();
    let workspace_id_for_session_sync = workspace_id;
    Effect::new(move |_| {
        let current_status = chat.status.get();
        let current_session_id = chat.session_id.get();
        let previous_status = last_chat_status.get_untracked();

        if matches!(
            previous_status,
            ChatStatus::Submitting | ChatStatus::Streaming
        ) && matches!(current_status, ChatStatus::Done | ChatStatus::Error)
            && current_session_id.is_some()
        {
            refresh_workspace_sessions(
                auth_for_session_sync.token,
                workspace_id_for_session_sync.get(),
                set_sessions,
                set_sessions_loading,
                false,
            );
        }

        set_last_chat_status.set(current_status);
    });

    let handle_create_session = {
        let auth = auth.clone();
        let chat = chat.clone();
        move || {
            let Some(token) = auth.token.get_untracked() else {
                chat.reset();
                return;
            };
            let workspace_id_value = workspace_id.get_untracked();
            if workspace_id_value.is_empty() {
                chat.reset();
                return;
            }

            set_creating_session.set(true);
            set_workspace_error.set(String::new());
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let req = CreateChatSessionRequest {
                notebook_id: workspace_id_value,
                title: None,
                agent_type: chat.agent_mode.get().as_str().to_string(),
            };
            let chat = chat.clone();

            spawn(async move {
                match client.create_chat_session(&req).await {
                    Ok(session) => {
                        chat.reset();
                        chat.set_session(session.id.clone());
                        set_sessions.update(|items| {
                            items.retain(|existing| existing.id != session.id);
                            items.insert(0, session);
                        });
                    }
                    Err(error) => {
                        chat.reset();
                        set_workspace_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "创建新会话失败",
                                "Failed to create session"
                            ),
                            error
                        ));
                    }
                }
                set_creating_session.set(false);
            });
        }
    };
    let handle_create_session =
        StoredValue::new(Arc::new(handle_create_session) as Arc<dyn Fn() + Send + Sync>);

    WorkspaceSessionRuntime {
        handle_create_session,
        save_session_update,
    }
}
