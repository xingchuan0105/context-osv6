//! Unified desktop IPC error shape + thin wrappers over `desktop_core::api_proxy`.
//! 代理逻辑（REST / 上传安全边界 / zstd）在 core;此处仅 IpcApiError 与命令绑定。

pub use desktop_core::HostError;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct IpcApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl IpcApiError {
    pub fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(500, "internal_error", message)
    }

    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(400, code, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(503, "service_unavailable", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }
}

impl From<HostError> for IpcApiError {
    fn from(e: HostError) -> Self {
        Self::new(e.status, e.code, e.message)
    }
}

impl From<String> for IpcApiError {
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}

impl From<&str> for IpcApiError {
    fn from(message: &str) -> Self {
        Self::internal(message)
    }
}

impl std::fmt::Display for IpcApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for IpcApiError {}

/// Proxy REST calls to the local product API (avrag-api on CLIENT_API_PORT).
#[tauri::command]
pub async fn api_call(
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    token: Option<String>,
) -> Result<serde_json::Value, IpcApiError> {
    desktop_core::api_proxy::api_call(method, path, body, token)
        .await
        .map_err(Into::into)
}

/// PUT file bytes to a signed local upload URL (WebView fetch is blocked by CORS).
#[tauri::command]
pub async fn upload_bytes(
    url: String,
    content_type: Option<String>,
    body_base64: String,
) -> Result<serde_json::Value, IpcApiError> {
    desktop_core::api_proxy::upload_bytes(url, content_type, body_base64)
        .await
        .map_err(Into::into)
}

/// 供 ipc_error 兼容测试:确认 501 契约映射保留。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_error_converts_status_and_code() {
        let host = HostError::new(413, "publish_export_too_large", "too big");
        let ipc: IpcApiError = host.into();
        assert_eq!(ipc.status, 413);
        assert_eq!(ipc.code, "publish_export_too_large");
        assert_eq!(ipc.message, "too big");
    }

    #[test]
    fn from_string_uses_internal_code() {
        let err: IpcApiError = "boom".into();
        assert_eq!(err.status, 500);
        assert_eq!(err.code, "internal_error");
        assert_eq!(err.message, "boom");
    }
}
