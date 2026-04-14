#[component]
fn WorkspaceDocumentPane(
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
    set_docscope_initialized: WriteSignal<bool>,
    handle_delete_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
    handle_reindex_document: StoredValue<Arc<dyn Fn(String) + Send + Sync>>,
) -> impl IntoView {
    view! {
        <Show
            when=move || selected_document.get().is_none()
            fallback=move || {
                selected_document
                    .get()
                    .map(|source| {
                        view! {
                            <div class="min-h-0 flex-[1.15]">
                                <DocumentDetail
                                    source=source
                                    on_close=move || set_selected_document.set(None)
                                    on_delete=move |document_id| {
                                        handle_delete_document.with_value(|callback| callback(document_id));
                                    }
                                    on_reindex=move |document_id| {
                                        handle_reindex_document.with_value(|callback| callback(document_id));
                                    }
                                />
                            </div>
                        }
                        .into_any()
                    })
                    .unwrap_or_else(|| view! { <></> }.into_any())
            }
        >
            <WorkspaceSourcesPane
                locale=locale
                chat=chat.get_value()
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
            />
        </Show>
    }
}
