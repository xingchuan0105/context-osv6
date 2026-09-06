//! Local document reindex (G5): re-vectorize documents ingested while RAG was off.
//! 逻辑在 `desktop_core::documents`;此处仅解析本地会话 token 并委托。

use tauri::Manager;

use super::api::IpcApiError;
use super::local_host::local_session_token;
use desktop_core::documents::ReindexDocumentsResult;

#[tauri::command]
pub async fn reindex_local_documents(
    app: tauri::AppHandle,
) -> Result<ReindexDocumentsResult, IpcApiError> {
    let token = local_session_token(&app)
        .ok_or_else(|| IpcApiError::service_unavailable("no local session token"))?;
    desktop_core::documents::reindex_local_documents(&token)
        .await
        .map_err(Into::into)
}
