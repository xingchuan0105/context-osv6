use crate::conversation_api::{encode_path_segment, trim_base_url};
use crate::transport::TransportError;
use contracts::documents::{
    CreateDocumentRequest, CreateDocumentUploadResponse, SessionFileRow, SessionFilesResponse,
};

pub const SESSION_FILE_ACCEPT: &str =
    ".pdf,.doc,.docx,.ppt,.pptx,.xls,.xlsx,.txt,.md,.csv,.json,.toml,.yaml,.yml,.rst";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayFileStatus {
    Uploading,
    Parsing,
    Ready,
    Failed,
}

pub fn session_files_url(base_url: &str, session_id: &str) -> String {
    format!(
        "{}/api/v1/chat/sessions/{}/files",
        trim_base_url(base_url),
        encode_path_segment(session_id)
    )
}

pub fn session_file_url(base_url: &str, session_id: &str, binding_id: &str) -> String {
    format!(
        "{}/api/v1/chat/sessions/{}/files/{}",
        trim_base_url(base_url),
        encode_path_segment(session_id),
        encode_path_segment(binding_id)
    )
}

pub fn complete_upload_url(base_url: &str, document_id: &str) -> String {
    format!(
        "{}/api/v1/documents/{}/complete-upload",
        trim_base_url(base_url),
        encode_path_segment(document_id)
    )
}

pub fn reindex_document_url(base_url: &str, document_id: &str) -> String {
    format!(
        "{}/api/v1/documents/{}/reindex",
        trim_base_url(base_url),
        encode_path_segment(document_id)
    )
}

pub fn create_session_json() -> Vec<u8> {
    b"{}".to_vec()
}

pub fn create_upload_json(filename: &str, file_size: u64, mime_type: &str) -> Result<Vec<u8>, TransportError> {
    Ok(serde_json::to_vec(&CreateDocumentRequest {
        filename: filename.to_string(),
        file_size,
        mime_type: mime_type.to_string(),
    })?)
}

pub fn parse_session_files(body: &[u8]) -> Result<SessionFilesResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_upload_response(body: &[u8]) -> Result<CreateDocumentUploadResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn tray_status(status: &str) -> TrayFileStatus {
    match status {
        "completed" => TrayFileStatus::Ready,
        "failed" | "upload_invalid" => TrayFileStatus::Failed,
        "pending" | "enqueueing" => TrayFileStatus::Uploading,
        _ => TrayFileStatus::Parsing,
    }
}

pub fn files_block_send(files: &[SessionFileRow]) -> bool {
    files.iter().any(|file| {
        matches!(
            file.status.as_str(),
            "pending" | "enqueueing" | "queued" | "processing"
        )
    })
}

pub fn ready_file_count(files: &[SessionFileRow]) -> usize {
    files.iter().filter(|file| file.status == "completed").count()
}

pub fn tray_status_label(status: TrayFileStatus) -> &'static str {
    match status {
        TrayFileStatus::Uploading => "上传中",
        TrayFileStatus::Parsing => "解析中",
        TrayFileStatus::Ready => "就绪",
        TrayFileStatus::Failed => "解析失败",
    }
}

pub fn tray_status_attr(status: TrayFileStatus) -> &'static str {
    match status {
        TrayFileStatus::Uploading => "uploading",
        TrayFileStatus::Parsing => "parsing",
        TrayFileStatus::Ready => "ready",
        TrayFileStatus::Failed => "failed",
    }
}

pub fn resolve_upload_url(base_url: &str, upload_url: &str) -> String {
    let trimmed_base = trim_base_url(base_url);
    if upload_url.starts_with('/') {
        return format!("{}{}", trimmed_base, upload_url);
    }
    if trimmed_base.is_empty() {
        return upload_url.to_string();
    }
    if let (Some(base_scheme_end), Some(upload_scheme_end)) = (
        trimmed_base.find("://"),
        upload_url.find("://"),
    ) {
        let base_origin = &trimmed_base;
        let upload_after_scheme = &upload_url[upload_scheme_end + 3..];
        if let Some(path_start) = upload_after_scheme.find('/') {
            let upload_host = &upload_after_scheme[..path_start];
            let upload_path_and_query = &upload_after_scheme[path_start..];
            let base_after_scheme = &trimmed_base[base_scheme_end + 3..];
            let base_host = base_after_scheme.split('/').next().unwrap_or(base_after_scheme);

            let is_local_upload =
                upload_host.starts_with("127.0.0.1") || upload_host.starts_with("localhost");
            let is_local_base =
                base_host.starts_with("127.0.0.1") || base_host.starts_with("localhost");
            if is_local_upload && is_local_base && upload_path_and_query.starts_with("/uploads/") {
                return format!("{}{}", base_origin, upload_path_and_query);
            }
        }
    }
    upload_url.to_string()
}

