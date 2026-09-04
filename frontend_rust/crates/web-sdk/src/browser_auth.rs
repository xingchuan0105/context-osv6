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

        let controller = AbortController::new().map_err(network_error)?;
        let signal = controller.signal();
        let timeout = Closure::once(move || controller.abort());
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout.as_ref().unchecked_ref(),
                AUTH_BOOTSTRAP_TIMEOUT_MS,
            );
        }

        let init = RequestInit::new();
        init.set_method("GET");
        init.set_mode(RequestMode::Cors);
        init.set_headers_headers(&headers);
        init.set_signal(Some(&signal));

        let window = web_sys::window()
            .ok_or_else(|| TransportError::Unavailable("no window object".to_string()))?;
        let request = Request::new_with_str_and_init(&url, &init).map_err(network_error)?;
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(network_error)?;
        drop(timeout);
        let response: web_sys::Response = response.dyn_into().map_err(network_error)?;
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

    fn network_error(value: wasm_bindgen::JsValue) -> TransportError {
        let message = value.as_string().unwrap_or_else(|| format!("{value:?}"));
        TransportError::Network(message)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{clear_browser_auth, fetch_me, read_browser_auth, write_browser_auth};
