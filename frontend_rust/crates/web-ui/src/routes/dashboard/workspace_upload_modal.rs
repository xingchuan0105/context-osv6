#[component]
fn WorkspaceUploadModal(
    workspace_id: Memo<String>,
    show_upload_modal: ReadSignal<bool>,
    set_show_upload_modal: WriteSignal<bool>,
    on_upload_success: StoredValue<Arc<dyn Fn() + Send + Sync>>,
    url_source: ReadSignal<String>,
    set_url_source: WriteSignal<String>,
    adding_url_source: ReadSignal<bool>,
    handle_add_url_source: StoredValue<Arc<dyn Fn() + Send + Sync>>,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let (source_tab, set_source_tab) = signal("file".to_string());

    view! {
        <Show when=move || show_upload_modal.get()>
            <div class=workspace_ui_style::upload_overlay>
                <button
                    type="button"
                    class=workspace_ui_style::upload_backdrop
                    on:click=move |_| set_show_upload_modal.set(false)
                ></button>

                <div class=workspace_ui_style::upload_shell>
                    <div class=workspace_ui_style::upload_header>
                        <div>
                            <div class=workspace_ui_style::upload_title>
                                {move || choose(locale.get(), "添加新资料", "Add New Source")}
                            </div>
                            <div class=workspace_ui_style::upload_subtitle>
                                {move || choose(locale.get(), "将文件、链接或笔记整理后的文本加入当前 Workspace。", "Bring files, links, or note-derived text into this workspace.")}
                            </div>
                        </div>
                        <button
                            type="button"
                            class=workspace_ui_style::upload_close_button
                            on:click=move |_| set_show_upload_modal.set(false)
                        >
                            <svg class=workspace_ui_style::upload_close_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                            </svg>
                        </button>
                    </div>

                    <div class=workspace_ui_style::upload_tabs>
                        <button
                            type="button"
                            class=workspace_ui_style::upload_tab
                            class=(workspace_ui_style::upload_tab_active, move || source_tab.get() == "file")
                            on:click=move |_| set_source_tab.set("file".to_string())
                        >
                            {move || choose(locale.get(), "上传文件", "Upload File")}
                        </button>
                        <button
                            type="button"
                            class=workspace_ui_style::upload_tab
                            class=(workspace_ui_style::upload_tab_active, move || source_tab.get() == "link")
                            on:click=move |_| set_source_tab.set("link".to_string())
                        >
                            {move || choose(locale.get(), "网页链接", "Web Link")}
                        </button>
                        <button
                            type="button"
                            class=workspace_ui_style::upload_tab
                            class=(workspace_ui_style::upload_tab_active, move || source_tab.get() == "text")
                            on:click=move |_| set_source_tab.set("text".to_string())
                        >
                            {move || choose(locale.get(), "粘贴文本", "Paste Text")}
                        </button>
                    </div>

                    <div class=workspace_ui_style::upload_body>
                        <Show when=move || source_tab.get() == "file">
                            <DocumentUpload
                                notebook_id={workspace_id.get()}
                                on_upload_success=move |_document_id| {
                                    set_show_upload_modal.set(false);
                                    on_upload_success.with_value(|callback| callback());
                                }
                                on_cancel_request=move || {
                                    set_show_upload_modal.set(false);
                                }
                            />
                        </Show>

                        <Show when=move || source_tab.get() == "link">
                            <div class=workspace_ui_style::upload_link_panel>
                                <p class=workspace_ui_style::upload_link_desc>
                                    {move || choose(locale.get(), "Context OS 会抓取并索引这个 Workspace 对应的目标页面。", "Context OS will fetch and index the target page for this workspace.")}
                                </p>
                                <input
                                    type="url"
                                    class={format!("workspace-input {}", workspace_ui_style::upload_link_input)}
                                    placeholder="https://example.com/article"
                                    prop:value=move || url_source.get()
                                    on:input=move |ev| set_url_source.set(event_target_value(&ev))
                                />
                                <button
                                    type="button"
                                    class={format!("{} {}", workspace_ui_style::primary_action_button, workspace_ui_style::upload_link_action)}
                                    disabled=move || adding_url_source.get() || url_source.get().trim().is_empty()
                                    on:click=move |_| handle_add_url_source.with_value(|callback| callback())
                                >
                                    {move || {
                                        if adding_url_source.get() {
                                            choose(locale.get(), "添加中...", "Adding...")
                                        } else {
                                            choose(locale.get(), "添加链接", "Add Link")
                                        }
                                    }}
                                </button>
                            </div>
                        </Show>

                        <Show when=move || source_tab.get() == "text">
                            <div class=workspace_ui_style::upload_text_panel>
                                <div class=workspace_ui_style::upload_text_title>
                                    {move || choose(locale.get(), "粘贴文本", "Paste Text")}
                                </div>
                                <p class=workspace_ui_style::upload_text_desc>
                                    {move || choose(locale.get(), "文本直接转资料源还没有单独 API。当前建议先写入 Notes，再用“提升为资料源”保持正式入库链路一致。", "Direct text-to-source import is not wired to a dedicated API yet. In the live workspace, use Notes and then Promote to Source to keep the ingestion flow canonical.")}
                                </p>
                                <button
                                    type="button"
                                    class=workspace_ui_style::upload_text_action
                                    disabled=true
                                >
                                    {move || choose(locale.get(), "改用笔记提升", "Use note promotion")}
                                </button>
                            </div>
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
    }
}
