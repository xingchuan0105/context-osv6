#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct IpcApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

pub fn not_implemented_api_error(method: &str, path: &str) -> IpcApiError {
    IpcApiError {
        status: 501,
        code: "not_implemented".to_string(),
        message: format!("API call {method} {path} is not yet implemented in desktop mode"),
    }
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
}
