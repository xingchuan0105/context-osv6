mod commands;
mod registry;

use tauri::Manager;
use std::sync::Arc;

use storage_local::{LocalCache, LocalContentStore};
use tauri_plugin_deep_link::DeepLinkExt;

pub struct AppLocalState {
    pub content_store: Arc<LocalContentStore>,
    pub cache: Arc<LocalCache>,
}

use commands::api::api_call;
use commands::cache::{get_cache_value, set_cache_value};
use commands::chat_stream::{chat_cancel, chat_stream};
use commands::license::{
    activate_license, get_device_id, get_license_status, heartbeat_license, license_allows_chat,
    open_in_browser, revoke_this_device, start_trial,
};
use commands::llm_config::{
    diagnose_llm, get_llm_config, list_available_models, set_llm_config, test_llm_connection,
};
use commands::local::{get_backend_status, init_local_backend, list_local_documents};
use commands::system::{get_app_data_dir, get_app_version, is_tauri_environment};
use registry::ChatStreamRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init())
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
            api_call,
            get_device_id,
            start_trial,
            activate_license,
            get_license_status,
            heartbeat_license,
            revoke_this_device,
            open_in_browser,
            get_llm_config,
            set_llm_config,
            test_llm_connection,
            diagnose_llm,
            list_available_models
        ])
        .setup(|app| {
            #[cfg(any(windows, target_os = "linux"))]
            {
                app.deep_link().register_all()?;
            }

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    commands::license::handle_deep_link_url(&handle, url.as_ref());
                }
            });

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
