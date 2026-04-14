#[component]
fn WorkspaceHistoryPane(
    locale: ReadSignal<crate::i18n::Locale>,
    auth: crate::state::auth::AuthState,
    active_chat: crate::state::chat::ChatState,
    sessions: ReadSignal<Vec<ChatSession>>,
    set_sessions: WriteSignal<Vec<ChatSession>>,
    sessions_loading: ReadSignal<bool>,
    creating_session: ReadSignal<bool>,
    deleting_session_id: ReadSignal<Option<String>>,
    set_deleting_session_id: WriteSignal<Option<String>>,
    set_workspace_error: WriteSignal<String>,
    on_create_session: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    on_update_session: StoredValue<Arc<dyn Fn(String, UpdateChatSessionRequest) + Send + Sync>>,
    close_on_select: Option<WriteSignal<bool>>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());
    let (open_menu_id, set_open_menu_id) = signal(Option::<String>::None);
    let session_views = Signal::derive(move || {
        let query = search_query.get().trim().to_lowercase();
        sort_workspace_sessions(&sessions.get())
            .into_iter()
            .filter(|session| {
                if query.is_empty() {
                    return true;
                }
                session
                    .title
                    .clone()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query)
                    || session
                        .summary
                        .clone()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>()
    });

    view! {
        <div class="app-pane flex-1">
            <div class="app-pane-header">
                <div>
                    <h2 class="app-pane-title">
                        {move || choose(locale.get(), "线程", "Threads")}
                    </h2>
                    <p class="app-pane-meta">
                        {move || choose(locale.get(), "这个知识库中的所有研究线程", "Research threads inside this notebook")}
                    </p>
                </div>
                {move || view! {
                    <StatusBadge
                        label=sessions.get().len().to_string()
                        tone=NoticeTone::Neutral
                    />
                }}
            </div>

            <div class="app-pane-body flex flex-col gap-3 p-3">
                <input
                    type="search"
                    class="app-input"
                    placeholder={move || choose(locale.get(), "搜索线程", "Search Threads")}
                    prop:value=move || search_query.get()
                    on:input=move |ev| set_search_query.set(event_target_value(&ev))
                />

                <button
                    class="app-button-primary w-full"
                    on:click=move |_| on_create_session.with_value(|cb| cb())
                    disabled=move || creating_session.get()
                >
                    {move || {
                        if creating_session.get() {
                            choose(locale.get(), "创建中...", "Creating...")
                        } else {
                            choose(locale.get(), "新线程", "New Thread")
                        }
                    }}
                </button>

                <div class="min-h-0 flex-1 overflow-y-auto">
                    <Show when=move || sessions_loading.get()>
                        <div class="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
                            {move || choose(locale.get(), "正在加载线程...", "Loading threads...")}
                        </div>
                    </Show>

                    <Show when=move || !sessions_loading.get() && session_views.get().is_empty()>
                        <div class="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
                            {move || {
                                if search_query.get().trim().is_empty() {
                                    choose(locale.get(), "还没有线程", "No threads yet")
                                } else {
                                    choose(locale.get(), "没有匹配的线程", "No matching threads")
                                }
                            }}
                        </div>
                    </Show>

                    <div class="space-y-2">
                        {move || {
                            session_views
                                .get()
                                .into_iter()
                                .map(|session| {
                                    let active_session_id = session.id.clone();
                                    let session_id = session.id.clone();
                                    let close_on_select_signal = close_on_select;
                                    let session_id_for_open = session_id.clone();
                                    let session_id_for_menu_toggle = session_id.clone();
                                    let session_id_for_menu_visibility = session_id.clone();
                                    let auth_for_open = auth.clone();
                                    let auth_for_delete = auth.clone();
                                    let chat_for_session = active_chat.clone();
                                    let active_chat_for_row_state = active_chat.clone();
                                    let active_chat_for_delete = active_chat.clone();
                                    let summary = session
                                        .summary
                                        .clone()
                                        .filter(|text| !text.is_empty())
                                        .unwrap_or_else(|| {
                                            session
                                                .title
                                                .clone()
                                                .unwrap_or_else(|| choose(locale.get(), "未命名线程", "Untitled thread").to_string())
                                        });
                                    let display_title = session
                                        .title
                                        .clone()
                                        .filter(|text| !text.is_empty())
                                        .unwrap_or_else(|| choose(locale.get(), "未命名线程", "Untitled thread").to_string());
                                    let display_title_for_label = display_title.clone();
                                    let display_title_for_rename =
                                        StoredValue::new(display_title.clone());
                                    let pinned = session.pinned;
                                    view! {
                                        <div
                                            class="rounded-2xl border border-transparent bg-card/60 px-3 py-3 transition-colors hover:border-border hover:bg-muted/40"
                                            class=("border-primary/30 bg-primary/5", move || {
                                                active_chat_for_row_state.session_id.get().as_deref() == Some(active_session_id.as_str())
                                            })
                                        >
                                            <div class="flex items-start gap-2">
                                                <button
                                                    type="button"
                                                    class="min-w-0 flex-1 text-left"
                                                    on:click=move |_| {
                                                        let Some(token) = auth_for_open.token.get() else {
                                                            return;
                                                        };
                                                        let client = ApiClient::new(api_base_url()).with_auth(token);
                                                        let session_id = session_id_for_open.clone();
                                                        let chat = chat_for_session.clone();
                                                        let close_on_select_signal = close_on_select_signal;
                                                        spawn(async move {
                                                            if let Ok(resp) = client.get_chat_messages(&session_id).await {
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
                                                                chat.load_session_messages(session_id, messages);
                                                            }
                                                            if let Some(setter) = close_on_select_signal {
                                                                setter.set(false);
                                                            }
                                                        });
                                                    }
                                                >
                                                    <div class="flex items-center gap-2">
                                                        <span class="truncate text-sm font-medium text-foreground">
                                                            {display_title_for_label.clone()}
                                                        </span>
                                                        <Show when=move || pinned>
                                                            <span class="rounded-full bg-muted px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] text-muted-foreground">
                                                                {move || choose(locale.get(), "置顶", "Pinned")}
                                                            </span>
                                                        </Show>
                                                    </div>
                                                    <div class="mt-1 truncate text-xs text-muted-foreground">
                                                        {summary}
                                                    </div>
                                                </button>

                                                <div class="relative">
                                                    <button
                                                        type="button"
                                                        class="rounded-lg p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
                                                        on:click=move |_| {
                                                            let menu_id = session_id_for_menu_toggle.clone();
                                                            set_open_menu_id.update(|current| {
                                                                if current.as_deref() == Some(menu_id.as_str()) {
                                                                    *current = None;
                                                                } else {
                                                                    *current = Some(menu_id.clone());
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6h.01M12 12h.01M12 18h.01"/>
                                                        </svg>
                                                    </button>

                                                    <Show when=move || open_menu_id.get().as_deref() == Some(session_id_for_menu_visibility.as_str())>
                                                        <div class="absolute right-0 top-9 z-20 w-36 rounded-2xl border border-border bg-card p-1 shadow-lg">
                                                            <button
                                                                type="button"
                                                                class="block w-full rounded-xl px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
                                                                on:click=move |_| {
                                                                    let Some(target_id) =
                                                                        open_menu_id.get_untracked()
                                                                    else {
                                                                        return;
                                                                    };
                                                                    set_open_menu_id.set(None);
                                                                    on_update_session.with_value(|cb| cb(
                                                                        target_id,
                                                                        UpdateChatSessionRequest {
                                                                            title: None,
                                                                            pinned: Some(!pinned),
                                                                        },
                                                                    ));
                                                                }
                                                            >
                                                                {move || {
                                                                    if pinned {
                                                                        choose(locale.get(), "取消置顶", "Unpin")
                                                                    } else {
                                                                        choose(locale.get(), "置顶", "Pin")
                                                                    }
                                                                }}
                                                            </button>
                                                            <button
                                                                type="button"
                                                                class="block w-full rounded-xl px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
                                                                on:click=move |_| {
                                                                    let title_seed =
                                                                        display_title_for_rename
                                                                            .with_value(|title| {
                                                                                title.clone()
                                                                            });
                                                                    if let Some(next_title) = prompt_session_title(&title_seed) {
                                                                        let Some(target_id) = open_menu_id.get_untracked() else {
                                                                            return;
                                                                        };
                                                                        set_open_menu_id.set(None);
                                                                        on_update_session.with_value(|cb| cb(
                                                                            target_id,
                                                                            UpdateChatSessionRequest {
                                                                                title: Some(next_title),
                                                                                pinned: None,
                                                                            },
                                                                        ));
                                                                    }
                                                                }
                                                            >
                                                                {move || choose(locale.get(), "重命名", "Rename")}
                                                            </button>
                                                            <button
                                                                type="button"
                                                                class="block w-full rounded-xl px-3 py-2 text-left text-sm text-red-600 hover:bg-red-50"
                                                                on:click={
                                                                    let active_chat_for_delete =
                                                                        active_chat_for_delete.clone();
                                                                    move |_| {
                                                                        let Some(token) = auth_for_delete.token.get()
                                                                        else {
                                                                            return;
                                                                        };
                                                                        let Some(deleting_id) =
                                                                            open_menu_id.get_untracked()
                                                                        else {
                                                                            return;
                                                                        };
                                                                        set_open_menu_id.set(None);
                                                                        set_deleting_session_id
                                                                            .set(Some(deleting_id.clone()));
                                                                        set_workspace_error.set(String::new());
                                                                        let client = ApiClient::new(api_base_url())
                                                                            .with_auth(token);
                                                                        let chat = active_chat_for_delete.clone();
                                                                        let active_session = chat.session_id.get();
                                                                        spawn(async move {
                                                                            match client.delete_chat_session(&deleting_id).await {
                                                                                Ok(_) => {
                                                                                    set_sessions.update(|items| {
                                                                                        items.retain(|item| item.id != deleting_id);
                                                                                    });
                                                                                    if active_session.as_deref() == Some(deleting_id.as_str()) {
                                                                                        chat.reset();
                                                                                    }
                                                                                }
                                                                                Err(error) => {
                                                                                    set_workspace_error.set(format!(
                                                                                        "{}: {}",
                                                                                        choose(locale.get_untracked(), "删除线程失败", "Failed to delete thread"),
                                                                                        error
                                                                                    ));
                                                                                }
                                                                            }
                                                                            set_deleting_session_id.set(None);
                                                                        });
                                                                    }
                                                                }
                                                            >
                                                                {move || {
                                                                    if deleting_session_id.get().is_some() {
                                                                        choose(locale.get(), "删除中...", "Deleting...")
                                                                    } else {
                                                                        choose(locale.get(), "删除", "Delete")
                                                                    }
                                                                }}
                                                            </button>
                                                        </div>
                                                    </Show>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                </div>
            </div>
        </div>
    }
}
