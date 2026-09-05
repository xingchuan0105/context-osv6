use crate::conversation_api::{encode_path_segment, trim_base_url};
use crate::transport::TransportError;
use contracts::documents::DocumentsResponse;
use contracts::workspaces::{
    CreateWorkspaceNoteRequest, CreateWorkspaceRequest, WorkspaceListResponse,
    WorkspaceNoteListResponse, WorkspaceResponse,
};

pub fn workspaces_url(base_url: &str) -> String {
    format!("{}/api/v1/workspaces", trim_base_url(base_url))
}

pub fn workspace_url(base_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}",
        trim_base_url(base_url),
        encode_path_segment(workspace_id)
    )
}

pub fn workspace_documents_url(base_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/documents",
        trim_base_url(base_url),
        encode_path_segment(workspace_id)
    )
}

pub fn workspace_document_url(base_url: &str, workspace_id: &str, document_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/documents/{}",
        trim_base_url(base_url),
        encode_path_segment(workspace_id),
        encode_path_segment(document_id)
    )
}

pub fn workspace_notes_url(base_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/notes",
        trim_base_url(base_url),
        encode_path_segment(workspace_id)
    )
}

pub fn workspace_note_url(base_url: &str, workspace_id: &str, note_id: &str) -> String {
    format!(
        "{}/api/v1/workspaces/{}/notes/{}",
        trim_base_url(base_url),
        encode_path_segment(workspace_id),
        encode_path_segment(note_id)
    )
}

pub fn parse_workspace_list(body: &[u8]) -> Result<WorkspaceListResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_workspace_response(body: &[u8]) -> Result<WorkspaceResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_workspace_documents(body: &[u8]) -> Result<DocumentsResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn parse_workspace_notes(body: &[u8]) -> Result<WorkspaceNoteListResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}

pub fn create_workspace_json(name: &str, description: &str) -> Result<Vec<u8>, TransportError> {
    Ok(serde_json::to_vec(&CreateWorkspaceRequest {
        name: name.to_string(),
        description: description.to_string(),
    })?)
}

pub fn create_note_json(title: &str, content: &str) -> Result<Vec<u8>, TransportError> {
    Ok(serde_json::to_vec(&CreateWorkspaceNoteRequest {
        title: Some(title.to_string()),
        content: Some(content.to_string()),
    })?)
}
