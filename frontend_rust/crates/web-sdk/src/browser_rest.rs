use crate::transport::TransportError;
use contracts::chat::ChatMessageListResponse;
use contracts::documents::{CreateDocumentUploadResponse, SessionFilesResponse};
use contracts::workspaces::{ChatSession, ChatSessionListResponse};

#[cfg(target_arch = "wasm32")]
use crate::conversation_api::{
    parse_message_list, parse_session, parse_session_list, session_messages_url, session_url,
    sessions_url,
};
#[cfg(target_arch = "wasm32")]
use crate::session_files::{
    complete_upload_url, create_session_json, create_upload_json, parse_session_files,
    parse_upload_response, reindex_document_url, session_file_url, session_files_url,
};

/// 浏览器 REST。空 `base_url` 表示同源 `/api/v1/chat/sessions*`。
/// native 不伪造成功；wasm32 走真实 Fetch。
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

    pub async fn create_personal_session(&self) -> Result<ChatSession, TransportError> {
        self.unavailable()
    }

    pub async fn list_session_files(
        &self,
        _session_id: &str,
    ) -> Result<SessionFilesResponse, TransportError> {
        self.unavailable()
    }

    pub async fn create_session_file_upload(
        &self,
        _session_id: &str,
        _filename: &str,
        _file_size: u64,
        _mime_type: &str,
    ) -> Result<CreateDocumentUploadResponse, TransportError> {
        self.unavailable()
    }

    pub async fn put_upload_bytes(
        &self,
        _upload_url: &str,
        _bytes: &[u8],
        _mime_type: &str,
    ) -> Result<(), TransportError> {
        self.unavailable()
    }

    pub async fn complete_upload(&self, _document_id: &str) -> Result<(), TransportError> {
        self.unavailable()
    }

    pub async fn delete_session_file(
        &self,
        _session_id: &str,
        _binding_id: &str,
    ) -> Result<(), TransportError> {
        self.unavailable()
    }

    pub async fn reindex_document(&self, _document_id: &str) -> Result<(), TransportError> {
        self.unavailable()
    }

    pub async fn list_provider_secrets(
        &self,
    ) -> Result<crate::providers::ProviderSecretsResponse, TransportError> {
        self.unavailable()
    }

    pub async fn submit_feedback(
        &self,
        _session_id: &str,
        _message_id: i64,
        _rating: &str,
    ) -> Result<(), TransportError> {
        self.unavailable()
    }

    pub async fn upsert_provider_secret(
        &self,
        _provider: &str,
        _api_key: &str,
        _purpose: &str,
        _model_hint: Option<&str>,
    ) -> Result<(), TransportError> {
        self.unavailable()
    }

    pub async fn revoke_provider_secret(&self, _id: &str) -> Result<(), TransportError> {
        self.unavailable()
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_request {
    use super::BrowserRestClient;
    use crate::transport::{MAX_ERROR_BODY_BYTES, TransportError};
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

    pub async fn get_bytes(
        client: &BrowserRestClient,
        url: &str,
    ) -> Result<Vec<u8>, TransportError> {
        request_bytes(client, "GET", url, None, None, true).await
    }

    pub async fn request_bytes(
        client: &BrowserRestClient,
        method: &str,
        url: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
        send_auth: bool,
    ) -> Result<Vec<u8>, TransportError> {
        let headers = Headers::new().map_err(network_error)?;
        headers
            .set("Accept", "application/json")
            .map_err(network_error)?;
        if let Some(content_type) = content_type {
            headers
                .set("Content-Type", content_type)
                .map_err(network_error)?;
        }
        if send_auth {
            if let Some(token) = client.auth_token.as_deref().filter(|token| !token.is_empty()) {
                headers
                    .set("Authorization", &format!("Bearer {token}"))
                    .map_err(network_error)?;
            }
        }

        let init = RequestInit::new();
        init.set_method(method);
        init.set_mode(RequestMode::Cors);
        init.set_headers_headers(&headers);
        let json_text;
        if let Some(bytes) = body {
            if content_type == Some("application/json") {
                json_text = String::from_utf8_lossy(bytes).into_owned();
                init.set_body_opt_str(Some(json_text.as_str()));
            } else {
                let parts = js_sys::Array::new();
                parts.push(&js_sys::Uint8Array::from(bytes));
                let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)
                    .map_err(network_error)?;
                init.set_body(blob.as_ref());
            }
        }

        let window = web_sys::window()
            .ok_or_else(|| TransportError::Unavailable("no window object".to_string()))?;
        let request = Request::new_with_str_and_init(url, &init).map_err(network_error)?;
        let response: Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(network_error)?
            .dyn_into()
            .map_err(network_error)?;

        let status = response.status();
        if status == 204 || status == 205 {
            return Ok(Vec::new());
        }
        let body_text = read_bounded_text(&response).await;
        if !(200..=299).contains(&status) {
            return Err(TransportError::from_http_status(status, body_text));
        }
        if body_text.is_empty() {
            if method == "GET" {
                return Err(TransportError::EmptyBody);
            }
            return Ok(Vec::new());
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
        parse_session_list(&wasm_request::get_bytes(self, &sessions_url(&self.base_url)).await?)
    }

    pub async fn get_session(&self, session_id: &str) -> Result<ChatSession, TransportError> {
        parse_session(
            &wasm_request::get_bytes(self, &session_url(&self.base_url, session_id)).await?,
        )
    }

    pub async fn list_messages(
        &self,
        session_id: &str,
    ) -> Result<ChatMessageListResponse, TransportError> {
        parse_message_list(
            &wasm_request::get_bytes(self, &session_messages_url(&self.base_url, session_id))
                .await?,
        )
    }

    pub async fn create_personal_session(&self) -> Result<ChatSession, TransportError> {
        parse_session(
            &wasm_request::request_bytes(
                self,
                "POST",
                &sessions_url(&self.base_url),
                Some(&create_session_json()),
                Some("application/json"),
                true,
            )
            .await?,
        )
    }

    pub async fn list_session_files(
        &self,
        session_id: &str,
    ) -> Result<SessionFilesResponse, TransportError> {
        parse_session_files(
            &wasm_request::get_bytes(self, &session_files_url(&self.base_url, session_id)).await?,
        )
    }

    pub async fn create_session_file_upload(
        &self,
        session_id: &str,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<CreateDocumentUploadResponse, TransportError> {
        let body = create_upload_json(filename, file_size, mime_type)?;
        parse_upload_response(
            &wasm_request::request_bytes(
                self,
                "POST",
                &session_files_url(&self.base_url, session_id),
                Some(&body),
                Some("application/json"),
                true,
            )
            .await?,
        )
    }

    pub async fn put_upload_bytes(
        &self,
        upload_url: &str,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<(), TransportError> {
        let resolved = crate::session_files::resolve_upload_url(&self.base_url, upload_url);
        wasm_request::request_bytes(
            self,
            "PUT",
            &resolved,
            Some(bytes),
            Some(mime_type),
            false,
        )
        .await?;
        Ok(())
    }

    pub async fn complete_upload(&self, document_id: &str) -> Result<(), TransportError> {
        wasm_request::request_bytes(
            self,
            "POST",
            &complete_upload_url(&self.base_url, document_id),
            None,
            None,
            true,
        )
        .await?;
        Ok(())
    }

    pub async fn delete_session_file(
        &self,
        session_id: &str,
        binding_id: &str,
    ) -> Result<(), TransportError> {
        wasm_request::request_bytes(
            self,
            "DELETE",
            &session_file_url(&self.base_url, session_id, binding_id),
            None,
            None,
            true,
        )
        .await?;
        Ok(())
    }

    pub async fn reindex_document(&self, document_id: &str) -> Result<(), TransportError> {
        wasm_request::request_bytes(
            self,
            "POST",
            &reindex_document_url(&self.base_url, document_id),
            None,
            None,
            true,
        )
        .await?;
        Ok(())
    }

    pub async fn list_provider_secrets(
        &self,
    ) -> Result<crate::providers::ProviderSecretsResponse, TransportError> {
        crate::providers::parse_provider_secrets(
            &wasm_request::get_bytes(self, &crate::providers::provider_secrets_url(&self.base_url))
                .await?,
        )
    }

    pub async fn submit_feedback(
        &self,
        session_id: &str,
        message_id: i64,
        rating: &str,
    ) -> Result<(), TransportError> {
        let body = serde_json::to_vec(&contracts::chat::MessageFeedbackRequest {
            session_id: session_id.to_string(),
            message_id,
            rating: match rating {
                "down" => contracts::chat::MessageFeedbackRating::Down,
                _ => contracts::chat::MessageFeedbackRating::Up,
            },
        })?;
        wasm_request::request_bytes(
            self,
            "POST",
            &crate::conversation_api::message_feedback_url(&self.base_url, session_id, message_id),
            Some(&body),
            Some("application/json"),
            true,
        )
        .await?;
        Ok(())
    }

    pub async fn upsert_provider_secret(
        &self,
        provider: &str,
        api_key: &str,
        purpose: &str,
        model_hint: Option<&str>,
    ) -> Result<(), TransportError> {
        let payload = serde_json::json!({
            "provider": provider,
            "api_key": api_key,
            "purpose": purpose,
            "model_hint": model_hint,
        });
        let body = serde_json::to_vec(&payload)?;
        wasm_request::request_bytes(
            self,
            "PUT",
            &crate::providers::provider_secrets_url(&self.base_url),
            Some(&body),
            Some("application/json"),
            true,
        )
        .await?;
        Ok(())
    }

    pub async fn revoke_provider_secret(&self, id: &str) -> Result<(), TransportError> {
        let trimmed = crate::conversation_api::trim_base_url(&self.base_url);
        let url = format!("{trimmed}/api/v1/settings/provider-secrets/{id}");
        wasm_request::request_bytes(self, "DELETE", &url, None, None, true).await?;
        Ok(())
    }
}
