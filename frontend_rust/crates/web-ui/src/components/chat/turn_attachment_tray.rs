use crate::i18n::use_i18n;
use contracts::chat::TurnAttachment;
use leptos::prelude::*;

#[component]
pub fn TurnAttachmentTray(
    files: RwSignal<Vec<TurnAttachment>>,
    files_blocked: RwSignal<bool>,
    disabled: Signal<bool>,
    epoch: Signal<u64>,
) -> impl IntoView {
    let i18n = use_i18n();
    let token = expect_context::<RwSignal<String>>();
    let input = NodeRef::<leptos::html::Input>::new();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let last_epoch = RwSignal::new(None::<u64>);
    Effect::new(move |_| {
        let current = epoch.get();
        if last_epoch.get_untracked() == Some(current) {
            return;
        }
        last_epoch.set(Some(current));
        files.set(Vec::new());
        busy.set(false);
        error.set(None);
        files_blocked.set(false);
    });

    let on_change = move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            use contracts::chat::{
                MAX_TURN_ATTACHMENT_BYTES, MAX_TURN_ATTACHMENTS, MAX_TURN_CONTEXT_BYTES,
            };
            use wasm_bindgen_futures::JsFuture;
            let Some(el) = input.get() else {
                return;
            };
            let Some(picked) = el.files() else {
                return;
            };
            let picked: Vec<_> = (0..picked.length())
                .filter_map(|index| picked.get(index))
                .collect();
            el.set_value("");
            if picked.is_empty() || disabled.get_untracked() || busy.get_untracked() {
                return;
            }
            error.set(None);
            files_blocked.set(true);
            if files.get_untracked().len() + picked.len() > MAX_TURN_ATTACHMENTS
                || picked.iter().any(|file| {
                    file.size() == 0.0 || file.size() > MAX_TURN_ATTACHMENT_BYTES as f64
                })
            {
                error.set(Some(i18n.t("chat.attachmentLimits")));
                return;
            }
            let tok = token.get_untracked();
            if tok.is_empty() {
                error.set(Some(i18n.t("chat.attachmentLogin")));
                return;
            }
            busy.set(true);
            let generation = epoch.get_untracked();
            leptos::task::spawn_local(async move {
                let client =
                    web_sdk::BrowserRestClient::new(&crate::api_base::poc_api_base(), Some(tok));
                for file in picked {
                    let result = async {
                        let buffer = JsFuture::from(file.array_buffer()).await.map_err(|_| {
                            web_sdk::TransportError::Network("file read failed".into())
                        })?;
                        client
                            .parse_turn_attachment(
                                &file.name(),
                                &file.type_(),
                                &js_sys::Uint8Array::new(&buffer).to_vec(),
                            )
                            .await
                    }
                    .await;
                    if epoch.try_get_untracked() != Some(generation) {
                        return;
                    }
                    match result {
                        Ok(attachment) => {
                            let total = files.with_untracked(|items| {
                                items
                                    .iter()
                                    .map(|f| f.text.len() + f.filename.len())
                                    .sum::<usize>()
                            }) + attachment.text.len()
                                + attachment.filename.len();
                            if total > MAX_TURN_CONTEXT_BYTES {
                                error.set(Some(i18n.t("chat.attachmentContextLimit")));
                                break;
                            }
                            files.update(|items| items.push(attachment));
                        }
                        Err(err) => {
                            error.set(Some(i18n.tf(
                                "chat.attachmentFailed",
                                &[("name", &file.name()), ("error", &err.to_string())],
                            )));
                            break;
                        }
                    }
                }
                busy.set(false);
                files_blocked.set(error.get_untracked().is_some());
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = token;
    };
    view! {
        <div class="chat-file-tray" data-testid="turn-attachment-tray">
            <input node_ref=input type="file" multiple hidden class="chat-file-input" data-testid="turn-attachment-input"
                accept=".pdf,.doc,.docx,.xls,.xlsx,.xlsm,.xlsb,.ppt,.pptx,.pptm,.txt,.md,.csv,.tsv,.json,.png,.jpg,.jpeg,.webp,.gif,.bmp,.odt,.ods,.odp,.rtf"
                on:change=on_change/>
            <div class="chat-file-tray-row">
            <button type="button" class="chat-file-attach" data-testid="turn-attachment-add"
                aria-label=move || i18n.t("chat.attachFile")
                aria-describedby="turn-attachment-hint"
                title=move || i18n.t("chat.attachmentHint")
                disabled=move || disabled.get() || busy.get()
                on:click=move |_| { if let Some(el) = input.get() { el.click(); } }>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="m21 11-8 8a6 6 0 0 1-8.5-8.5l9-9a4 4 0 0 1 5.7 5.7l-9 9a2 2 0 0 1-2.8-2.8l8-8"/>
                </svg>
            </button>
            <span class="chat-attachment-scope">{move || i18n.t("chat.attachmentScope")}</span>
            </div>
            <p id="turn-attachment-hint" class="sr-only">{move || i18n.t("chat.attachmentHint")}</p>
            <Show when=move || busy.get()><p role="status">{move || i18n.t("chat.attachmentParsing")}</p></Show>
            <ul class="chat-file-list">
                {move || files.get().into_iter().enumerate().map(|(index, file)| view! {
                    <li class="chat-file-item" data-testid="turn-attachment-item">
                        <span class="chat-file-name" title=file.filename.clone()>{file.filename.clone()}</span>
                        <button type="button" class="chat-file-action" disabled=move || disabled.get() || busy.get()
                            on:click=move |_| files.update(|items| { if index < items.len() { items.remove(index); } })>{move || i18n.t("chat.attachmentRemove")}</button>
                    </li>
                }).collect_view()}
            </ul>
            {move || error.get().map(|message| view! {
                <p role="alert" class="chat-file-error" data-testid="turn-attachment-error">{message}</p>
                <button type="button" class="chat-file-dismiss" disabled=move || busy.get() || disabled.get()
                    on:click=move |_| { error.set(None); files_blocked.set(false); }>{move || i18n.t("chat.attachmentDismissFailure")}</button>
            })}
        </div>
    }
}
