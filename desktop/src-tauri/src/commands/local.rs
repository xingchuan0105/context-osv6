use common::ContentStore;
use tauri::Manager;
use tauri::State;

use crate::commands::api::IpcApiError;
use crate::AppLocalState;

#[tauri::command]
pub async fn init_local_backend(app: tauri::AppHandle) -> Result<String, IpcApiError> {
    if app.try_state::<AppLocalState>().is_some() {
        return Err(IpcApiError::bad_request(
            "already_initialized",
            "Local backend already initialized",
        ));
    }

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("Failed to get app data dir: {e}")))?
        .to_string_lossy()
        .to_string();

    tokio::fs::create_dir_all(&data_dir)
        .await
        .map_err(|e| IpcApiError::internal(format!("Failed to create data dir: {e}")))?;

    let content_store = std::sync::Arc::new(storage_local::LocalContentStore::new(format!(
        "{data_dir}/content"
    )));
    let cache = std::sync::Arc::new(storage_local::LocalCache::new());

    app.manage(AppLocalState {
        content_store,
        cache,
    });

    Ok(format!("Local backend initialized at {data_dir}"))
}

#[tauri::command]
pub async fn get_backend_status(app: tauri::AppHandle) -> Result<serde_json::Value, IpcApiError> {
    let initialized = app.try_state::<AppLocalState>().is_some();
    Ok(super::backend::backend_status_payload(initialized))
}

#[tauri::command]
pub async fn list_local_documents(
    state: State<'_, AppLocalState>,
) -> Result<Vec<serde_json::Value>, IpcApiError> {
    let auth = contracts::auth_runtime::AuthContext::new(
        contracts::auth_runtime::OrgId::from(uuid::Uuid::nil()),
        contracts::auth_runtime::SubjectKind::System,
    );

    let documents = state
        .content_store
        .list_documents(&auth, None, None)
        .await
        .map_err(|e| IpcApiError::internal(format!("Failed to list documents: {e}")))?;

    Ok(documents
        .iter()
        .map(super::backend::local_document_json)
        .collect())
}
