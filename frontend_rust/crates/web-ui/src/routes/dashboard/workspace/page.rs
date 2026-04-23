#[component]
pub fn WorkspacePage() -> impl IntoView {
    let WorkspacePageRuntime {
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
        ..
    } = setup_workspace_page();
    let desktop_auth = auth.clone();
    let mobile_auth = auth.clone();

    view! {
        <div class=format!("{} workspace-shell", workspace_style::shell)>
            <WorkspaceTopBar
                locale=locale
                workspace_id=workspace_id
                workspace_name=workspace_name
                set_workspace_name=set_workspace_name
                set_workspace_error=set_workspace_error
                set_left_rail_open=set_left_rail_open
                set_right_rail_open=set_right_rail_open
            />

            <Show when=move || !workspace_error.get().is_empty()>
                <div class=workspace_style::error_strip>
                    <NoticeBanner message=workspace_error.get() tone=NoticeTone::Danger />
                </div>
            </Show>

            <div class=format!("{} workspace-body", workspace_style::body)>
                <aside class=format!("{} workspace-left-rail", workspace_style::desktop_left_rail)>
                    <WorkspaceLeftRail
                        locale=locale
                        auth=desktop_auth.clone()
                        active_chat=chat_for_desktop_history.clone()
                        sessions=sessions
                        set_sessions=set_sessions
                        sessions_loading=sessions_loading
                        creating_session=creating_session
                        deleting_session_id=deleting_session_id
                        set_deleting_session_id=set_deleting_session_id
                        set_workspace_error=set_workspace_error
                        on_create_session=handle_create_session
                        on_update_session=save_session_update
                        close_on_select=None
                    />
                </aside>

                <Show when=move || left_rail_open.get() && is_mobile()>
                    <div class=workspace_style::mobile_overlay on:click=move |_| set_left_rail_open.set(false)>
                        <div class=workspace_style::mobile_scrim></div>
                        <aside
                            class=format!("{} workspace-left-rail", workspace_style::mobile_left_rail)
                            on:click=|ev| ev.stop_propagation()
                        >
                            <WorkspaceLeftRail
                                locale=locale
                                auth=mobile_auth.clone()
                                active_chat=chat_for_mobile_history.clone()
                                sessions=sessions
                                set_sessions=set_sessions
                                sessions_loading=sessions_loading
                                creating_session=creating_session
                                deleting_session_id=deleting_session_id
                                set_deleting_session_id=set_deleting_session_id
                                set_workspace_error=set_workspace_error
                                on_create_session=handle_create_session
                                on_update_session=save_session_update
                                close_on_select=Some(set_left_rail_open)
                            />
                        </aside>
                    </div>
                </Show>

                <WorkspaceChatArea
                    notebook_id={workspace_id.get()}
                    append_to_note=append_to_note
                />

                <aside class=format!("{} workspace-right-rail", workspace_style::desktop_right_rail)>
                    <WorkspaceRightRail
                        locale=locale
                        chat=chat_for_desktop_sources
                        sources=sources
                        pinned_source_ids=pinned_source_ids
                        selected_source_ids=selected_source_ids
                        set_selected_source_ids=set_selected_source_ids
                        selected_document=selected_document
                        set_selected_document=set_selected_document
                        sources_loading=sources_loading
                        status_polling=status_polling
                        url_source=url_source
                        set_url_source=set_url_source
                        adding_url_source=adding_url_source
                        set_show_upload_modal=set_show_upload_modal
                        handle_add_url_source=handle_add_url_source
                        handle_toggle_source_pin=handle_toggle_source_pin
                        handle_delete_document=handle_delete_document
                        handle_reindex_document=handle_reindex_document
                        set_docscope_initialized=set_docscope_initialized
                        notes=notes
                        active_note_id=active_note_id
                        set_active_note_id=set_active_note_id
                        note_title=note_title
                        set_note_title=set_note_title
                        note_content=note_content
                        set_note_content=set_note_content
                        notes_loading=notes_loading
                        note_sync_state=note_sync_state
                        set_note_sync_revision=set_note_sync_revision
                        handle_create_note=handle_create_note
                        handle_delete_note=handle_delete_note
                        handle_promote_note=handle_promote_note
                        show_note_actions=true
                    />
                </aside>

                <Show when=move || right_rail_open.get() && is_mobile()>
                    <div class=workspace_style::mobile_overlay on:click=move |_| set_right_rail_open.set(false)>
                        <div class=workspace_style::mobile_scrim></div>
                        <aside
                            class=format!("{} workspace-right-rail", workspace_style::mobile_right_rail)
                            on:click=|ev| ev.stop_propagation()
                        >
                            <WorkspaceRightRail
                                locale=locale
                                chat=chat_for_mobile_sources
                                sources=sources
                                pinned_source_ids=pinned_source_ids
                                selected_source_ids=selected_source_ids
                                set_selected_source_ids=set_selected_source_ids
                                selected_document=selected_document
                                set_selected_document=set_selected_document
                                sources_loading=sources_loading
                                status_polling=status_polling
                                url_source=url_source
                                set_url_source=set_url_source
                                adding_url_source=adding_url_source
                                set_show_upload_modal=set_show_upload_modal
                                handle_add_url_source=handle_add_url_source
                                handle_toggle_source_pin=handle_toggle_source_pin
                                handle_delete_document=handle_delete_document
                                handle_reindex_document=handle_reindex_document
                                set_docscope_initialized=set_docscope_initialized
                                notes=notes
                                active_note_id=active_note_id
                                set_active_note_id=set_active_note_id
                                note_title=note_title
                                set_note_title=set_note_title
                                note_content=note_content
                                set_note_content=set_note_content
                                notes_loading=notes_loading
                                note_sync_state=note_sync_state
                                set_note_sync_revision=set_note_sync_revision
                                handle_create_note=handle_create_note
                                handle_delete_note=handle_delete_note
                                handle_promote_note=handle_promote_note
                                show_note_actions=false
                            />
                        </aside>
                    </div>
                </Show>
            </div>
        </div>

        <WorkspaceUploadModal
            workspace_id=workspace_id
            show_upload_modal=show_upload_modal
            set_show_upload_modal=set_show_upload_modal
            on_upload_success=refresh_sources_after_upload
            url_source=url_source
            set_url_source=set_url_source
            adding_url_source=adding_url_source
            handle_add_url_source=handle_add_url_source
        />
    }
}

