use crate::transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
use contracts::chat::{ChatEvent, ChatRequest};

/// 桌面 IPC transport。事件信道与 Next `streamChatViaIPC` 对齐：
/// `invoke("chat_stream")` + `listen("chat://{request_id}")` + `invoke("chat_cancel")`。
///
/// native 无 Tauri 宿主：`new` 返回 `Unavailable`；`with_mock_events` 供 fixture 对等。
/// wasm32 在检测到 `window.__TAURI__` 时走真实 IPC。
pub struct TauriIpcTransport {
    #[allow(dead_code)] // wasm32 IPC path reads this; native mock/Unavailable does not
    auth_token: Option<String>,
    mock_events: Option<Vec<ChatEvent>>,
}

impl TauriIpcTransport {
    pub fn new(auth_token: Option<String>) -> Self {
        Self {
            auth_token,
            mock_events: None,
        }
    }

    pub fn with_mock_events(events: Vec<ChatEvent>) -> Self {
        Self {
            auth_token: None,
            mock_events: Some(events),
        }
    }
}

impl Default for TauriIpcTransport {
    fn default() -> Self {
        Self::new(None)
    }
}

/// 把 `ChatRequest` 序列化后写入桌面 host 需要的 `request_id` / `stream`。
/// 附加字段不是第二套 DTO：API 反序列化 `ChatRequest` 时忽略未知字段。
pub fn prepare_ipc_request(request: &ChatRequest, request_id: &str) -> serde_json::Value {
    let mut value = serde_json::to_value(request).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "request_id".to_string(),
            serde_json::Value::String(request_id.to_string()),
        );
        object.insert("stream".to_string(), serde_json::Value::Bool(true));
    }
    value
}

/// IPC 事件 payload → `ChatEvent`。与 fixture JSON 行同一 serde 路径。
pub fn parse_ipc_event(value: &serde_json::Value) -> Result<ChatEvent, TransportError> {
    serde_json::from_value(value.clone()).map_err(TransportError::Serialization)
}

pub fn map_ipc_error_status(status: u16, body: String) -> TransportError {
    TransportError::from_http_status(status, body)
}

