use crate::auth::PersistedAuth;
use crate::transport::TransportError;

#[cfg(not(target_arch = "wasm32"))]
pub fn read_browser_auth() -> Option<PersistedAuth> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_browser_auth(_auth: &PersistedAuth) {}

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_browser_auth() {}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_me(_base_url: &str, _token: &str) -> Result<crate::auth::AuthUser, TransportError> {
    Err(TransportError::Unavailable(
        "auth me adapter only exists on wasm32 browser targets".to_string(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn auth_login(
    _base_url: &str,
    _email: &str,
    _password: &str,
) -> Result<contracts::auth::AuthPayload, TransportError> {
    Err(TransportError::Unavailable(
        "auth login adapter only exists on wasm32 browser targets".to_string(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn auth_register(
    _base_url: &str,
    _email: &str,
    _password: &str,
    _full_name: Option<&str>,
) -> Result<contracts::auth::AuthPayload, TransportError> {
    Err(TransportError::Unavailable(
        "auth register adapter only exists on wasm32 browser targets".to_string(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn auth_reset_send_code(_base_url: &str, _email: &str) -> Result<(), TransportError> {
    Err(TransportError::Unavailable(
        "reset send code adapter only exists on wasm32 browser targets".to_string(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn auth_reset_verify_code(
    _base_url: &str,
    _email: &str,
    _code: &str,
) -> Result<String, TransportError> {
    Err(TransportError::Unavailable(
        "reset verify code adapter only exists on wasm32 browser targets".to_string(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn auth_reset_confirm(
    _base_url: &str,
    _ticket: &str,
    _new_password: &str,
) -> Result<(), TransportError> {
    Err(TransportError::Unavailable(
        "reset confirm adapter only exists on wasm32 browser targets".to_string(),
    ))
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use crate::auth::{
        AUTH_BOOTSTRAP_TIMEOUT_MS, AUTH_STORAGE_KEY, auth_me_url, parse_auth_me_json,
        parse_persisted_auth_cookie, parse_persisted_auth_json, persisted_clear_cookie,
        persisted_set_cookie, session_hint_clear_cookie, session_hint_set_cookie,
    };
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{AbortController, Headers, HtmlDocument, Request, RequestInit, RequestMode};

    fn is_secure() -> bool {
        web_sys::window()
            .and_then(|window| window.location().protocol().ok())
            .is_some_and(|protocol| protocol == "https:")
    }

    fn document() -> Option<HtmlDocument> {
        web_sys::window()?
            .document()?
            .dyn_into::<HtmlDocument>()
            .ok()
    }

    fn cookie_header() -> String {
        document()
            .and_then(|doc| doc.cookie().ok())
            .unwrap_or_default()
    }

    fn set_cookie(value: &str) {
        if let Some(doc) = document() {
            let _ = doc.set_cookie(value);
        }
    }

    pub fn read_browser_auth() -> Option<PersistedAuth> {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                match storage.get_item(AUTH_STORAGE_KEY) {
                    Ok(Some(raw)) => {
                        if let Some(parsed) = parse_persisted_auth_json(&raw) {
                            return Some(parsed);
                        }
                    }
                    Ok(None) => {}
                    Err(_) => {}
                }
            }
        }
        parse_persisted_auth_cookie(&cookie_header())
    }

    pub fn write_browser_auth(auth: &PersistedAuth) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(raw) = serde_json::to_string(auth) {
                    let _ = storage.set_item(AUTH_STORAGE_KEY, &raw);
                }
            }
        }
        let secure = is_secure();
        set_cookie(&persisted_set_cookie(auth, secure));
        set_cookie(&session_hint_set_cookie(secure));
    }

    pub fn clear_browser_auth() {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.remove_item(AUTH_STORAGE_KEY);
            }
        }
        let secure = is_secure();
        set_cookie(&persisted_clear_cookie(secure));
        set_cookie(&session_hint_clear_cookie(secure));
    }

    pub async fn fetch_me(
        base_url: &str,
        token: &str,
    ) -> Result<crate::auth::AuthUser, TransportError> {
        let url = auth_me_url(base_url);
        let headers = Headers::new().map_err(network_error)?;
        headers
            .set("Accept", "application/json")
            .map_err(network_error)?;
        headers
            .set("Authorization", &format!("Bearer {token}"))
            .map_err(network_error)?;

        let window = web_sys::window()
            .ok_or_else(|| TransportError::Unavailable("no window object".to_string()))?;
        let controller = AbortController::new().map_err(network_error)?;
        let signal = controller.signal();
        let timeout = Closure::once(move || controller.abort());
        let timeout_id = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout.as_ref().unchecked_ref(),
                AUTH_BOOTSTRAP_TIMEOUT_MS,
            )
            .map_err(network_error)?;

        let init = RequestInit::new();
        init.set_method("GET");
        init.set_mode(RequestMode::Cors);
        init.set_headers_headers(&headers);
        init.set_signal(Some(&signal));

        let request = Request::new_with_str_and_init(&url, &init).map_err(network_error)?;
        let response = JsFuture::from(window.fetch_with_request(&request)).await;
        window.clear_timeout_with_handle(timeout_id);
        drop(timeout);
        let response: web_sys::Response = response.map_err(network_error)?.dyn_into().map_err(network_error)?;
        let status = response.status();
        let body = read_text(&response).await;
        if !(200..=299).contains(&status) {
            return Err(TransportError::from_http_status(status, body));
        }
        parse_auth_me_json(body.as_bytes()).ok_or(TransportError::Unauthorized)
    }

    async fn read_text(response: &web_sys::Response) -> String {
        let Ok(promise) = response.text() else {
            return String::new();
        };
        JsFuture::from(promise)
            .await
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default()
    }

    async fn post_json_envelope<T: serde::de::DeserializeOwned>(
        url: &str,
        body_bytes: &[u8],
    ) -> Result<T, TransportError> {
        let headers = Headers::new().map_err(network_error)?;
        headers
            .set("Accept", "application/json")
            .map_err(network_error)?;
        headers
            .set("Content-Type", "application/json")
            .map_err(network_error)?;

        let window = web_sys::window()
            .ok_or_else(|| TransportError::Unavailable("no window object".to_string()))?;
        let init = RequestInit::new();
        init.set_method("POST");
        init.set_mode(RequestMode::Cors);
        init.set_headers_headers(&headers);
        let body_str = String::from_utf8_lossy(body_bytes);
        init.set_body_opt_str(Some(&body_str));

        let request = Request::new_with_str_and_init(url, &init).map_err(network_error)?;
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(network_error)?;
        let response: web_sys::Response = response.dyn_into().map_err(network_error)?;
        let status = response.status();
        let body = read_text(&response).await;
        if !(200..=299).contains(&status) {
            let error_msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| body.clone());
            if !error_msg.is_empty() {
                return Err(TransportError::Network(error_msg));
            }
            return Err(TransportError::from_http_status(status, body));
        }
        let env: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| TransportError::Framing(format!("invalid json: {e}")))?;
        if env.get("success") == Some(&serde_json::Value::Bool(false)) {
            let msg = env.get("error").and_then(|e| e.as_str()).unwrap_or("failed").to_string();
            return Err(TransportError::Network(msg));
        }
        let data_val = env.get("data").cloned().unwrap_or(serde_json::Value::Null);
        serde_json::from_value(data_val).map_err(|e| TransportError::Framing(e.to_string()))
    }

    async fn post_json_void(url: &str, body_bytes: &[u8]) -> Result<(), TransportError> {
        let headers = Headers::new().map_err(network_error)?;
        headers
            .set("Accept", "application/json")
            .map_err(network_error)?;
        headers
            .set("Content-Type", "application/json")
            .map_err(network_error)?;

        let window = web_sys::window()
            .ok_or_else(|| TransportError::Unavailable("no window object".to_string()))?;
        let init = RequestInit::new();
        init.set_method("POST");
        init.set_mode(RequestMode::Cors);
        init.set_headers_headers(&headers);
        let body_str = String::from_utf8_lossy(body_bytes);
        init.set_body_opt_str(Some(&body_str));

        let request = Request::new_with_str_and_init(url, &init).map_err(network_error)?;
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(network_error)?;
        let response: web_sys::Response = response.dyn_into().map_err(network_error)?;
        let status = response.status();
        let body = read_text(&response).await;
        if !(200..=299).contains(&status) {
            let error_msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| body.clone());
            return Err(TransportError::Network(error_msg));
        }
        let env: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| TransportError::Framing(format!("invalid json: {e}")))?;
        if env.get("success") == Some(&serde_json::Value::Bool(false)) {
            let msg = env.get("error").and_then(|e| e.as_str()).unwrap_or("failed").to_string();
            return Err(TransportError::Network(msg));
        }
        Ok(())
    }

    pub async fn auth_login(
        base_url: &str,
        email: &str,
        password: &str,
    ) -> Result<contracts::auth::AuthPayload, TransportError> {
        let trimmed = crate::conversation_api::trim_base_url(base_url);
        let url = format!("{trimmed}/api/auth/login");
        let body = serde_json::to_vec(&contracts::auth::LoginRequest {
            email: email.to_string(),
            password: password.to_string(),
        })?;
        post_json_envelope(&url, &body).await
    }

    pub async fn auth_register(
        base_url: &str,
        email: &str,
        password: &str,
        full_name: Option<&str>,
    ) -> Result<contracts::auth::AuthPayload, TransportError> {
        let trimmed = crate::conversation_api::trim_base_url(base_url);
        let url = format!("{trimmed}/api/auth/register");
        let body = serde_json::to_vec(&contracts::auth::RegisterRequest {
            email: email.to_string(),
            password: password.to_string(),
            full_name: full_name.map(str::to_string),
        })?;
        post_json_envelope(&url, &body).await
    }

    pub async fn auth_reset_send_code(base_url: &str, email: &str) -> Result<(), TransportError> {
        let trimmed = crate::conversation_api::trim_base_url(base_url);
        let url = format!("{trimmed}/api/auth/reset/send-code");
        let body = serde_json::to_vec(&contracts::auth::SendResetCodeRequest {
            email: email.to_string(),
            lang: Some("zh-CN".to_string()),
        })?;
        post_json_void(&url, &body).await
    }

    pub async fn auth_reset_verify_code(
        base_url: &str,
        email: &str,
        code: &str,
    ) -> Result<String, TransportError> {
        let trimmed = crate::conversation_api::trim_base_url(base_url);
        let url = format!("{trimmed}/api/auth/reset/verify-code");
        let body = serde_json::to_vec(&contracts::auth::VerifyResetCodeRequest {
            email: email.to_string(),
            code: code.to_string(),
        })?;
        let env: serde_json::Value = post_json_envelope(&url, &body).await?;
        let ticket = env
            .get("reset_ticket")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                TransportError::Framing("missing reset_ticket in response".to_string())
            })?;
        Ok(ticket.to_string())
    }

    pub async fn auth_reset_confirm(
        base_url: &str,
        ticket: &str,
        new_password: &str,
    ) -> Result<(), TransportError> {
        let trimmed = crate::conversation_api::trim_base_url(base_url);
        let url = format!("{trimmed}/api/auth/reset/confirm");
        let body = serde_json::to_vec(&contracts::auth::ConfirmResetPasswordRequest {
            reset_ticket: ticket.to_string(),
            new_password: new_password.to_string(),
        })?;
        post_json_void(&url, &body).await
    }

    fn network_error(value: wasm_bindgen::JsValue) -> TransportError {
        let message = value.as_string().unwrap_or_else(|| format!("{value:?}"));
        TransportError::Network(message)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    auth_login, auth_register, auth_reset_confirm, auth_reset_send_code, auth_reset_verify_code,
    clear_browser_auth, fetch_me, read_browser_auth, write_browser_auth,
};
