#[component]
fn WorkspaceSourcesPane(
    locale: ReadSignal<crate::i18n::Locale>,
    chat: crate::state::chat::ChatState,
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
    handle_add_url_source: StoredValue<Arc<dyn Fn() + Send + Sync + 'static>>,
    handle_toggle_source_pin: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    set_docscope_initialized: WriteSignal<bool>,
) -> impl IntoView {
    let ready_source_ids = Signal::derive(move || {
        sources
            .get()
            .into_iter()
            .filter(|source| source_status_docscope_eligible(&source.status))
            .map(|source| source.id)
            .collect::<Vec<_>>()
    });

    view! {
        <div class="app-pane min-h-0 flex-[1.15]">
            <div class="app-pane-header">
                <div>
                    <h2 class="app-pane-title">
                        {move || choose(locale.get(), "资料", "Sources")}
                    </h2>
                    <p class="app-pane-meta">
                        {move || choose(locale.get(), "选择当前对话可用的知识范围", "Select the source scope for the current thread")}
                    </p>
                </div>
                <Show when=move || ui_capabilities().document_upload>
                    <button
                        class="app-button-primary"
                        on:click=move |_| set_show_upload_modal.set(true)
                    >
                        {move || choose(locale.get(), "新建资料", "New Source")}
                    </button>
                </Show>
            </div>

            <div class="app-pane-body flex min-h-0 flex-col">
                <div class="space-y-3 border-b border-border px-4 py-4">
                    <div class="flex gap-2">
                        <input
                            type="url"
                            class="app-input flex-1"
                            placeholder={move || choose(locale.get(), "添加网页链接作为资料...", "Add a web URL as a source...")}
                            value=move || url_source.get()
                            on:input=move |ev| set_url_source.set(event_target_value(&ev))
                        />
                        <button
                            class="app-button-secondary shrink-0"
                            disabled=move || adding_url_source.get()
                            on:click=move |_| handle_add_url_source.with_value(|callback| callback())
                        >
                            {move || {
                                if adding_url_source.get() {
                                    choose(locale.get(), "添加中...", "Adding...")
                                } else {
                                    choose(locale.get(), "添加 URL", "Add URL")
                                }
                            }}
                        </button>
                    </div>

                    <div class="flex flex-wrap items-center gap-2">
                        {move || view! {
                            <StatusBadge
                                label=format!(
                                    "{} {}",
                                    selected_source_ids.get().len(),
                                    choose(locale.get(), "已选", "selected")
                                )
                                tone=NoticeTone::Info
                            />
                        }}
                        {move || view! {
                            <StatusBadge
                                label=format!(
                                    "{} {}",
                                    sources.get().len(),
                                    choose(locale.get(), "总数", "total")
                                )
                                tone=NoticeTone::Neutral
                            />
                        }}
                        {move || view! {
                            <StatusBadge
                                label=format!(
                                    "{} {}",
                                    pinned_source_ids.get().len(),
                                    choose(locale.get(), "固定", "pinned")
                                )
                                tone=NoticeTone::Neutral
                            />
                        }}
                        <Show when=move || status_polling.get()>
                            {move || view! {
                                <StatusBadge
                                    label=choose(locale.get(), "索引更新中", "Indexing").to_string()
                                    tone=NoticeTone::Warning
                                />
                            }}
                        </Show>
                        <Show when=move || !ready_source_ids.get().is_empty()>
                            <button
                                type="button"
                                class="app-button-ghost text-xs"
                                on:click=move |_| {
                                    let ready = ready_source_ids.get_untracked();
                                    if selected_source_ids.get_untracked().len() == ready.len() {
                                        set_selected_source_ids.set(Vec::new());
                                    } else {
                                        set_selected_source_ids.set(ready);
                                    }
                                    set_docscope_initialized.set(true);
                                }
                            >
                                {move || {
                                    if selected_source_ids.get().len() == ready_source_ids.get().len() {
                                        choose(locale.get(), "取消全选", "Clear all")
                                    } else {
                                        choose(locale.get(), "全选", "Select all")
                                    }
                                }}
                            </button>
                        </Show>
                    </div>
                </div>

                <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
                    <Show when=move || !chat.citations.get().is_empty()>
                        <div class="mb-4 rounded-2xl border border-border bg-muted/40 p-3">
                            <EvidencePanel />
                        </div>
                    </Show>

                    <Show when=move || sources_loading.get()>
                        <div class="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
                            {move || choose(locale.get(), "正在加载资料...", "Loading sources...")}
                        </div>
                    </Show>

                    <Show when=move || !sources_loading.get() && sources.get().is_empty()>
                        <div class="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
                            {move || choose(locale.get(), "还没有资料，先上传文件或添加链接。", "No sources yet. Upload a file or add a URL to begin.")}
                        </div>
                    </Show>

                    <div class="space-y-2">
                        <For
                            each=move || sort_workspace_sources(&sources.get(), &pinned_source_ids.get())
                            key=|source| source.id.clone()
                            children=move |source| {
                                let source_id = source.id.clone();
                                let checked_source_id = source.id.clone();
                                let pin_source_id = source.id.clone();
                                let checkbox_disabled = !source_status_docscope_eligible(&source.status);
                                view! {
                                    <DocumentListItem
                                        source=source.clone()
                                        is_selected={selected_document
                                            .get()
                                            .as_ref()
                                            .map(|selected| selected.id == source_id)
                                            .unwrap_or(false)}
                                        is_checked={selected_source_ids
                                            .get()
                                            .iter()
                                            .any(|id| id == &checked_source_id)}
                                        is_pinned={pinned_source_ids
                                            .get()
                                            .iter()
                                            .any(|id| id == &pin_source_id)}
                                        checkbox_disabled=checkbox_disabled
                                        on_click=move |item| set_selected_document.set(Some(item))
                                        on_toggle_checked=move |next_source_id, checked| {
                                            set_selected_source_ids.update(|selected| {
                                                if checked {
                                                    if !selected.iter().any(|id| id == &next_source_id) {
                                                        selected.push(next_source_id.clone());
                                                    }
                                                } else {
                                                    selected.retain(|id| id != &next_source_id);
                                                }
                                            });
                                            set_docscope_initialized.set(true);
                                        }
                                        on_toggle_pinned=move |next_source_id| {
                                            handle_toggle_source_pin
                                                .with_value(|callback| callback(next_source_id));
                                        }
                                    />
                                }
                            }
                        />
                    </div>
                </div>
            </div>
        </div>
    }
}
