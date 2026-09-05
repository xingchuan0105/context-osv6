use crate::api_base::poc_api_base;
use contracts::documents::SessionFileRow;
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use std::collections::HashSet;
use web_sdk::{
    SESSION_FILE_ACCEPT, TrayFileStatus, files_block_send, ready_file_count, tray_status,
    tray_status_attr, tray_status_label,
};

#[derive(Params, PartialEq, Clone, Debug)]
struct TrayParams {
    session_id: Option<String>,
}

#[component]
pub fn SessionFileTray(
    files_blocked: RwSignal<bool>,
    ready_count: RwSignal<usize>,
    disabled: Signal<bool>,
) -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<TrayParams>();
    let navigate = use_navigate();
    let files = RwSignal::new(Vec::<SessionFileRow>::new());
    let removed = RwSignal::new(HashSet::<String>::new());
    let action_error = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let refresh_gen = RwSignal::new(0_u64);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    let session_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.session_id.clone())
    });
    let bound_session = RwSignal::new(None::<Option<String>>);

    Effect::new(move |_| {
        let next = session_id.get();
        if bound_session.get_untracked().as_ref() == Some(&next) {
            return;
        }
        let prev = bound_session.get_untracked().flatten();
        bound_session.set(Some(next.clone()));
        // 首个上传会新建会话并改 URL；busy 期间不要把托盘和 in-flight refresh 作废。
        if prev.is_none() && next.is_some() && busy.get_untracked() {
            return;
        }
        files.set(Vec::new());
        action_error.set(false);
        removed.set(HashSet::new());
        refresh_gen.update(|n| *n += 1);
    });

    Effect::new(move |_| {
        let rows = files.get();
        files_blocked.set(files_block_send(&rows));
        ready_count.set(ready_file_count(&rows));
    });

    Effect::new(move |_| {
        let Some(active) = session_id.get() else {
            return;
        };
        let token_value = token.get();
        if token_value.is_empty() || busy.get() {
            return;
        }
        let _ = refresh_gen.get();
        spawn_refresh(files, removed, active, token_value, session_id, busy);
    });

    Effect::new(move |_| {
        let Some(active) = session_id.get() else {
            return;
        };
        let token_value = token.get();
        if token_value.is_empty() || !files_block_send(&files.get()) {
            return;
        }
        leptos::task::spawn_local(async move {
            sleep_ms(2000).await;
            spawn_refresh(files, removed, active, token_value, session_id, busy);
        });
    });

    let pick = move |_| {
        if disabled.get() || busy.get() {
            return;
        }
        if let Some(input) = input_ref.get() {
            input.click();
        }
    };

    #[cfg(target_arch = "wasm32")]
    {
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            for _ in 0..40 {
                if try_listen_file_input(
                    input_ref,
                    files,
                    removed,
                    action_error,
                    busy,
                    session_id,
                    token,
                    disabled,
                    navigate.clone(),
                ) {
                    return;
                }
                sleep_ms(50).await;
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = &navigate;
    }

    view! {
        <div class="chat-file-tray" data-testid="session-file-tray">
            <input
                node_ref=input_ref
                class="chat-file-input"
                type="file"
                accept=SESSION_FILE_ACCEPT
                multiple
                hidden
                data-testid="session-file-input"
            />
            <div class="chat-file-tray-row">
                <button
                    type="button"
                    class="chat-file-attach"
                    data-testid="session-file-attach"
                    disabled=move || disabled.get() || busy.get()
                    on:click=pick
                >
                    "添加文件"
                </button>
                <Show when=move || !files.get().is_empty()>
                    <span class="chat-file-tray-label">
                        {move || format!("会话文件 · {}", files.get().len())}
                    </span>
                </Show>
            </div>
            <Show when=move || action_error.get()>
                <p class="chat-file-error" role="alert" data-testid="session-file-error">
                    "文件操作失败，请重试。"
                </p>
            </Show>
            <Show when=move || files_blocked.get()>
                <p class="chat-file-blocked" data-testid="session-file-blocked">
                    "文件处理完成后才能发送消息；可移除未就绪的文件后继续。"
                </p>
            </Show>
            <Show when=move || !files.get().is_empty()>
                <ul class="chat-file-list">
                    <For
                        each=move || files.get()
                        key=|file| format!("{}:{}", file.binding_id, file.status)
                        children=move |file| {
                            file_row(file, files, removed, action_error, session_id, token, busy)
                        }
                    />
                </ul>
            </Show>
        </div>
    }
}

