#[component]
fn WorkspaceUploadModal(
    workspace_id: Memo<String>,
    show_upload_modal: ReadSignal<bool>,
    set_show_upload_modal: WriteSignal<bool>,
    on_upload_success: StoredValue<Arc<dyn Fn() + Send + Sync>>,
) -> impl IntoView {
    view! {
        <Show when=move || ui_capabilities().document_upload && show_upload_modal.get()>
            <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
                <div class="app-surface-card w-full max-w-md mx-4">
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
                </div>
            </div>
        </Show>
    }
}