#[component]
fn WorkspaceLeftRail(
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
    view! {
        <div class=workspace_style::left_inner>
            <WorkspaceHistoryPane
                locale=locale
                auth=auth
                active_chat=active_chat
                sessions=sessions
                set_sessions=set_sessions
                sessions_loading=sessions_loading
                creating_session=creating_session
                deleting_session_id=deleting_session_id
                set_deleting_session_id=set_deleting_session_id
                set_workspace_error=set_workspace_error
                on_create_session=on_create_session
                on_update_session=on_update_session
                close_on_select=close_on_select
            />
        </div>
    }
}

#[component]
fn WorkspaceChatArea(
    notebook_id: String,
    append_to_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
) -> impl IntoView {
    view! {
        <main class=format!("{} workspace-main", workspace_style::main)>
            <div class=workspace_style::full_height>
                <ChatPanel notebook_id=notebook_id append_to_note=append_to_note />
            </div>
        </main>
    }
}

fn update_workspace_right_rail_split(
    right_inner_ref: &NodeRef<leptos::html::Div>,
    set_top_section_percent: WriteSignal<f64>,
    client_y: i32,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(container) = right_inner_ref.get() else {
            return;
        };
        let rect = container.get_bounding_client_rect();
        let height = rect.height();
        if height <= 0.0 {
            return;
        }

        let next_percent = (((client_y as f64) - rect.top()) / height * 100.0).clamp(28.0, 72.0);
        set_top_section_percent.set(next_percent);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = right_inner_ref;
        let _ = set_top_section_percent;
        let _ = client_y;
    }
}

