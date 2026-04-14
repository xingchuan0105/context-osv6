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
    view! {
        <div class="workspace-shell flex h-screen flex-col">
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
                <div class="px-4 pt-4">
                    <NoticeBanner message=workspace_error.get() tone=NoticeTone::Danger />
                </div>
            </Show>

            <div class="flex min-h-0 flex-1 overflow-hidden">
                // Desktop left rail
                <aside class="workspace-left-rail hidden md:block w-[248px] min-w-[248px] shrink-0 border-r border-border">
                    <div class="flex h-full flex-col p-4">
                        <WorkspaceHistoryPane
                            locale=locale
                            auth=auth.clone()
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
                    </div>
                </aside>

                // Mobile left rail overlay
                <Show when=move || left_rail_open.get() && is_mobile()>
                    <div class="fixed inset-0 z-50 md:hidden" on:click=move |_| set_left_rail_open.set(false)>
                        <div class="absolute inset-0 bg-black/50"></div>
                        <aside class="workspace-left-rail absolute left-0 top-0 h-full w-80 shadow-xl" on:click=|ev| ev.stop_propagation()>
                            <div class="flex h-full flex-col p-4">
                                <WorkspaceHistoryPane
                                    locale=locale
                                    auth=auth.clone()
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
                            </div>
                        </aside>
                    </div>
                </Show>

                <main class="workspace-main min-w-0 flex-1">
                    <div class="h-full p-6">
                        <div class="h-full overflow-hidden rounded-[14px] border border-border bg-card">
                            <ChatPanel notebook_id={workspace_id.get_untracked()} append_to_note=append_to_note />
                        </div>
                    </div>
                </main>

                // Desktop right rail
                <aside class="workspace-right-rail hidden md:block w-[328px] min-w-[328px] shrink-0 border-l border-border">
                    <div class="flex h-full flex-col px-4 py-3">
                        <div class="min-h-0 flex-[1.05] overflow-hidden">
                            <div class="h-full overflow-y-auto pr-1">
                                <WorkspaceDocumentPane
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
                                    set_docscope_initialized=set_docscope_initialized
                                    handle_delete_document=handle_delete_document
                                    handle_reindex_document=handle_reindex_document
                                />
                            </div>
                        </div>

                        <div class="my-2 h-px bg-border"></div>

                        <div class="min-h-0 flex-[0.95] overflow-hidden">
                            <div class="h-full overflow-y-auto">
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
                                    show_actions=true
                                />
                            </div>
                        </div>
                    </div>
                </aside>

                // Mobile right rail overlay
                <Show when=move || right_rail_open.get() && is_mobile()>
                    <div class="fixed inset-0 z-50 md:hidden" on:click=move |_| set_right_rail_open.set(false)>
                        <div class="absolute inset-0 bg-black/50"></div>
                        <aside class="workspace-right-rail absolute right-0 top-0 h-full w-[85vw] max-w-[26rem] overflow-y-auto shadow-xl" on:click=|ev| ev.stop_propagation()>
                            <div class="flex h-full flex-col p-4 pl-3">
                                <WorkspaceDocumentPane
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
                                    set_docscope_initialized=set_docscope_initialized
                                    handle_delete_document=handle_delete_document
                                    handle_reindex_document=handle_reindex_document
                                />

                                <div class="my-2 h-px bg-border"></div>

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
                                    show_actions=false
                                />
                            </div>
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
        />
    }
}
