use crate::transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
use contracts::chat::ChatRequest;

/// 浏览器 HTTP/SSE transport。空 `base_url` 表示同源 `/api/v1/chat`；
/// 显式 base URL 只用于隔离测试。凭据由调用方按请求注入，本类型不读取、
/// 不持久化任何浏览器存储。
pub struct BrowserHttpTransport {
    pub base_url: String,
    pub auth_token: Option<String>,
}

impl BrowserHttpTransport {
    pub fn new(base_url: &str, auth_token: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}/api/v1/chat", self.base_url)
    }

    pub fn build_headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("Content-Type", "application/json".to_string()),
            ("Accept", "text/event-stream".to_string()),
        ];
        if let Some(token) = self.auth_token.as_deref().filter(|t| !t.is_empty()) {
            headers.push(("Authorization", format!("Bearer {}", token)));
        }
        headers
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ChatTransport for BrowserHttpTransport {
    async fn stream_chat(
        &self,
        _request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        if cancellation.is_cancelled() {
            return Err(TransportError::Cancelled);
        }
        // 非浏览器路径不伪造空流成功（Gate C）
        Err(TransportError::Unavailable(
            "browser Fetch/SSE adapter only exists on wasm32 browser targets".to_string(),
        ))
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_fetch {
    use super::BrowserHttpTransport;
    use crate::sse_decoder::events_from_byte_stream;
    use crate::transport::{Cancellation, ChatEventStream, TransportError};
    use contracts::chat::{ChatEvent, ChatRequest};
    use futures_util::future::{Either, select};
    use futures_util::{Stream, StreamExt, pin_mut};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{AbortController, Headers, Request, RequestInit, RequestMode, Response};

    pub async fn stream_chat(
        transport: &BrowserHttpTransport,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        if cancellation.is_cancelled() {
            return Err(TransportError::Cancelled);
        }

        let body = serde_json::to_string(&request).map_err(TransportError::Serialization)?;

        let headers = Headers::new().map_err(network_error)?;
        for (name, value) in transport.build_headers() {
            headers.set(name, &value).map_err(network_error)?;
        }

        let controller = AbortController::new().map_err(network_error)?;
        let init = RequestInit::new();
        init.set_method("POST");
        init.set_mode(RequestMode::Cors);
        init.set_headers_headers(&headers);
        init.set_body_opt_str(Some(&body));
        init.set_signal(Some(&controller.signal()));

        let window = web_sys::window()
            .ok_or_else(|| TransportError::Unavailable("no window object".to_string()))?;
        let request = Request::new_with_str_and_init(&transport.endpoint(), &init)
            .map_err(network_error)?;
        let response: Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|error| {
                if is_abort_error(&error) || cancellation.is_cancelled() {
                    TransportError::Cancelled
                } else {
                    network_error(error)
                }
            })?
            .dyn_into()
            .map_err(network_error)?;

        // 非 2xx 在进入 SSE decoder 前映射为 typed error（body 有界截断）
        let status = response.status();
        if !(200..=299).contains(&status) {
            let body_text = read_bounded_text(&response).await;
            return Err(TransportError::from_http_status(status, body_text));
        }

        let raw_body = response.body().ok_or(TransportError::EmptyBody)?;
        let cancel_for_map = cancellation.clone();
        let byte_stream = wasm_streams::ReadableStream::from_raw(raw_body)
            .into_stream()
            .map(move |item| match item {
                Ok(value) => Ok(js_sys::Uint8Array::new(&value).to_vec()),
                Err(error) => {
                    if cancel_for_map.is_cancelled() || is_abort_error(&error) {
                        Err(TransportError::Cancelled)
                    } else {
                        Err(TransportError::Interrupted(describe_js(&error)))
                    }
                }
            });

        Ok(Box::pin(abortable_event_stream(
            events_from_byte_stream(byte_stream),
            cancellation,
            controller,
        )))
    }

    /// 消费回路：取消信号与事件流竞争。取消时真正 abort 底层 Fetch，
    /// 消费者得到且只得到一次 `Cancelled`；流被提前丢弃时守卫同样触发 abort，
    /// 释放 reader 与网络资源。
    fn abortable_event_stream<S>(
        events: S,
        cancellation: Cancellation,
        controller: AbortController,
    ) -> impl Stream<Item = Result<ChatEvent, TransportError>>
    where
        S: Stream<Item = Result<ChatEvent, TransportError>>,
    {
        async_stream::stream! {
            let _abort_guard = AbortOnDrop(Some(controller));
            let events = events;
            pin_mut!(events);
            loop {
                let next = events.next();
                let cancelled = cancellation.cancelled();
                pin_mut!(next);
                pin_mut!(cancelled);
                match select(next, cancelled).await {
                    Either::Left((Some(item), _)) => {
                        let terminal = matches!(item, Err(TransportError::Cancelled));
                        yield item;
                        if terminal {
                            return;
                        }
                    }
                    Either::Left((None, _)) => return,
                    Either::Right(((), _)) => {
                        // AbortOnDrop 在 return 时执行 controller.abort()
                        yield Err(TransportError::Cancelled);
                        return;
                    }
                }
            }
        }
    }

    struct AbortOnDrop(Option<AbortController>);

    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            if let Some(controller) = self.0.take() {
                controller.abort();
            }
        }
    }

    async fn read_bounded_text(response: &Response) -> String {
        let Ok(promise) = response.text() else {
            return String::new();
        };
        let text = JsFuture::from(promise)
            .await
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        text.chars()
            .take(crate::transport::MAX_ERROR_BODY_BYTES)
            .collect()
    }

    fn is_abort_error(value: &wasm_bindgen::JsValue) -> bool {
        js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str("name"))
            .ok()
            .and_then(|name| name.as_string())
            .as_deref()
            == Some("AbortError")
    }

    fn describe_js(value: &wasm_bindgen::JsValue) -> String {
        value
            .as_string()
            .or_else(|| {
                js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str("message"))
                    .ok()
                    .and_then(|message| message.as_string())
            })
            .unwrap_or_else(|| format!("{value:?}"))
    }

    fn network_error(value: wasm_bindgen::JsValue) -> TransportError {
        TransportError::Network(describe_js(&value))
    }
}

#[cfg(target_arch = "wasm32")]
impl ChatTransport for BrowserHttpTransport {
    async fn stream_chat(
        &self,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        wasm_fetch::stream_chat(self, request, cancellation).await
    }
}