#[component]
fn WorkspaceRightRail(
    locale: ReadSignal<crate::i18n::Locale>,
    chat: StoredValue<crate::state::chat::ChatState>,
    sources: ReadSignal<Vec<SourceRow>>,
    pinned_source_ids: ReadSignal<Vec<String>>,
    selected_source_ids: ReadSignal<Vec<String>>,
    set_selected_source_ids: WriteSignal<Vec<String>>,
    selected_document: ReadSignal<Option<SourceRow>>,
    set_selected_document: WriteSignal<Option<SourceRow>>,
    sources_loading: ReadSignal<bool>,
    status_polling: ReadSignal<bool>,
    url_source: ReadSignal<String>,
    set_url_source: WriteSignal<String>,
    adding_url_source: ReadSignal<bool>,
    set_show_upload_modal: WriteSignal<bool>,
    handle_add_url_source: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    handle_toggle_source_pin: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    handle_delete_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    handle_reindex_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    set_docscope_initialized: WriteSignal<bool>,
    notes: ReadSignal<Vec<NotebookNote>>,
    active_note_id: ReadSignal<Option<String>>,
    set_active_note_id: WriteSignal<Option<String>>,
    note_title: ReadSignal<String>,
    set_note_title: WriteSignal<String>,
    note_content: ReadSignal<String>,
    set_note_content: WriteSignal<String>,
    notes_loading: ReadSignal<bool>,
    note_sync_state: ReadSignal<DraftSyncState>,
    set_note_sync_revision: WriteSignal<u64>,
    handle_create_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    handle_delete_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    handle_promote_note: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    show_note_actions: bool,
) -> impl IntoView {
    let right_inner_ref = NodeRef::<leptos::html::Div>::new();
    let right_inner_ref_for_drag_start = right_inner_ref.clone();
    let right_inner_ref_for_drag_move = right_inner_ref.clone();
    let (top_section_percent, set_top_section_percent) = signal(52.0_f64);
    let (dragging_split, set_dragging_split) = signal(false);

    view! {
        <Show when=move || dragging_split.get()>
            <div
                class=workspace_style::resize_overlay
                on:mousemove=move |ev| {
                    update_workspace_right_rail_split(
                        &right_inner_ref_for_drag_move,
                        set_top_section_percent,
                        ev.client_y(),
                    );
                }
                on:mouseup=move |_| set_dragging_split.set(false)
            />
        </Show>

        <div class=workspace_style::right_inner node_ref=right_inner_ref>
            <div
                class=workspace_style::top_section
                style=move || format!("flex: 0 0 {:.2}%;", top_section_percent.get())
            >
                <div class=workspace_style::section_scroll_padded>
                    <WorkspaceDocumentPane
                        locale=locale
                        chat=chat
                        sources=sources
                        pinned_source_ids=pinned_source_ids
                        selected_source_ids=selected_source_ids
                        set_selected_source_ids=set_selected_source_ids
                        selected_document=selected_document
                        set_selected_document=set_selected_document
                        sources_loading=sources_loading
                        status_polling=status_polling
                        url_source=url_source
                        set_url_source=set_url_source
                        adding_url_source=adding_url_source
                        set_show_upload_modal=set_show_upload_modal
                        handle_add_url_source=handle_add_url_source
                        handle_toggle_source_pin=handle_toggle_source_pin
                        set_docscope_initialized=set_docscope_initialized
                        handle_delete_document=handle_delete_document
                        handle_reindex_document=handle_reindex_document
                    />
                </div>
            </div>

            <div
                class=workspace_style::rail_divider
                role="separator"
                aria-orientation="horizontal"
                aria-label={move || choose(locale.get(), "调整右侧面板高度", "Resize right rail panels")}
                aria-valuemin="28"
                aria-valuemax="72"
                aria-valuenow=move || top_section_percent.get().round() as i32
                on:mousedown=move |ev| {
                    ev.prevent_default();
                    set_dragging_split.set(true);
                    update_workspace_right_rail_split(
                        &right_inner_ref_for_drag_start,
                        set_top_section_percent,
                        ev.client_y(),
                    );
                }
            >
                <span class=workspace_style::rail_divider_handle></span>
            </div>

            <div
                class=workspace_style::bottom_section
                style=move || format!("flex: 1 1 {:.2}%;", 100.0 - top_section_percent.get())
            >
                <div class=workspace_style::section_scroll>
                    <WorkspaceNotesPane
                        locale=locale
                        notes=notes
                        active_note_id=active_note_id
                        set_active_note_id=set_active_note_id
                        note_title=note_title
                        set_note_title=set_note_title
                        note_content=note_content
                        set_note_content=set_note_content
                        notes_loading=notes_loading
                        note_sync_state=note_sync_state
                        set_note_sync_revision=set_note_sync_revision
                        handle_create_note=handle_create_note
                        handle_delete_note=handle_delete_note
                        handle_promote_note=handle_promote_note
                        show_actions=show_note_actions
                    />
                </div>
            </div>
        </div>
    }
}
