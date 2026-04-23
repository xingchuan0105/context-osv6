#[component]
fn WorkspaceNotesPane(
    locale: ReadSignal<crate::i18n::Locale>,
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
    show_actions: bool,
) -> impl IntoView {
    let (export_error, set_export_error) = signal(String::new());
    let active_note = Signal::derive(move || {
        active_note_id.get().and_then(|note_id| {
            notes
                .get()
                .into_iter()
                .find(|note| note.id == note_id)
        })
    });

    view! {
        <div class={format!("app-pane {}", workspace_ui_style::notes_pane)}>
            <div class="app-pane-header">
                <div class=workspace_ui_style::pane_header_row>
                    <h2 class="app-pane-title">
                        {move || choose(locale.get(), "笔记", "Notes")}
                    </h2>
                    <span class="workspace-count-pill">
                        {move || notes.get().len()}
                    </span>
                </div>
            </div>

            <div class={format!("app-pane-body {}", workspace_ui_style::pane_body_notes)}>
                <Show when=move || notes_loading.get()>
                    <div class="workspace-empty-state">
                        {move || choose(locale.get(), "正在加载笔记...", "Loading notes...")}
                    </div>
                </Show>

                <Show when=move || active_note.get().is_none() && !notes_loading.get()>
                    <div class=workspace_ui_style::notes_list_scroll>
                        <button
                            type="button"
                            class=workspace_ui_style::primary_action_button
                            on:click=move |_| handle_create_note.with_value(|callback| callback(String::new()))
                        >
                            <span class=workspace_ui_style::primary_label_icon>{"+"}</span>
                            <span>{move || choose(locale.get(), "新建笔记", "New Note")}</span>
                        </button>

                        <Show when=move || !notes.get().is_empty() fallback=move || view! {
                            <div class="workspace-empty-state">
                                {move || choose(locale.get(), "还没有保存的笔记，先记下一条想法吧。", "No saved notes yet. Capture your first idea to get started.")}
                            </div>
                        }>
                            <div class=workspace_ui_style::saved_notes_group>
                                <div class=workspace_ui_style::section_label>
                                    {move || choose(locale.get(), "已保存笔记", "Saved Notes")}
                                </div>
                                {move || {
                                    sort_workspace_notes(&notes.get())
                                        .into_iter()
                                        .map(|note| {
                                            let open_note_id = note.id.clone();
                                            let delete_note_id = note.id.clone();
                                            view! {
                                                <div class="workspace-note-card">
                                                    <div class=workspace_ui_style::note_row>
                                                        <button
                                                            type="button"
                                                            class=workspace_ui_style::note_open_button
                                                            on:click=move |_| set_active_note_id.set(Some(open_note_id.clone()))
                                                        >
                                                            <div class=workspace_ui_style::note_item_title>
                                                                {note.title.clone()}
                                                            </div>
                                                            <div class=workspace_ui_style::note_item_preview>
                                                                {if note.preview.is_empty() {
                                                                    choose(locale.get(), "空白笔记", "Empty note").to_string()
                                                                } else {
                                                                    note.preview.clone()
                                                                }}
                                                            </div>
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="workspace-note-delete"
                                                            on:click=move |_| {
                                                                handle_delete_note.with_value(|callback| callback(delete_note_id.clone()));
                                                            }
                                                        >
                                                            {move || choose(locale.get(), "删除", "Delete")}
                                                        </button>
                                                    </div>
                                                    <div class=workspace_ui_style::note_item_meta>
                                                        <span>{move || choose(locale.get(), "已保存", "Saved")}</span>
                                                        <span>{"·"}</span>
                                                        <span>{note.updated_at.clone()}</span>
                                                        <Show when=move || note.promoted_document_id.is_some()>
                                                            <span>{"·"}</span>
                                                            <span>{move || choose(locale.get(), "已转内容源", "Promoted")}</span>
                                                        </Show>
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()
                                }}
                            </div>
                        </Show>
                    </div>
                </Show>

                <Show when=move || active_note.get().is_some()>
                    {move || active_note.get().map(|note| {
                        let note_id_for_promote = StoredValue::new(note.id.clone());
                        let note_id_for_delete = StoredValue::new(note.id.clone());
                        let note_updated_for_export = StoredValue::new(note.updated_at.clone());
                        view! {
                        <div class=workspace_ui_style::note_detail_header>
                            <button
                                type="button"
                                class={format!("app-button-ghost {}", workspace_ui_style::compact_action)}
                                on:click=move |_| set_active_note_id.set(None)
                            >
                                {move || choose(locale.get(), "返回笔记列表", "Back to notes")}
                            </button>
                            <StatusBadge
                                label=match note_sync_state.get() {
                                    DraftSyncState::Idle => choose(locale.get(), "未同步", "Not synced").to_string(),
                                    DraftSyncState::Syncing => choose(locale.get(), "同步中", "Syncing").to_string(),
                                    DraftSyncState::Synced => choose(locale.get(), "已同步", "Synced").to_string(),
                                    DraftSyncState::Error => choose(locale.get(), "同步失败", "Sync failed").to_string(),
                                }
                                tone=match note_sync_state.get() {
                                    DraftSyncState::Idle => NoticeTone::Neutral,
                                    DraftSyncState::Syncing => NoticeTone::Info,
                                    DraftSyncState::Synced => NoticeTone::Success,
                                    DraftSyncState::Error => NoticeTone::Danger,
                                }
                            />
                        </div>

                        <Show when=move || show_actions>
                            <div class=workspace_ui_style::note_actions>
                                <button
                                    type="button"
                                    class={format!("app-button-secondary {}", workspace_ui_style::compact_action)}
                                    on:click=move |_| {
                                        let note_id = note_id_for_promote.with_value(|id| id.clone());
                                        handle_promote_note
                                            .with_value(|callback| callback(note_id));
                                    }
                                >
                                    {move || choose(locale.get(), "转为内容源", "Promote to Source")}
                                </button>
                                <button
                                    type="button"
                                    class={format!("app-button-secondary {}", workspace_ui_style::compact_action)}
                                    on:click=move |_| {
                                        let title = note_title.get_untracked();
                                        let content = note_content.get_untracked();
                                        let updated_at = note_updated_for_export
                                            .with_value(|value| value.clone());
                                        let markdown = render_note_markdown(
                                            &title,
                                            &content,
                                            Some(updated_at.as_str()),
                                        );
                                        let filename = sanitize_markdown_filename(&title);
                                        match export_text_file(&filename, &markdown) {
                                            Ok(_) => set_export_error.set(String::new()),
                                            Err(error) => set_export_error.set(format!(
                                                "{}: {}",
                                                choose(
                                                    locale.get_untracked(),
                                                    "导出 Markdown 失败",
                                                    "Failed to export markdown",
                                                ),
                                                error,
                                            )),
                                        }
                                    }
                                >
                                    {move || choose(locale.get(), "导出 Markdown", "Export Markdown")}
                                </button>
                                <button
                                    type="button"
                                    class={format!("app-button-danger {}", workspace_ui_style::compact_action)}
                                    on:click=move |_| {
                                        let note_id = note_id_for_delete.with_value(|id| id.clone());
                                        handle_delete_note.with_value(|callback| callback(note_id));
                                    }
                                >
                                    {move || choose(locale.get(), "删除", "Delete")}
                                </button>
                            </div>
                        </Show>

                        <Show when=move || !export_error.get().is_empty()>
                            <NoticeBanner message=export_error.get() tone=NoticeTone::Danger />
                        </Show>

                        <input
                            type="text"
                            class="app-input"
                            placeholder={move || choose(locale.get(), "给这条笔记起个名字", "Give this note a title")}
                            prop:value=move || note_title.get()
                            on:input=move |ev| {
                                set_note_title.set(event_target_value(&ev));
                                set_note_sync_revision.update(|value| *value += 1);
                            }
                        />

                        <div class=workspace_ui_style::note_editor_wrap>
                            <textarea
                                class={format!("app-input {}", workspace_ui_style::note_textarea)}
                                placeholder={move || choose(locale.get(), "记录结论、引用、待办和灵感...", "Capture findings, citations, next steps, and ideas...")}
                                prop:value=move || note_content.get()
                                on:input=move |ev| {
                                    set_note_content.set(event_target_value(&ev));
                                    set_note_sync_revision.update(|value| *value += 1);
                                }
                            ></textarea>
                        </div>
                    }})}
                </Show>
            </div>
        </div>
    }
}
