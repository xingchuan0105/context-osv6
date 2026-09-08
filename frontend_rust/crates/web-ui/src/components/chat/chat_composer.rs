// Adapted from Rust/UI Input Prompt; see frontend_rust/THIRD_PARTY.md.
use crate::components::chat::{ModelRoleBadge, ScopeBar, TurnAttachmentTray};
use crate::components::ui::{Button, ButtonVariant};
use crate::i18n::use_i18n;
use leptos::prelude::*;
use web_sdk::Capability;

/// Input presentation and DOM behavior. Conversation and upload policy stay with their owners.
#[component]
pub fn ChatComposer(
    model_role: Signal<String>,
    has_byok: Signal<bool>,
    value: RwSignal<String>,
    streaming: Signal<bool>,
    locked: Signal<bool>,
    can_retry: Signal<bool>,
    history_loading: RwSignal<bool>,
    files_blocked: RwSignal<bool>,
    ready_count: RwSignal<usize>,
    knowledge_chat: bool,
    attachments: RwSignal<Vec<contracts::chat::TurnAttachment>>,
    epoch: Signal<u64>,
    attach_disabled: Signal<bool>,
    capabilities: RwSignal<Vec<Capability>>,
    capabilities_manual: RwSignal<bool>,
    on_submit: Callback<()>,
    on_stop: Callback<()>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let composer_ref = NodeRef::<leptos::html::Textarea>::new();
    let composer_height = RwSignal::new(72_i32);
    let composing = RwSignal::new(false);
    let input_initialized = RwSignal::new(false);
    let send_disabled = Signal::derive(move || locked.get() || value.get().trim().is_empty());
    let focus = move || {
        if let Some(area) = composer_ref.get() {
            let _ = area.focus();
        }
    };
    let submit = move || {
        if send_disabled.get_untracked() || composing.get_untracked() {
            return;
        }
        on_submit.run(());
        focus();
    };

    // Adopt text entered into the SSR textarea before hydration; subsequent
    // programmatic edits (send, edit-message) synchronize the value and height.
    Effect::new(move |_| {
        let desired = value.get();
        #[cfg(target_arch = "wasm32")]
        if let Some(area) = composer_ref.get() {
            if !input_initialized.get_untracked() {
                input_initialized.set(true);
                let typed = area.value();
                if !typed.is_empty() && desired.is_empty() {
                    value.set(typed);
                    return;
                }
            }
            if area.value() != desired { area.set_value(&desired); }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (&desired, input_initialized);
        autosize_composer(composer_ref, composer_height);
    });

    view! {
        <form
            class="chat-composer"
            aria-label=move || i18n.t("chat.composerSendLabel")
            data-testid="chat-composer"
            data-knowledge=knowledge_chat
            data-ready=move || input_initialized.get().to_string()
            on:submit=move |ev| {
                ev.prevent_default();
                submit();
            }
        >
            <label class="sr-only" for="chat-composer-input">
                {move || i18n.t("chat.composerInputLabel")}
            </label>
            <textarea
                id="chat-composer-input"
                data-testid="composer-input"
                node_ref=composer_ref
                rows=2
                aria-describedby="chat-composer-hint"
                placeholder=move || i18n.t("chat.composerPlaceholder")
                on:input=move |ev| value.set(event_target_value(&ev))
                on:compositionstart=move |_| composing.set(true)
                on:compositionend=move |_| composing.set(false)
                on:keydown=move |ev| {
                    if ev.key() == "Enter" && !ev.shift_key()
                        && !ev.is_composing() && ev.key_code() != 229 && !composing.get_untracked()
                    {
                        ev.prevent_default();
                        submit();
                    }
                }
            ></textarea>
            <Show when=move || !knowledge_chat>
                <TurnAttachmentTray files=attachments files_blocked=files_blocked disabled=attach_disabled epoch=epoch/>
            </Show>
            <div class="chat-composer-footer">
                <ScopeBar
                    capabilities=capabilities capabilities_manual=capabilities_manual
                    ready_count=Signal::derive(move || ready_count.get()) disabled=locked
                    knowledge_chat=knowledge_chat
                />
                <div class="chat-composer-actions">
                    <a class="chat-model-settings" href="/settings?tab=providers" title=move || i18n.t("settings.tabs.providers")>
                        <ModelRoleBadge model_role=model_role has_byok=has_byok/>
                    </a>
                    <Show when=move || can_retry.get()>
                        <Button variant=ButtonVariant::Secondary test_id="retry-button"
                            on_click=Callback::new(move |_| { on_retry.run(()); focus(); })>
                            <span class="sr-only">{move || i18n.t("chat.retry")}</span>
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M20 7v5h-5M20 12a8 8 0 1 0-2 6"/></svg>
                        </Button>
                    </Show>
                    <Show when=move || streaming.get() fallback=move || view! {
                        <Button button_type="submit" disabled=send_disabled test_id="send-button">
                            <span class="sr-only">{move || i18n.t("workspaceSend")}</span>
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M12 19V5m-6 6 6-6 6 6"/></svg>
                        </Button>
                    }>
                        <Button test_id="stop-button"
                            on_click=Callback::new(move |_| { on_stop.run(()); focus(); })>
                            <span class="sr-only">{move || i18n.t("workspaceChatStop")}</span>
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
                        </Button>
                    </Show>
                </div>
            </div>
            <p id="chat-composer-hint" class=move || if history_loading.get() || files_blocked.get() || streaming.get() { "chat-composer-hint" } else { "sr-only" } role="status">
                {move || i18n.t(if history_loading.get() {
                    "chat.historyLoading"
                } else if files_blocked.get() {
                    "chat.composerFilesPending"
                } else if streaming.get() {
                    "chat.composerStreaming"
                } else {
                    "chat.composerShortcut"
                })}
            </p>
        </form>
    }
}

fn autosize_composer(
    composer_ref: NodeRef<leptos::html::Textarea>,
    composer_height: RwSignal<i32>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(area) = composer_ref.get() else {
            return;
        };
        let html: &web_sys::HtmlElement = &area;
        let _ = html.style().set_property("height", "auto");
        let height = if area.value().is_empty() {
            72
        } else {
            html.scroll_height().clamp(72, 240)
        };
        composer_height.set(height);
        let _ = html.style().set_property("height", &format!("{height}px"));
    }
    let _ = (composer_ref, composer_height);
}