impl ChatTransport for TauriIpcTransport {
    async fn stream_chat(
        &self,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        if cancellation.is_cancelled() {
            return Err(TransportError::Cancelled);
        }

        if let Some(events) = &self.mock_events {
            let events = events.clone();
            let cancel = cancellation.clone();
            let stream = async_stream::stream! {
                for event in events {
                    if cancel.is_cancelled() {
                        yield Err(TransportError::Cancelled);
                        return;
                    }
                    yield Ok(event);
                }
            };
            return Ok(Box::pin(stream));
        }

        #[cfg(target_arch = "wasm32")]
        {
            return wasm_ipc::stream_chat(self.auth_token.clone(), request, cancellation).await;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = request;
            Err(TransportError::Unavailable(
                "Tauri IPC adapter only exists inside a Tauri webview".to_string(),
            ))
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_ipc {
    use super::{map_ipc_error_status, parse_ipc_event, prepare_ipc_request};
    use crate::transport::{Cancellation, ChatEventStream, TransportError};
    use contracts::chat::ChatRequest;
    use futures_util::StreamExt;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen_futures::JsFuture;

    pub async fn stream_chat(
        auth_token: Option<String>,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        if !is_tauri_runtime() {
            return Err(TransportError::Unavailable(
                "window.__TAURI__ is not present".to_string(),
            ));
        }

        let request_id = new_request_id()?;
        let payload = prepare_ipc_request(&request, &request_id);
        let token = auth_token.unwrap_or_default();
        let channel = format!("chat://{request_id}");

        let (tx, rx) = futures_channel::mpsc::unbounded();
        let unlisten = listen_channel(&channel, tx.clone()).await?;

        let invoke_args = serde_json::json!({
            "token": token,
            "request": payload,
        });
        // invoke 会阻塞到宿主流结束；必须后台跑，否则 UI 只能在终态后一次性回放。
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(error) = invoke_command("chat_stream", &invoke_args).await {
                let _ = tx.unbounded_send(Err(error));
            }
            let _ = tx.unbounded_send(Ok(None));
        });

        let cancel = cancellation.clone();
        let request_id_for_cancel = request_id.clone();
        let stream = async_stream::stream! {
            let _unlisten = unlisten;
            let mut rx = rx;
            loop {
                if cancel.is_cancelled() {
                    let _ = invoke_command(
                        "chat_cancel",
                        &serde_json::json!({ "request_id": request_id_for_cancel }),
                    )
                    .await;
                    yield Err(TransportError::Cancelled);
                    return;
                }

                let next = rx.next();
                let cancelled = cancel.cancelled();
                futures_util::pin_mut!(next);
                futures_util::pin_mut!(cancelled);
                match futures_util::future::select(next, cancelled).await {
                    futures_util::future::Either::Left((Some(Ok(Some(event))), _)) => {
                        yield Ok(event);
                    }
                    futures_util::future::Either::Left((Some(Ok(None)), _)) => return,
                    futures_util::future::Either::Left((Some(Err(error)), _)) => {
                        yield Err(error);
                        return;
                    }
                    futures_util::future::Either::Left((None, _)) => return,
                    futures_util::future::Either::Right(((), _)) => {
                        let _ = invoke_command(
                            "chat_cancel",
                            &serde_json::json!({ "request_id": request_id_for_cancel }),
                        )
                        .await;
                        yield Err(TransportError::Cancelled);
                        return;
                    }
                }
            }
        };
        Ok(Box::pin(stream))
    }

    struct UnlistenGuard(Option<js_sys::Function>);

    impl Drop for UnlistenGuard {
        fn drop(&mut self) {
            if let Some(unlisten) = self.0.take() {
                let _ = unlisten.call0(&wasm_bindgen::JsValue::UNDEFINED);
            }
        }
    }

    pub fn is_tauri_runtime() -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let tauri = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("__TAURI__"))
            .ok()
            .filter(|value| !value.is_undefined() && !value.is_null());
        if tauri.is_some() {
            return true;
        }
        js_sys::Reflect::get(
            &window,
            &wasm_bindgen::JsValue::from_str("__TAURI_INTERNALS__"),
        )
        .ok()
        .is_some_and(|value| !value.is_undefined() && !value.is_null())
    }

    fn new_request_id() -> Result<String, TransportError> {
        let window = web_sys::window().ok_or_else(|| {
            TransportError::Unavailable("no window object".to_string())
        })?;
        let crypto = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("crypto"))
            .map_err(js_error)?;
        let func = js_sys::Reflect::get(&crypto, &wasm_bindgen::JsValue::from_str("randomUUID"))
            .map_err(js_error)?
            .dyn_into::<js_sys::Function>()
            .map_err(js_error)?;
        func.call0(&crypto)
            .map_err(js_error)?
            .as_string()
            .ok_or_else(|| TransportError::Unavailable("crypto.randomUUID returned empty".to_string()))
    }

    async fn listen_channel(
        channel: &str,
        tx: futures_channel::mpsc::UnboundedSender<Result<Option<contracts::chat::ChatEvent>, TransportError>>,
    ) -> Result<UnlistenGuard, TransportError> {
        let listen = tauri_event_fn("listen")?;
        let callback = Closure::wrap(Box::new(move |event: wasm_bindgen::JsValue| {
            let payload = js_sys::Reflect::get(&event, &wasm_bindgen::JsValue::from_str("payload"))
                .unwrap_or(event);
            let parsed = js_to_json(&payload).and_then(|value| parse_ipc_event(&value).map(Some));
            let _ = tx.unbounded_send(parsed);
        }) as Box<dyn FnMut(wasm_bindgen::JsValue)>);

        let promise = listen
            .call2(
                &tauri_event_ns()?,
                &wasm_bindgen::JsValue::from_str(channel),
                callback.as_ref().unchecked_ref(),
            )
            .map_err(js_error)?;
        let unlisten = JsFuture::from(
            promise
                .dyn_into::<js_sys::Promise>()
                .map_err(js_error)?,
        )
        .await
        .map_err(js_error)?
        .dyn_into::<js_sys::Function>()
        .map_err(js_error)?;
        callback.forget();
        Ok(UnlistenGuard(Some(unlisten)))
    }

    async fn invoke_command(command: &str, args: &serde_json::Value) -> Result<(), TransportError> {
        let invoke = tauri_core_fn("invoke")?;
        let js_args = json_to_js(args)?;
        let promise = invoke
            .call2(
                &tauri_core_ns()?,
                &wasm_bindgen::JsValue::from_str(command),
                &js_args,
            )
            .map_err(js_error)?;
        JsFuture::from(promise.dyn_into::<js_sys::Promise>().map_err(js_error)?)
            .await
            .map_err(map_rejected)?;
        Ok(())
    }

    fn tauri_root() -> Result<wasm_bindgen::JsValue, TransportError> {
        let window = web_sys::window().ok_or_else(|| {
            TransportError::Unavailable("no window object".to_string())
        })?;
        let value = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("__TAURI__"))
            .map_err(js_error)?;
        if value.is_undefined() || value.is_null() {
            return Err(TransportError::Unavailable(
                "window.__TAURI__ is not present".to_string(),
            ));
        }
        Ok(value)
    }

    fn tauri_core_ns() -> Result<wasm_bindgen::JsValue, TransportError> {
        let root = tauri_root()?;
        js_sys::Reflect::get(&root, &wasm_bindgen::JsValue::from_str("core")).map_err(js_error)
    }

    fn tauri_event_ns() -> Result<wasm_bindgen::JsValue, TransportError> {
        let root = tauri_root()?;
        js_sys::Reflect::get(&root, &wasm_bindgen::JsValue::from_str("event")).map_err(js_error)
    }

    fn tauri_core_fn(name: &str) -> Result<js_sys::Function, TransportError> {
        js_sys::Reflect::get(&tauri_core_ns()?, &wasm_bindgen::JsValue::from_str(name))
            .map_err(js_error)?
            .dyn_into::<js_sys::Function>()
            .map_err(js_error)
    }

    fn tauri_event_fn(name: &str) -> Result<js_sys::Function, TransportError> {
        js_sys::Reflect::get(&tauri_event_ns()?, &wasm_bindgen::JsValue::from_str(name))
            .map_err(js_error)?
            .dyn_into::<js_sys::Function>()
            .map_err(js_error)
    }

    fn json_to_js(value: &serde_json::Value) -> Result<wasm_bindgen::JsValue, TransportError> {
        let text = serde_json::to_string(value)?;
        js_sys::JSON::parse(&text).map_err(js_error)
    }

    fn js_to_json(value: &wasm_bindgen::JsValue) -> Result<serde_json::Value, TransportError> {
        let text = js_sys::JSON::stringify(value)
            .map_err(js_error)?
            .as_string()
            .unwrap_or_else(|| "null".to_string());
        serde_json::from_str(&text).map_err(TransportError::Serialization)
    }

    fn map_rejected(error: wasm_bindgen::JsValue) -> TransportError {
        if let Ok(value) = js_to_json(&error) {
            let status = value.get("status").and_then(serde_json::Value::as_u64);
            let message = value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tauri invoke failed")
                .to_string();
            if let Some(status) = status {
                return map_ipc_error_status(status as u16, message);
            }
            return TransportError::Unavailable(message);
        }
        js_error(error)
    }

    fn js_error(value: wasm_bindgen::JsValue) -> TransportError {
        let message = value
            .as_string()
            .or_else(|| {
                js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("message"))
                    .ok()
                    .and_then(|inner| inner.as_string())
            })
            .unwrap_or_else(|| format!("{value:?}"));
        TransportError::Unavailable(message)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_ipc::is_tauri_runtime;

