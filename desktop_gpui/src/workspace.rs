//! Native knowledge operations reuse the existing workspace contract and desktop transport.
use contracts::{
    documents::{Document, DocumentContentResponse},
    workspaces::{Workspace, WorkspaceNote, WorkspaceNoteResponse},
};
use serde_json::Value;
use std::path::PathBuf;
use web_sdk::{conversation_api::encode_path_segment, workspace_api as api};

#[derive(Clone, Debug)]
pub enum Action {
    List,
    Create(String),
    Load,
    SaveNote {
        id: Option<String>,
        title: String,
        content: String,
    },
    DeleteNote(String),
    DeleteDocument(String),
    Reindex(String),
    Preview(String),
    Upload(PathBuf),
}

pub enum ResultData {
    List(Vec<Workspace>),
    Created(Workspace),
    Loaded {
        documents: Vec<Document>,
        notes: Vec<WorkspaceNote>,
    },
    NoteSaved(WorkspaceNote),
    Changed,
    Preview {
        document_id: String,
        content: String,
    },
}

pub struct Reply {
    pub request: u64,
    pub workspace: Option<String>,
    pub action: Action,
    pub result: Result<ResultData, String>,
}

pub async fn call(
    token: &str,
    method: &str,
    path: String,
    body: Option<Value>,
) -> Result<Value, String> {
    desktop_core::api_call(method.into(), path, body, Some(token.into()))
        .await
        .map_err(|e| e.to_string())
}

fn body(bytes: Result<Vec<u8>, web_sdk::TransportError>) -> Result<Option<Value>, String> {
    serde_json::from_slice(&bytes.map_err(|e| e.to_string())?)
        .map(Some)
        .map_err(|e| e.to_string())
}

pub async fn execute(
    token: &str,
    workspace: Option<&str>,
    action: &Action,
) -> Result<ResultData, String> {
    if matches!(action, Action::List) {
        let value = call(token, "GET", api::workspaces_url(""), None).await?;
        return api::parse_workspace_list(&serde_json::to_vec(&value).unwrap())
            .map(|v| ResultData::List(v.workspaces))
            .map_err(|e| e.to_string());
    }
    if let Action::Create(name) = action {
        if name.trim().is_empty() {
            return Err("工作区名称不能为空".into());
        }
        let value = call(
            token,
            "POST",
            api::workspaces_url(""),
            body(api::create_workspace_json(name.trim(), ""))?,
        )
        .await?;
        return api::parse_workspace_response(&serde_json::to_vec(&value).unwrap())
            .map(|v| ResultData::Created(v.workspace))
            .map_err(|e| e.to_string());
    }
    let wid = workspace
        .filter(|v| !v.is_empty())
        .ok_or("尚未选择工作区")?;
    match action {
        Action::Load => {
            let (documents, notes) = tokio::try_join!(
                call(token, "GET", api::workspace_documents_url("", wid), None),
                call(token, "GET", api::workspace_notes_url("", wid), None)
            )?;
            let documents =
                api::parse_workspace_documents(&serde_json::to_vec(&documents).unwrap())
                    .map_err(|e| e.to_string())?
                    .documents;
            let notes = api::parse_workspace_notes(&serde_json::to_vec(&notes).unwrap())
                .map_err(|e| e.to_string())?
                .notes;
            if documents
                .iter()
                .any(|d| d.workspace_id.as_deref() != Some(wid))
                || notes.iter().any(|n| n.workspace_id != wid)
            {
                return Err("资料返回的工作区归属不一致，请重试".into());
            }
            Ok(ResultData::Loaded { documents, notes })
        }
        Action::SaveNote { id, title, content } => {
            if title.trim().is_empty() && content.trim().is_empty() {
                return Err("笔记标题或正文至少填写一项".into());
            }
            let (method, url, payload) = if let Some(id) = id {
                (
                    "PATCH",
                    api::workspace_note_url("", wid, id),
                    api::update_note_json(title, content),
                )
            } else {
                (
                    "POST",
                    api::workspace_notes_url("", wid),
                    api::create_note_json(title, content),
                )
            };
            let value = call(token, method, url, body(payload)?).await?;
            let note: WorkspaceNoteResponse =
                serde_json::from_value(value).map_err(|e| e.to_string())?;
            if note.note.workspace_id != wid {
                return Err("笔记归属不一致".into());
            }
            Ok(ResultData::NoteSaved(note.note))
        }
        Action::DeleteNote(id) => {
            call(token, "DELETE", api::workspace_note_url("", wid, id), None).await?;
            Ok(ResultData::Changed)
        }
        Action::DeleteDocument(id) | Action::Reindex(id) | Action::Preview(id) => {
            // The UI only offers documents from the active workspace; the API authorizes access.
            let url = api::document_url("", id);
            match action {
                Action::Preview(_) => {
                    let value = call(token, "GET", format!("{url}/content"), None).await?;
                    let content: DocumentContentResponse =
                        serde_json::from_value(value).map_err(|e| e.to_string())?;
                    Ok(ResultData::Preview {
                        document_id: id.clone(),
                        content: content.content,
                    })
                }
                Action::Reindex(_) => {
                    call(token, "POST", web_sdk::reindex_document_url("", id), None).await?;
                    Ok(ResultData::Changed)
                }
                _ => {
                    call(token, "DELETE", url, None).await?;
                    Ok(ResultData::Changed)
                }
            }
        }
        Action::Upload(path) => upload(token, wid, path.clone()).await,
        _ => unreachable!(),
    }
}

