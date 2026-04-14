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
        <div class="app-pane min-h-0 flex-[0.9] overflow-hidden">
            <div class="app-pane-header">
                <div>
                    <h2 class="app-pane-title">
                        {move || choose(locale.get(), "笔记", "Notes")}
                    </h2>
                    <p class="app-pane-meta">
                        {move || choose(locale.get(), "个人工作笔记，仅对你可见", "Private workspace notes visible only to you")}
                    </p>
                </div>
                <button
                    type="button"
                    class="app-button-secondary"
                    on:click=move |_| handle_create_note.with_value(|callback| callback(String::new()))
                >
                    {move || choose(locale.get(), "新建笔记", "New Note")}
                </button>
            </div>

            <div class="app-pane-body flex min-h-0 flex-col gap-4 p-4">
                <Show when=move || notes_loading.get()>
                    <div class="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
                        {move || choose(locale.get(), "正在加载笔记...", "Loading notes...")}
                    </div>
                </Show>

                <Show when=move || active_note.get().is_none() && !notes_loading.get()>
                    <div class="space-y-3 overflow-y-auto pr-1">
                        <Show when=move || !notes.get().is_empty() fallback=move || view! {
                            <div class="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
                                {move || choose(locale.get(), "还没有保存的笔记，先记下一条想法吧。", "No saved notes yet. Capture your first idea to get started.")}
                            </div>
                        }>
                            <div class="space-y-3">
                                {move || {
                                    sort_workspace_notes(&notes.get())
                                        .into_iter()
                                        .map(|note| {
                                            let open_note_id = note.id.clone();
                                            let delete_note_id = note.id.clone();
                                            view! {
                                                <button
                                                    type="button"
                                                    class="w-full rounded-2xl border border-border bg-card/70 px-4 py-3 text-left transition-colors hover:bg-muted/50"
                                                    on:click=move |_| set_active_note_id.set(Some(open_note_id.clone()))
                                                >
                                                    <div class="flex items-start justify-between gap-3">
                                                        <div class="min-w-0 flex-1">
                                                            <div class="truncate text-sm font-semibold text-foreground">
                                                                {note.title.clone()}
                                                            </div>
                                                            <div class="mt-1 line-clamp-3 text-xs leading-5 text-muted-foreground">
                                                                {if note.preview.is_empty() {
                                                                    choose(locale.get(), "空白笔记", "Empty note").to_string()
                                                                } else {
                                                                    note.preview.clone()
                                                                }}
                                                            </div>
                                                        </div>
                                                        <button
                                                            type="button"
                                                            class="rounded-lg px-2 py-1 text-xs text-muted-foreground hover:bg-red-50 hover:text-red-600"
                                                            on:click=move |ev| {
                                                                ev.stop_propagation();
                                                                handle_delete_note.with_value(|callback| callback(delete_note_id.clone()));
                                                            }
                                                        >
                                                            {move || choose(locale.get(), "删除", "Delete")}
                                                        </button>
                                                    </div>
                                                    <div class="mt-3 flex items-center gap-2 text-[11px] text-muted-foreground">
                                                        <span>{move || choose(locale.get(), "已保存", "Saved")}</span>
                                                        <span>{"·"}</span>
                                                        <span>{note.updated_at.clone()}</span>
                                                        <Show when=move || note.promoted_document_id.is_some()>
                                                            <span>{"·"}</span>
                                                            <span>{move || choose(locale.get(), "已转内容源", "Promoted")}</span>
                                                        </Show>
                                                    </div>
                                                </button>
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
                        <div class="flex items-center justify-between gap-3">
                            <button
                                type="button"
                                class="app-button-ghost text-xs"
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
                            <div class="flex items-center gap-2">
                                <button
                                    type="button"
                                    class="app-button-secondary text-xs"
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
                                    class="app-button-secondary text-xs"
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
                                    class="app-button-danger text-xs"
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

                        <div class="min-h-0 flex-1">
                            <textarea
                                class="app-input h-full min-h-[220px] resize-none"
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