fn file_row(
    file: SessionFileRow,
    files: RwSignal<Vec<SessionFileRow>>,
    removed: RwSignal<HashSet<String>>,
    action_error: RwSignal<bool>,
    session_id: Signal<Option<String>>,
    token: RwSignal<String>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    let status = tray_status(&file.status);
    let status_attr = tray_status_attr(status);
    let label = tray_status_label(status);
    let failed = status == TrayFileStatus::Failed;
    let retry_file = file.clone();
    let remove_file = file.clone();
    let file_name = file.file_name.clone();
    view! {
        <li class="chat-file-item" data-status=status_attr data-testid="session-file-item">
            <span class="chat-file-name">{file_name}</span>
            <span class="chat-file-status" data-testid="session-file-status">{label}</span>
            <button
                type="button"
                class="chat-file-action"
                data-testid="session-file-retry"
                hidden=!failed
                on:click=move |_| {
                    if session_id.get().is_none() {
                        return;
                    }
                    spawn_retry(
                        files,
                        removed,
                        action_error,
                        session_id,
                        busy,
                        token.get(),
                        retry_file.clone(),
                    );
                }
            >
                "重试解析"
            </button>
            <button
                type="button"
                class="chat-file-action"
                data-testid="session-file-remove"
                on:click=move |_| {
                    if session_id.get().is_none() {
                        return;
                    }
                    spawn_remove(
                        files,
                        removed,
                        action_error,
                        session_id,
                        busy,
                        token.get(),
                        remove_file.clone(),
                    );
                }
            >
                "移除"
            </button>
        </li>
    }
}

#[cfg(target_arch = "wasm32")]
fn try_listen_file_input(
    input_ref: NodeRef<leptos::html::Input>,
    files: RwSignal<Vec<SessionFileRow>>,
    removed: RwSignal<HashSet<String>>,
    action_error: RwSignal<bool>,
    busy: RwSignal<bool>,
    session_id: Signal<Option<String>>,
    token: RwSignal<String>,
    disabled: Signal<bool>,
    navigate: impl Fn(&str, NavigateOptions) + Clone + 'static,
) -> bool {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    let Some(input) = input_ref.get() else {
        return false;
    };
    if input.get_attribute("data-listening").as_deref() == Some("true") {
        return true;
    }
    let navigate = navigate.clone();
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        if disabled.get() || busy.get() {
            return;
        }
        let Some(input) = input_ref.get() else {
            return;
        };
        let Some(list) = input.files() else {
            return;
        };
        let mut picked = Vec::new();
        for index in 0..list.length() {
            if let Some(file) = list.item(index) {
                picked.push(file);
            }
        }
        if picked.is_empty() {
            return;
        }
        let current = session_id.get();
        let token_value = token.get();
        input.set_value("");
        spawn_upload(
            files,
            removed,
            action_error,
            busy,
            session_id,
            current,
            token_value,
            picked,
            navigate.clone(),
        );
    });
    let _ = input.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    let _ = input.set_attribute("data-listening", "true");
    closure.forget();
    true
}

fn spawn_refresh(
    files: RwSignal<Vec<SessionFileRow>>,
    removed: RwSignal<HashSet<String>>,
    session_id: String,
    token: String,
    route_session: Signal<Option<String>>,
    busy: RwSignal<bool>,
) {
    leptos::task::spawn_local(async move {
        let client = web_sdk::BrowserRestClient::new(&poc_api_base(), Some(token));
        let Ok(list) = client.list_session_files(&session_id).await else {
            return;
        };
        if route_session
            .get_untracked()
            .is_some_and(|id| id != session_id)
        {
            return;
        }
        if list.files.is_empty()
            && (busy.get_untracked() || !files.get_untracked().is_empty())
        {
            return;
        }
        let tomb = removed.get_untracked();
        files.set(
            list.files
                .into_iter()
                .filter(|file| !tomb.contains(&file.binding_id))
                .collect(),
        );
    });
}

fn spawn_remove(
    files: RwSignal<Vec<SessionFileRow>>,
    removed: RwSignal<HashSet<String>>,
    action_error: RwSignal<bool>,
    route_session: Signal<Option<String>>,
    busy: RwSignal<bool>,
    token: String,
    file: SessionFileRow,
) {
    removed.update(|set| {
        set.insert(file.binding_id.clone());
    });
    files.update(|list| list.retain(|row| row.binding_id != file.binding_id));
    leptos::task::spawn_local(async move {
        let Some(session_id) = route_session.get_untracked() else {
            return;
        };
        let client = web_sdk::BrowserRestClient::new(&poc_api_base(), Some(token.clone()));
        if client
            .delete_session_file(&session_id, &file.binding_id)
            .await
            .is_err()
        {
            removed.update(|set| {
                set.remove(&file.binding_id);
            });
            files.update(|list| {
                if !list.iter().any(|row| row.binding_id == file.binding_id) {
                    list.insert(0, file);
                }
            });
            action_error.set(true);
            return;
        }
        spawn_refresh(files, removed, session_id, token, route_session, busy);
    });
}