async fn upload(token: &str, wid: &str, path: PathBuf) -> Result<ResultData, String> {
    let selected = path.clone();
    let size = tokio::task::spawn_blocking(move || std::fs::metadata(selected))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("读取文件失败：{e}"))?;
    if !size.is_file() || size.len() == 0 {
        return Err("请选择非空文件".into());
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("文件名无效")?;
    let mime = mime_type(&path);
    let value = call(
        token,
        "POST",
        api::workspace_document_upload_url("", wid),
        body(web_sdk::create_upload_json(name, size.len(), mime))?,
    )
    .await?;
    let upload = web_sdk::parse_upload_response(&serde_json::to_vec(&value).unwrap())
        .map_err(|e| e.to_string())?;
    let url =
        web_sdk::resolve_upload_url(&desktop_core::product_api_base_url(), &upload.upload_url);
    let sent = desktop_core::api_proxy::upload_local_file(url, mime.into(), path, size.len()).await;
    if let Err(error) = sent {
        let cleanup = call(
            token,
            "DELETE",
            api::document_url("", &upload.document_id),
            None,
        )
        .await;
        return Err(format!(
            "上传失败：{error}{}",
            if cleanup.is_err() {
                "；未完成的资料记录仍保留，可在资料列表删除"
            } else {
                ""
            }
        ));
    }
    call(
        token,
        "POST",
        web_sdk::complete_upload_url("", &upload.document_id),
        None,
    )
    .await?;
    Ok(ResultData::Changed)
}

fn mime_type(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" | "md" | "rst" => "text/plain",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",
        _ => "application/octet-stream",
    }
}

pub fn session_list_path(workspace: Option<&str>) -> String {
    let base = web_sdk::conversation_api::sessions_url("");
    workspace
        .map(|id| format!("{base}?workspace_id={}", encode_path_segment(id)))
        .unwrap_or(base)
}

pub fn status_label(status: &str) -> &'static str {
    match status {
        "completed" => "可检索",
        "pending" => "等待上传",
        "enqueueing" | "queued" => "排队中",
        "processing" => "处理中",
        "failed" | "upload_invalid" => "处理失败",
        "deleting" | "deleted" => "删除中",
        _ => "状态未知",
    }
}
