//! 宿主层统一错误（原 Tauri `IpcApiError` 的平台中立形态）。
//! Tauri 宿主通过 `From<HostError>` 转回 IPC 序列化错误；GPUI 宿主可直接展示。

use std::fmt;

#[derive(Debug, Clone)]
pub struct HostError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl HostError {
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
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.status, self.code, self.message)
    }
}

impl std::error::Error for HostError {}