fn spawn_retry(
    files: RwSignal<Vec<SessionFileRow>>,
    removed: RwSignal<HashSet<String>>,
    action_error: RwSignal<bool>,
    route_session: Signal<Option<String>>,
    busy: RwSignal<bool>,
    token: String,
    file: SessionFileRow,
) {
    leptos::task::spawn_local(async move {
        let client = web_sdk::BrowserRestClient::new(&poc_api_base(), Some(token.clone()));
        if client.reindex_document(&file.document_id).await.is_err() {
            action_error.set(true);
            return;
        }
        let Some(session_id) = route_session.get_untracked() else {
            return;
        };
        spawn_refresh(files, removed, session_id, token, route_session, busy);
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_upload(
    files: RwSignal<Vec<SessionFileRow>>,
    removed: RwSignal<HashSet<String>>,
    action_error: RwSignal<bool>,
    busy: RwSignal<bool>,
    route_session: Signal<Option<String>>,
    mut session_id: Option<String>,
    token: String,
    picked: Vec<web_sys::File>,
    navigate: impl Fn(&str, NavigateOptions) + Clone + 'static,
) {
    if token.is_empty() || picked.is_empty() {
        return;
    }
    busy.set(true);
    action_error.set(false);
    leptos::task::spawn_local(async move {
        let client = web_sdk::BrowserRestClient::new(&poc_api_base(), Some(token.clone()));
        for file in picked {
            if session_id.is_none() {
                match client.create_personal_session().await {
                    Ok(created) => {
                        let id = created.id.clone();
                        navigate(
                            &format!("/chat/{id}"),
                            NavigateOptions {
                                replace: true,
                                scroll: false,
                                ..Default::default()
                            },
                        );
                        session_id = Some(id);
                    }
                    Err(_) => {
                        action_error.set(true);
                        busy.set(false);
                        return;
                    }
                }
            }
            let Some(active) = session_id.clone() else {
                busy.set(false);
                return;
            };
            let Ok((name, mime, bytes)) = file_bytes(&file).await else {
                action_error.set(true);
                busy.set(false);
                return;
            };
            let upload = match client
                .create_session_file_upload(&active, &name, bytes.len() as u64, &mime)
                .await
            {
                Ok(upload) => upload,
                Err(_) => {
                    action_error.set(true);
                    busy.set(false);
                    return;
                }
            };
            files.update(|list| {
                if !list.iter().any(|row| row.document_id == upload.document_id) {
                    list.push(contracts::documents::SessionFileRow {
                        binding_id: format!("tmp-{}", upload.document_id),
                        document_id: upload.document_id.clone(),
                        file_name: name.clone(),
                        mime_type: mime.clone(),
                        file_size: bytes.len() as u64,
                        status: "pending".into(),
                        parse_version: None,
                        created_at: String::new(),
                    });
                }
            });
            spawn_refresh(
                files,
                removed,
                active.clone(),
                token.clone(),
                route_session,
                busy,
            );
            if client
                .put_upload_bytes(&upload.upload_url, &bytes, &mime)
                .await
                .is_err()
            {
                action_error.set(true);
                busy.set(false);
                return;
            }
            if client.complete_upload(&upload.document_id).await.is_err() {
                action_error.set(true);
                busy.set(false);
                return;
            }
            files.update(|list| {
                for row in list.iter_mut() {
                    if row.document_id == upload.document_id {
                        row.status = "completed".into();
                    }
                }
            });
            spawn_refresh(files, removed, active, token.clone(), route_session, busy);
        }
        busy.set(false);
        if let Some(active) = session_id.clone() {
            spawn_refresh(files, removed, active, token, route_session, busy);
        }
    });
}

#[cfg(target_arch = "wasm32")]
async fn file_bytes(file: &web_sys::File) -> Result<(String, String, Vec<u8>), ()> {
    let name = file.name();
    let mime = file.type_();
    let mime = if mime.is_empty() {
        "application/octet-stream".to_string()
    } else {
        mime
    };
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
        .await
        .map_err(|_| ())?;
    Ok((name, mime, js_sys::Uint8Array::new(&buffer).to_vec()))
}

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
