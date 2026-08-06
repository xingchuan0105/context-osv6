mod commands;
mod registry;

use std::sync::Arc;

use storage_local::{LocalCache, LocalContentStore};
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

pub struct AppLocalState {
    pub content_store: Arc<LocalContentStore>,
    pub cache: Arc<LocalCache>,
}

use commands::api::api_call;
use commands::cache::{get_cache_value, set_cache_value};
use commands::chat_stream::{chat_cancel, chat_stream};
use commands::docker_status::get_docker_status;
use commands::license::{
    activate_license, get_device_id, get_license_status, heartbeat_license, open_in_browser,
    revoke_this_device, start_trial,
};
use commands::llm_config::{
    diagnose_llm, get_llm_config, list_available_models, set_llm_config, test_llm_connection,
};
use commands::local::{get_backend_status, init_local_backend, list_local_documents};
use commands::local_product::{
    ensure_local_product, get_local_product_status, stop_local_product,
};
use commands::local_session::{ensure_local_session, get_local_session};
use commands::local_stack::{
    ensure_local_stack, get_client_runtime_config, get_local_stack_status, stop_local_stack,
};
use commands::system::{get_app_data_dir, get_app_version, is_tauri_environment};
use registry::ChatStreamRegistry;

/// Prevent WebView2 `window.open` / `target=_blank` from raising NewWindowRequested.
/// Under mingw-w64 COM callbacks that path has aborted the process (0xc0000409).
const DESKTOP_EXTERNAL_LINK_GUARD_JS: &str = r#"
(function () {
  if (window.__cosExternalLinkGuard) return;
  window.__cosExternalLinkGuard = true;
  function openExternal(url) {
    if (!url) return;
    var u = String(url);
    if (!/^https?:/i.test(u) && !/^mailto:/i.test(u)) return;
    try {
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === "function") {
        window.__TAURI_INTERNALS__.invoke("open_in_browser", { url: u });
        return;
      }
    } catch (_e) {}
    try {
      if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function") {
        window.__TAURI__.core.invoke("open_in_browser", { url: u });
      }
    } catch (_e2) {}
  }
  try {
    window.open = function (url) {
      openExternal(url);
      return null;
    };
  } catch (_e3) {}
  document.addEventListener(
    "click",
    function (ev) {
      var t = ev.target;
      if (!t || !t.closest) return;
      var a = t.closest("a");
      if (!a) return;
      var href = a.href || a.getAttribute("href") || "";
      var target = (a.getAttribute("target") || "").toLowerCase();
      if (target === "_blank" || /^https?:/i.test(href)) {
        if (/^https?:/i.test(href) || /^mailto:/i.test(href)) {
          ev.preventDefault();
          ev.stopPropagation();
          openExternal(href);
        }
      }
    },
    true
  );
})();
"#;

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
            list_available_models,
            get_local_stack_status,
            get_client_runtime_config,
            ensure_local_stack,
            stop_local_stack,
            get_docker_status,
            get_local_product_status,
            ensure_local_product,
            stop_local_product,
            get_local_session,
            ensure_local_session
        ])
        .on_page_load(|webview, _payload| {
            let _ = webview.eval(DESKTOP_EXTERNAL_LINK_GUARD_JS);
        })
        .setup(|app| {
            // Protocol registration can fail without admin; never abort startup.
            #[cfg(any(windows, target_os = "linux"))]
            {
                if let Err(e) = app.deep_link().register_all() {
                    eprintln!("deep_link register_all failed (non-fatal): {e}");
                }
            }

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    commands::license::handle_deep_link_url(&handle, url.as_ref());
                }
            });

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(DESKTOP_EXTERNAL_LINK_GUARD_JS);
            }

            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
