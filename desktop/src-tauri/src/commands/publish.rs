//! Desktop workspace publish-to-cloud (ADR-0010 B3b).

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::api::{api_call, IpcApiError};
use super::cloud_session::{cloud_api_call, cloud_put_zstd, require_cloud_session};
use super::local_session::local_session_token;

const PROGRESS_EVENT: &str = "workspace-publish-progress";

#[derive(Debug, Clone, Serialize)]
struct PublishProgress {
    stage: String,
    current: u32,
    total: u32,
    message: String,
}

fn emit_progress(app: &AppHandle, stage: &str, current: u32, total: u32, message: &str) {
    let _ = app.emit(
        PROGRESS_EVENT,
        PublishProgress {
            stage: stage.to_string(),
            current,
            total,
            message: message.to_string(),
        },
    );
}

#[tauri::command]
pub async fn get_publish_status(
    app: AppHandle,
    local_workspace_id: String,
) -> Result<serde_json::Value, IpcApiError> {
    let _ = require_cloud_session(&app)?;
    cloud_api_call(
        app,
        "GET".into(),
        format!("/api/v1/workspaces/publish/status?local_workspace_id={local_workspace_id}"),
        None,
    )
    .await
}

#[tauri::command]
pub async fn publish_workspace(
    app: AppHandle,
    local_workspace_id: String,
) -> Result<serde_json::Value, IpcApiError> {
    let _ = require_cloud_session(&app)?;
    let local_token = local_session_token(&app).ok_or_else(|| {
        IpcApiError::new(
            401,
            "local_session_required",
            "本机会话未就绪，无法导出索引",
        )
    })?;

    emit_progress(&app, "pack", 0, 1, "正在列出本机已索引文档");

    let list = api_call(
        "GET".into(),
        format!("/api/v1/workspaces/{local_workspace_id}/publish/export"),
        None,
        Some(local_token.clone()),
    )
    .await?;
    let fingerprint = list.get("fingerprint").cloned().ok_or_else(|| {
        IpcApiError::internal("local export list missing fingerprint")
    })?;
    let document_ids = list
        .get("document_ids")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if document_ids.is_empty() {
        return Err(IpcApiError::bad_request(
            "publish_empty",
            "当前工作区没有已索引文档，无法发布",
        ));
    }

    let ws = api_call(
        "GET".into(),
        format!("/api/v1/workspaces/{local_workspace_id}"),
        None,
        Some(local_token.clone()),
    )
    .await?;
    let title = ws
        .get("workspace")
        .and_then(|w| w.get("name").or_else(|| w.get("title")))
        .and_then(|v| v.as_str())
        .unwrap_or("Desktop workspace")
        .to_string();

    let local_ws_uuid = serde_json::Value::String(local_workspace_id.clone());
    let session = cloud_api_call(
        app.clone(),
        "POST".into(),
        "/api/v1/workspaces/publish/sessions".into(),
        Some(serde_json::json!({
            "local_workspace_id": local_ws_uuid,
            "title": title,
            "fingerprint": fingerprint,
            "document_ids": document_ids,
        })),
    )
    .await?;
    let upload_id = session
        .get("upload_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| IpcApiError::internal("cloud session missing upload_id"))?
        .to_string();
    let total = u32::try_from(document_ids.len()).unwrap_or(u32::MAX);

    for (idx, doc_id) in document_ids.iter().enumerate() {
        let doc_id = doc_id
            .as_str()
            .ok_or_else(|| IpcApiError::internal("document id is not a string"))?;
        emit_progress(
            &app,
            "pack",
            idx as u32 + 1,
            total,
            &format!("正在打包 {doc_id}"),
        );
        let part = api_call(
            "GET".into(),
            format!("/api/v1/workspaces/{local_workspace_id}/publish/export/{doc_id}"),
            None,
            Some(local_token.clone()),
        )
        .await?;
        emit_progress(
            &app,
            "upload",
            idx as u32 + 1,
            total,
            &format!("正在上传 {doc_id}"),
        );
        let json = serde_json::to_vec(&part)
            .map_err(|err| IpcApiError::internal(format!("serialize publish part: {err}")))?;
        cloud_put_zstd(
            app.clone(),
            format!("/api/v1/workspaces/publish/sessions/{upload_id}/parts/{idx}"),
            json,
        )
        .await?;
    }

    emit_progress(&app, "commit", total, total, "正在写入云端副本");
    let committed = cloud_api_call(
        app.clone(),
        "POST".into(),
        format!("/api/v1/workspaces/publish/sessions/{upload_id}/commit"),
        None,
    )
    .await?;
    emit_progress(&app, "done", total, total, "发布完成");
    Ok(committed)
}
