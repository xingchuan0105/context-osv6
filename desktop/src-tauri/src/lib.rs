use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use contracts::chat::ChatEvent;
use common::ContentStore;
use storage_local::{CachePort, LocalCache, LocalContentStore};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

/// Managed local backend state (replaces OnceCell globals).
pub struct AppLocalState {
    pub content_store: Arc<LocalContentStore>,
    pub cache: Arc<LocalCache>,
}

/// Tracks in-flight chat streams for cancellation (C2).
#[derive(Default)]
pub struct ChatStreamRegistry {
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ChatStreamRegistry {
    pub fn register(&self, request_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .expect("chat registry lock")
            .insert(request_id.to_string(), Arc::clone(&flag));
        flag
    }

    pub fn cancel(&self, request_id: &str) -> bool {
        let mut guard = self.cancellations.lock().expect("chat registry lock");
        if let Some(flag) = guard.get(request_id) {
            flag.store(true, Ordering::SeqCst);
            guard.remove(request_id);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, request_id: &str) {
        self.cancellations
            .lock()
            .expect("chat registry lock")
            .remove(request_id);
    }
}

#[derive(Debug, serde::Serialize)]
struct IpcApiError {
    status: u16,
    code: String,
    message: String,
}

fn emit_chat_event(app: &AppHandle, request_id: &str, event: &ChatEvent) -> Result<(), String> {
    let channel = format!("chat://{request_id}");
    app.emit(&channel, event)
        .map_err(|e| format!("Failed to emit chat event: {e}"))
}

fn session_id_from_request(request: &serde_json::Value) -> String {
    request
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
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

    Ok(serde_json::json!({
        "initialized": initialized,
        "type": "local",
        "storage": {
            "type": "filesystem",
            "initialized": initialized
        },
        "cache": {
            "type": "memory",
            "initialized": initialized
        }
    }))
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

    let result: Vec<serde_json::Value> = documents
        .iter()
        .map(|doc| {
            serde_json::json!({
                "id": doc.id,
                "name": doc.file_name,
                "status": doc.status,
                "created_at": doc.created_at,
            })
        })
        .collect();

    Ok(result)
}

/// Chat 流式接口：通过事件推送 contracts ChatEvent 给前端 (C1)
#[tauri::command]
async fn chat_stream(
    _token: String,
    request: serde_json::Value,
    app: tauri::AppHandle,
    registry: State<'_, ChatStreamRegistry>,
) -> Result<(), String> {
    let request_id = request
        .get("request_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "request_id is required".to_string())?;

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
        if !emit_or_stop(
            &app,
            &ChatEvent::Start {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
            },
        )? {
            return Ok(());
        }

        let message_id: i64 = 1;
        if !emit_or_stop(
            &app,
            &ChatEvent::AnswerStart {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                message_id,
                agent_type: "chat".to_string(),
            },
        )? {
            return Ok(());
        }

        let placeholder = "[Desktop mode] Chat is not yet connected to LLM backend. This is a placeholder response.";
        if !emit_or_stop(
            &app,
            &ChatEvent::Token {
                request_id: request_id.clone(),
                message_id,
                content: placeholder.to_string(),
            },
        )? {
            return Ok(());
        }

        if is_cancelled(&cancel) {
            return Ok(());
        }

        emit_chat_event(
            &app,
            &request_id,
            &ChatEvent::Done {
                request_id: request_id.clone(),
                session_id,
                message_id,
                payload: serde_json::json!({
                    "answer": placeholder,
                    "status": "done",
                }),
            },
        )?;

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
    Err(IpcApiError {
        status: 501,
        code: "not_implemented".to_string(),
        message: format!("API call {method} {path} is not yet implemented in desktop mode"),
    })
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
