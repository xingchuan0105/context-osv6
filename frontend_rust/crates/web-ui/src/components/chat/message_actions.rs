use crate::api_base::poc_api_base;
use leptos::prelude::*;
use web_sdk::BrowserRestClient;

#[component]
pub fn MessageActions(
    content: String,
    session_id: Option<String>,
    message_id: Option<i64>,
) -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let copied = RwSignal::new(false);
    let rating = RwSignal::new(None::<&'static str>);
    let feedback_error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    let copy_content = content.clone();
    let on_copy = move |_| {
        copy_to_clipboard(copy_content.clone());
        copied.set(true);
        leptos::task::spawn_local(async move {
            sleep_ms(2000).await;
            copied.set(false);
        });
    };

    let sid_for_actions = session_id.clone();
    let has_ids = session_id.is_some() && message_id.is_some();

    view! {
        <div class="chat-message-actions" data-testid="message-actions">
            <button
                type="button"
                class="chat-action-button"
                data-testid="copy-answer-button"
                aria-label="复制回答"
                on:click=on_copy
            >
                {move || if copied.get() { "已复制" } else { "复制" }}
            </button>
            {has_ids.then(move || {
                let sid_up = sid_for_actions.clone();
                let sid_down = sid_for_actions.clone();
                view! {
                    <button
                        type="button"
                        class=move || {
                            if rating.get() == Some("up") {
                                "chat-action-button is-active"
                            } else {
                                "chat-action-button"
                            }
                        }
                        data-testid="feedback-up"
                        aria-label="赞同回答"
                        disabled=move || busy.get()
                        on:click=move |_| {
                            trigger_feedback(
                                sid_up.clone(),
                                message_id,
                                "up",
                                token,
                                busy,
                                rating,
                                feedback_error,
                            )
                        }
                    >
                        "赞"
                    </button>
                    <button
                        type="button"
                        class=move || {
                            if rating.get() == Some("down") {
                                "chat-action-button is-active"
                            } else {
                                "chat-action-button"
                            }
                        }
                        data-testid="feedback-down"
                        aria-label="踩回答"
                        disabled=move || busy.get()
                        on:click=move |_| {
                            trigger_feedback(
                                sid_down.clone(),
                                message_id,
                                "down",
                                token,
                                busy,
                                rating,
                                feedback_error,
                            )
                        }
                    >
                        "踩"
                    </button>
                }
            })}
            {move || {
                feedback_error.get().map(|err| {
                    view! {
                        <span class="chat-feedback-error" role="alert" data-testid="feedback-error">
                            {err}
                        </span>
                    }
                })
            }}
        </div>
    }
}

fn trigger_feedback(
    session_id: Option<String>,
    message_id: Option<i64>,
    new_rating: &'static str,
    token: RwSignal<String>,
    busy: RwSignal<bool>,
    rating: RwSignal<Option<&'static str>>,
    feedback_error: RwSignal<Option<String>>,
) {
    if busy.get() {
        return;
    }
    let Some(sid) = session_id else {
        return;
    };
    let Some(mid) = message_id else {
        return;
    };
    let token_val = token.get_untracked();
    if token_val.is_empty() {
        return;
    }

    busy.set(true);
    feedback_error.set(None);
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(token_val));
        match client.submit_feedback(&sid, mid, new_rating).await {
            Ok(_) => {
                rating.set(Some(new_rating));
                feedback_error.set(None);
            }
            Err(_) => {
                feedback_error.set(Some("反馈提交失败，请稍后重试".to_string()));
            }
        }
        busy.set(false);
    });
}

#[cfg(target_arch = "wasm32")]
fn copy_to_clipboard(text: String) {
    if let Some(window) = web_sys::window() {
        let promise = window.navigator().clipboard().write_text(&text);
        wasm_bindgen_futures::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_to_clipboard(_text: String) {}

#[cfg(target_arch = "wasm32")]
async fn sleep_ms(ms: i32) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep_ms(_ms: i32) {}