#[cfg(not(target_arch = "wasm32"))]
pub fn is_tauri_runtime() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::chat::ChatRequest;

    fn sample_request() -> ChatRequest {
        serde_json::from_value(serde_json::json!({
            "query": "只回答：pong",
            "stream": true,
            "capabilities": [],
            "agent_type": "chat",
        }))
        .expect("sample ChatRequest")
    }

    #[test]
    fn prepare_ipc_request_adds_host_request_id() {
        let value = prepare_ipc_request(&sample_request(), "req-ipc-1");
        assert_eq!(value["request_id"], "req-ipc-1");
        assert_eq!(value["stream"], true);
        assert_eq!(value["query"], "只回答：pong");
        assert_eq!(value["capabilities"], serde_json::json!([]));
        assert_eq!(value["agent_type"], "chat");
    }

    #[test]
    fn parse_ipc_event_reads_the_same_fixture_json() {
        let raw = serde_json::json!({
            "event": "start",
            "request_id": "req-002",
            "session_id": "sess-200"
        });
        let event = parse_ipc_event(&raw).expect("start event");
        match event {
            ChatEvent::Start {
                request_id,
                session_id,
            } => {
                assert_eq!(request_id, "req-002");
                assert_eq!(session_id, "sess-200");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn map_ipc_error_status_matches_browser_transport() {
        assert!(matches!(
            map_ipc_error_status(401, String::new()),
            TransportError::Unauthorized
        ));
        assert!(matches!(
            map_ipc_error_status(429, String::new()),
            TransportError::RateLimited
        ));
    }
}
