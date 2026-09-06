//! Thin Tauri IPC wrappers over `desktop_core::local_*`（D0.2 宿主抽库）。
//! 业务逻辑在 `desktop/core`；此处只做 AppHandle → 参数（data_dir / device_id /
//! relay_env）的绑定与 `HostError → IpcApiError` 转换。

use tauri::Manager;

use super::api::IpcApiError;
use super::cloud_session::load_session_standalone;
use super::license::compute_device_id;

pub(crate) fn device_id() -> Option<String> {
    compute_device_id().ok()
}

/// 云会话存在时渲染官方模型中继块；无会话（或栈未初始化）返回 None = BYOK-only。
pub(crate) fn relay_env() -> Option<String> {
    load_session_standalone().map(|session| {
        desktop_core::native_stack::render_relay_env(&desktop_core::native_stack::RelayEnv {
            base_url: session.relay.base_url,
            desktop_token: session.desktop_token,
            chat_model: session.relay.chat_model,
            ingestion_model: session.relay.ingestion_model,
            embedding_model: session.relay.embedding_model,
            rerank_model: session.relay.rerank_model,
        })
    })
}

fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, IpcApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcApiError::internal(format!("app_data_dir: {e}")))?;
    Ok(dir)
}

// ---------- local_stack ----------

pub use desktop_core::local_stack::{
    ClientRuntimeConfig, EnsureLocalStackResult, LocalStackStatus,
};

#[tauri::command]
pub fn get_local_stack_status() -> LocalStackStatus {
    desktop_core::local_stack::get_local_stack_status()
}

#[tauri::command]
pub fn get_client_runtime_config() -> ClientRuntimeConfig {
    desktop_core::local_stack::get_client_runtime_config()
}

#[tauri::command]
pub async fn ensure_local_stack() -> Result<EnsureLocalStackResult, IpcApiError> {
    desktop_core::local_stack::ensure_local_stack(device_id().as_deref(), relay_env())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn stop_local_stack() -> Result<EnsureLocalStackResult, IpcApiError> {
    desktop_core::local_stack::stop_local_stack()
        .await
        .map_err(Into::into)
}

// ---------- local_product ----------

pub use desktop_core::local_product::product_api_base_url;
pub use desktop_core::local_product::log_dir_path;

#[tauri::command]
pub fn get_local_product_status() -> desktop_core::local_product::LocalProductStatus {
    desktop_core::local_product::get_local_product_status()
}

#[tauri::command]
pub async fn ensure_local_product(
) -> Result<desktop_core::local_product::EnsureLocalProductResult, IpcApiError> {
    desktop_core::local_product::ensure_local_product()
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn stop_local_product(
) -> Result<desktop_core::local_product::EnsureLocalProductResult, IpcApiError> {
    desktop_core::local_product::stop_local_product()
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn restart_local_product(
) -> Result<desktop_core::local_product::EnsureLocalProductResult, IpcApiError> {
    desktop_core::local_product::restart_local_product()
        .await
        .map_err(Into::into)
}

// ---------- docker status（settings 安装引导 UI） ----------

pub use desktop_core::docker_status::DockerStatus;

#[tauri::command]
pub fn get_docker_status() -> DockerStatus {
    desktop_core::docker_status::get_docker_status()
}

// ---------- local_session ----------

pub use desktop_core::local_session::LocalAuthUser;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LocalSessionStatus {
    pub ready: bool,
    pub email: String,
    pub token: Option<String>,
    pub user: Option<LocalAuthUser>,
    pub message: String,
    pub api_base_url: String,
}

impl From<desktop_core::local_session::LocalSessionStatus> for LocalSessionStatus {
    fn from(s: desktop_core::local_session::LocalSessionStatus) -> Self {
        Self {
            ready: s.ready,
            email: s.email,
            token: s.token,
            user: s.user,
            message: s.message,
            api_base_url: s.api_base_url,
        }
    }
}

/// Local B2C session JWT（documents / publish 等 shell 编排调用用）。
pub fn local_session_token(app: &tauri::AppHandle) -> Option<String> {
    let data_dir = app.path().app_data_dir().ok()?;
    desktop_core::local_session::local_session_token(&data_dir)
}

#[tauri::command]
pub async fn get_local_session(app: tauri::AppHandle) -> Result<LocalSessionStatus, IpcApiError> {
    let data_dir = app_data_dir(&app)?;
    desktop_core::local_session::get_local_session(&data_dir)
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn ensure_local_session(
    app: tauri::AppHandle,
) -> Result<LocalSessionStatus, IpcApiError> {
    let data_dir = app_data_dir(&app)?;
    desktop_core::local_session::ensure_local_session(
        &data_dir,
        device_id().as_deref(),
        relay_env(),
    )
    .await
    .map(Into::into)
    .map_err(Into::into)
}
