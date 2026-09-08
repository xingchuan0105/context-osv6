// Adapted from Rust/UI Input Prompt; see frontend_rust/THIRD_PARTY.md.
use crate::components::chat::{ScopeBar, TurnAttachmentTray};
use crate::components::ui::{Button, ButtonVariant};
use crate::i18n::use_i18n;
use leptos::prelude::*;
use web_sdk::Capability;

/// Input presentation and DOM behavior. Conversation and upload policy stay with their owners.
#[component]
pub fn ChatComposer(
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
    let composer_height = RwSignal::new(96_i32);
    let resize_origin = RwSignal::new(None::<(i32, i32)>);
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
            data-ready=move || input_initialized.get().to_string()
            on:submit=move |ev| {
                ev.prevent_default();
                submit();
            }
        >
            <label class="chat-composer-label" for="chat-composer-input">
                {move || i18n.t("chat.composerInputLabel")}
            </label>
            <textarea
                id="chat-composer-input"
                data-testid="composer-input"
                node_ref=composer_ref
                rows=3
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
            <div
                class="chat-composer-resize"
                role="slider"
                tabindex="0"
                aria-label=move || i18n.t("workspaceChatComposerResize")
                aria-orientation="vertical"
                aria-controls="chat-composer-input"
                aria-valuemin="72"
                aria-valuemax="320"
                aria-valuenow=move || composer_height.get().to_string()
                data-testid="composer-resize"
                on:pointerdown=move |ev| start_composer_resize(ev, composer_ref, composer_height, resize_origin)
                on:pointermove=move |ev| continue_composer_resize(ev, composer_ref, composer_height, resize_origin)
                on:pointerup=move |_| resize_origin.set(None)
                on:pointercancel=move |_| resize_origin.set(None)
                on:keydown=move |ev| nudge_composer_height(ev, composer_ref, composer_height)
            ></div>
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
                    <Show when=move || can_retry.get()>
                        <Button variant=ButtonVariant::Secondary test_id="retry-button"
                            on_click=Callback::new(move |_| { on_retry.run(()); focus(); })>
                            {move || i18n.t("chat.retry")}
                        </Button>
                    </Show>
                    <Show when=move || streaming.get() fallback=move || view! {
                        <Button button_type="submit" disabled=send_disabled test_id="send-button">
                            {move || i18n.t("workspaceSend")}
                        </Button>
                    }>
                        <Button test_id="stop-button"
                            on_click=Callback::new(move |_| { on_stop.run(()); focus(); })>
                            {move || i18n.t("workspaceChatStop")}
                        </Button>
                    </Show>
                </div>
            </div>
            <p id="chat-composer-hint" class="chat-composer-hint" role="status">
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

fn apply_composer_height(composer_ref: NodeRef<leptos::html::Textarea>, px: i32) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(area) = composer_ref.get() {
            let html: &web_sys::HtmlElement = &area;
            let _ = html.style().set_property("height", &format!("{px}px"));
        }
    }
    let _ = (composer_ref, px);
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
            96
        } else {
            html.scroll_height().clamp(72, 240)
        };
        composer_height.set(height);
        let _ = html.style().set_property("height", &format!("{height}px"));
    }
    let _ = (composer_ref, composer_height);
}

fn start_composer_resize(
    ev: leptos::ev::PointerEvent,
    composer_ref: NodeRef<leptos::html::Textarea>,
    composer_height: RwSignal<i32>,
    resize_origin: RwSignal<Option<(i32, i32)>>,
) {
    ev.prevent_default();
    let start_h = composer_height.get_untracked();
    resize_origin.set(Some((ev.client_y() as i32, start_h)));
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        if let Some(target) = ev.current_target() {
            if let Ok(el) = target.dyn_into::<web_sys::Element>() {
                let _ = el.set_pointer_capture(ev.pointer_id());
            }
        }
        apply_composer_height(composer_ref, start_h);
    }
    let _ = composer_ref;
}

fn continue_composer_resize(
    ev: leptos::ev::PointerEvent,
    composer_ref: NodeRef<leptos::html::Textarea>,
    composer_height: RwSignal<i32>,
    resize_origin: RwSignal<Option<(i32, i32)>>,
) {
    let Some((start_y, start_h)) = resize_origin.get() else {
        return;
    };
    let next = (start_h + (start_y - ev.client_y() as i32)).clamp(72, 320);
    composer_height.set(next);
    apply_composer_height(composer_ref, next);
}

fn nudge_composer_height(
    ev: leptos::ev::KeyboardEvent,
    composer_ref: NodeRef<leptos::html::Textarea>,
    composer_height: RwSignal<i32>,
) {
    let delta = match ev.key().as_str() {
        "ArrowUp" => 16,
        "ArrowDown" => -16,
        _ => return,
    };
    ev.prevent_default();
    let next = (composer_height.get_untracked() + delta).clamp(72, 320);
    composer_height.set(next);
    apply_composer_height(composer_ref, next);
}
