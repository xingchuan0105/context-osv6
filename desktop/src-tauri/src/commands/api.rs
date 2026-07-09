//! Unified desktop IPC error shape.
//!
//! All Tauri commands should return `Result<T, IpcApiError>` (or a domain error
//! that converts into it) so the frontend can rely on `{ status, code, message }`.

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

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }

    pub fn not_implemented(method: &str, path: &str) -> Self {
        Self::new(
            501,
            "not_implemented",
            format!("API call {method} {path} is not yet implemented in desktop mode"),
        )
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

pub fn not_implemented_api_error(method: &str, path: &str) -> IpcApiError {
    IpcApiError::not_implemented(method, path)
}

#[tauri::command]
pub async fn api_call(
    method: String,
    path: String,
    _body: Option<serde_json::Value>,
    _token: Option<String>,
) -> Result<serde_json::Value, IpcApiError> {
    Err(not_implemented_api_error(&method, &path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_implemented_api_error_maps_to_frontend_contract() {
        let err = not_implemented_api_error("GET", "/api/v1/settings");

        assert_eq!(err.status, 501);
        assert_eq!(err.code, "not_implemented");
        assert!(err.message.contains("GET"));
        assert!(err.message.contains("/api/v1/settings"));
    }

    #[test]
    fn not_implemented_api_error_serializes_structured_fields() {
        let err = not_implemented_api_error("POST", "/api/v1/notebooks");
        let json = serde_json::to_value(&err).expect("serialize ipc api error");

        assert_eq!(json["status"], 501);
        assert_eq!(json["code"], "not_implemented");
        assert_eq!(
            json["message"],
            "API call POST /api/v1/notebooks is not yet implemented in desktop mode"
        );
    }

    #[test]
    fn from_string_uses_internal_code() {
        let err: IpcApiError = "boom".into();
        assert_eq!(err.status, 500);
        assert_eq!(err.code, "internal_error");
        assert_eq!(err.message, "boom");
    }
}
