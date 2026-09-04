use crate::transport::TransportError;
use contracts::chat::ChatMessageListResponse;
use contracts::workspaces::{ChatSession, ChatSessionListResponse};

#[cfg(target_arch = "wasm32")]
use crate::conversation_api::{
    parse_message_list, parse_session, parse_session_list, session_messages_url, session_url,
    sessions_url,
};

/// 浏览器只读 REST。空 `base_url` 表示同源 `/api/v1/chat/sessions*`。
/// native 不伪造成功；wasm32 走真实 Fetch GET。
pub struct BrowserRestClient {
    pub base_url: String,
    pub auth_token: Option<String>,
}

impl BrowserRestClient {
    pub fn new(base_url: &str, auth_token: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token,
        }
    }

}

#[cfg(not(target_arch = "wasm32"))]
impl BrowserRestClient {
    fn unavailable<T>(&self) -> Result<T, TransportError> {
        let _ = &self.auth_token;
        Err(TransportError::Unavailable(format!(
            "browser REST adapter only exists on wasm32 browser targets (base {})",
            self.base_url
        )))
    }

    pub async fn list_sessions(&self) -> Result<ChatSessionListResponse, TransportError> {
        self.unavailable()
    }

    pub async fn get_session(&self, _session_id: &str) -> Result<ChatSession, TransportError> {
        self.unavailable()
    }

    pub async fn list_messages(
        &self,
        _session_id: &str,
    ) -> Result<ChatMessageListResponse, TransportError> {
        self.unavailable()
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_get {
    use super::BrowserRestClient;
    use crate::transport::{MAX_ERROR_BODY_BYTES, TransportError};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

    pub async fn get_bytes(
        client: &BrowserRestClient,
        url: &str,
    ) -> Result<Vec<u8>, TransportError> {
        let headers = Headers::new().map_err(network_error)?;
        headers
            .set("Accept", "application/json")
            .map_err(network_error)?;
        if let Some(token) = client.auth_token.as_deref().filter(|token| !token.is_empty()) {
            headers
                .set("Authorization", &format!("Bearer {token}"))
                .map_err(network_error)?;
        }

        let init = RequestInit::new();
        init.set_method("GET");
        init.set_mode(RequestMode::Cors);
        init.set_headers_headers(&headers);

        let window = web_sys::window()
            .ok_or_else(|| TransportError::Unavailable("no window object".to_string()))?;
        let request = Request::new_with_str_and_init(url, &init).map_err(network_error)?;
        let response: Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(network_error)?
            .dyn_into()
            .map_err(network_error)?;

        let status = response.status();
        let body_text = read_bounded_text(&response).await;
        if !(200..=299).contains(&status) {
            return Err(TransportError::from_http_status(status, body_text));
        }
        if body_text.is_empty() {
            return Err(TransportError::EmptyBody);
        }
        Ok(body_text.into_bytes())
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
        text.chars().take(MAX_ERROR_BODY_BYTES).collect()
    }

    fn network_error(value: wasm_bindgen::JsValue) -> TransportError {
        let message = value
            .as_string()
            .or_else(|| {
                js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str("message"))
                    .ok()
                    .and_then(|inner| inner.as_string())
            })
            .unwrap_or_else(|| format!("{value:?}"));
        TransportError::Network(message)
    }
}

#[cfg(target_arch = "wasm32")]
impl BrowserRestClient {
    pub async fn list_sessions(&self) -> Result<ChatSessionListResponse, TransportError> {
        parse_session_list(&wasm_get::get_bytes(self, &sessions_url(&self.base_url)).await?)
    }

    pub async fn get_session(&self, session_id: &str) -> Result<ChatSession, TransportError> {
        parse_session(&wasm_get::get_bytes(self, &session_url(&self.base_url, session_id)).await?)
    }

    pub async fn list_messages(
        &self,
        session_id: &str,
    ) -> Result<ChatMessageListResponse, TransportError> {
        parse_message_list(
            &wasm_get::get_bytes(self, &session_messages_url(&self.base_url, session_id)).await?,
        )
    }
}
