mod commands;
mod registry;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use common::ContentStore;
use contracts::chat::ChatEvent;
use storage_local::{CachePort, LocalCache, LocalContentStore};
use tauri::{AppHandle, Emitter, Manager, State};

use commands::api::{not_implemented_api_error, IpcApiError};
use commands::backend::{backend_status_payload, local_document_json};
use commands::chat::{
    chat_event_channel, desktop_placeholder_events, parse_chat_request_id,
    session_id_from_request,
};
use registry::ChatStreamRegistry;

/// Managed local backend state (replaces OnceCell globals).
pub struct AppLocalState {
    pub content_store: Arc<LocalContentStore>,
    pub cache: Arc<LocalCache>,
}

fn emit_chat_event(app: &AppHandle, request_id: &str, event: &ChatEvent) -> Result<(), String> {
    app.emit(&chat_event_channel(request_id), event)
        .map_err(|e| format!("Failed to emit chat event: {e}"))
}

fn is_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::SeqCst)
}

/// 获取应用数据目录
#[tauri::command]
fn get_app_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    Ok(data_dir.to_string_lossy().to_string())
}

/// 检测是否在 Tauri 环境中运行
#[tauri::command]
fn is_tauri_environment() -> bool {
    true
}

/// 获取应用版本
#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 初始化本地后端
#[tauri::command]
async fn init_local_backend(app: tauri::AppHandle) -> Result<String, String> {
    if app.try_state::<AppLocalState>().is_some() {
        return Err("Local backend already initialized".to_string());
    }

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?
        .to_string_lossy()
        .to_string();

    tokio::fs::create_dir_all(&data_dir)
        .await
        .map_err(|e| format!("Failed to create data dir: {e}"))?;

    let content_store = Arc::new(LocalContentStore::new(format!("{data_dir}/content")));
    let cache = Arc::new(LocalCache::new());

    app.manage(AppLocalState {
        content_store,
        cache,
    });

    Ok(format!("Local backend initialized at {data_dir}"))
}

/// 获取本地后端状态
#[tauri::command]
async fn get_backend_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let initialized = app.try_state::<AppLocalState>().is_some();
    Ok(backend_status_payload(initialized))
}

/// 列出本地文档
#[tauri::command]
async fn list_local_documents(state: State<'_, AppLocalState>) -> Result<Vec<serde_json::Value>, String> {
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::from(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::System,
    );

    let documents = state
        .content_store
        .list_documents(&auth, None, None)
        .await
        .map_err(|e| format!("Failed to list documents: {e}"))?;

    Ok(documents
        .iter()
        .map(local_document_json)
        .collect())
}

/// Chat 流式接口：通过事件推送 contracts ChatEvent 给前端 (C1)
#[tauri::command]
async fn chat_stream(
    _token: String,
    request: serde_json::Value,
    app: tauri::AppHandle,
    registry: State<'_, ChatStreamRegistry>,
) -> Result<(), String> {
    let request_id = parse_chat_request_id(&request)?;
    let session_id = session_id_from_request(&request);
    let cancel = registry.register(&request_id);

    let emit_or_stop = |app: &AppHandle, event: &ChatEvent| -> Result<bool, String> {
        if is_cancelled(&cancel) {
            return Ok(false);
        }
        emit_chat_event(app, &request_id, event)?;
        Ok(true)
    };

    let result = (|| {
        for event in desktop_placeholder_events(&request_id, &session_id) {
            if !emit_or_stop(&app, &event)? {
                return Ok(());
            }
        }
        Ok(())
    })();

    registry.remove(&request_id);
    result
}

/// 取消进行中的 chat 流 (C2, idempotent)
#[tauri::command]
fn chat_cancel(request_id: String, registry: State<'_, ChatStreamRegistry>) -> Result<(), String> {
    registry.cancel(&request_id);
    Ok(())
}

/// 通用 API 调用代理：桌面端本地模式的 REST 代理骨架 (C3)
#[tauri::command]
async fn api_call(
    method: String,
    path: String,
    _body: Option<serde_json::Value>,
    _token: Option<String>,
) -> Result<serde_json::Value, IpcApiError> {
    Err(not_implemented_api_error(&method, &path))
}

/// 获取本地缓存值
#[tauri::command]
async fn get_cache_value(
    state: State<'_, AppLocalState>,
    key: String,
) -> Result<Option<String>, String> {
    Ok(state.cache.get(&key).await)
}

/// 设置本地缓存值
#[tauri::command]
async fn set_cache_value(
    state: State<'_, AppLocalState>,
    key: String,
    value: String,
    ttl_secs: u64,
) -> Result<(), String> {
    state.cache.set(&key, &value, ttl_secs).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(ChatStreamRegistry::default())
        .invoke_handler(tauri::generate_handler![
            get_app_data_dir,
            is_tauri_environment,
            get_app_version,
            init_local_backend,
            get_backend_status,
            list_local_documents,
            get_cache_value,
            set_cache_value,
            chat_stream,
            chat_cancel,
            api_call
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
