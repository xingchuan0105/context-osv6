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
    let _ = (
        url_source,
        set_url_source,
        adding_url_source,
        handle_add_url_source,
    );
    let ready_source_ids = Signal::derive(move || {
        sources
            .get()
            .into_iter()
            .filter(|source| source_status_docscope_eligible(&source.status))
            .map(|source| source.id)
            .collect::<Vec<_>>()
    });
    let all_selected = Signal::derive(move || {
        let ready = ready_source_ids.get();
        !ready.is_empty() && selected_source_ids.get().len() == ready.len()
    });

    view! {
        <div class={format!("app-pane {}", workspace_ui_style::sources_pane)}>
            <div class="app-pane-header">
                <div class=workspace_ui_style::pane_header_row>
                    <h2 class="app-pane-title">
                        {move || choose(locale.get(), "资料", "Sources")}
                    </h2>
                    <span class="workspace-count-pill">
                        {move || sources.get().len()}
                    </span>
                </div>
            </div>

            <div class={format!("app-pane-body {}", workspace_ui_style::pane_body_sources)}>
                <div class=workspace_ui_style::sources_toolbar>
                    <Show when=move || ui_capabilities().document_upload>
                        <button
                            class=workspace_ui_style::primary_action_button
                            on:click=move |_| set_show_upload_modal.set(true)
                        >
                            <span class=workspace_ui_style::primary_label_icon>{"+"}</span>
                            <span>{move || choose(locale.get(), "新建资料", "New Source")}</span>
                        </button>
                    </Show>

                    <Show when=move || !ready_source_ids.get().is_empty()>
                        <label class={format!("workspace-select-row {}", workspace_ui_style::select_row)}>
                            <span>{move || choose(locale.get(), "全选", "Select all")}</span>
                            <input
                                type="checkbox"
                                class=workspace_ui_style::select_checkbox
                                prop:checked=move || all_selected.get()
                                on:change=move |_| {
                                    let ready = ready_source_ids.get_untracked();
                                    if all_selected.get_untracked() {
                                        set_selected_source_ids.set(Vec::new());
                                    } else {
                                        set_selected_source_ids.set(ready);
                                    }
                                    set_docscope_initialized.set(true);
                                }
                            />
                        </label>
                    </Show>

                    <Show when=move || status_polling.get()>
                        <div class="workspace-inline-status">
                            {move || choose(locale.get(), "索引更新中", "Indexing")}
                        </div>
                    </Show>
                </div>

                <div class=workspace_ui_style::sources_scroll>
                    <Show when=move || chat.active_citation.get().is_some()>
                        <div class=workspace_ui_style::evidence_shell>
                            <EvidencePanel />
                        </div>
                    </Show>

                    <Show when=move || sources_loading.get()>
                        <div class="workspace-empty-state">
                            {move || choose(locale.get(), "正在加载资料...", "Loading sources...")}
                        </div>
                    </Show>

                    <Show when=move || !sources_loading.get() && sources.get().is_empty()>
                        <div class="workspace-empty-state">
                            {move || choose(locale.get(), "还没有资料，先上传文件或添加链接。", "No sources yet. Upload a file or add a URL to begin.")}
                        </div>
                    </Show>

                    <div class=workspace_ui_style::stack_compact>
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
