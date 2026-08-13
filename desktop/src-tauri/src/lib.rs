mod app_nav;
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

use commands::api::{api_call, upload_bytes};
use commands::cache::{get_cache_value, set_cache_value};
use commands::chat_stream::{chat_cancel, chat_stream};
use commands::docker_status::get_docker_status;
use commands::license::{
    activate_license, get_device_id, get_license_status, heartbeat_license, open_in_browser,
    revoke_this_device, start_trial,
};
use commands::local::{get_backend_status, init_local_backend, list_local_documents};
use commands::local_product::{
    ensure_local_product, get_local_product_status, restart_local_product, stop_local_product,
};
use commands::local_session::{ensure_local_session, get_local_session};
use commands::local_stack::{
    ensure_local_stack, get_client_runtime_config, get_local_stack_status, stop_local_stack,
};
use commands::system::{get_app_data_dir, get_app_version, is_tauri_environment};
use registry::ChatStreamRegistry;

/// Prevent WebView2 `window.open` / `target=_blank` from raising NewWindowRequested.
/// Under mingw-w64 COM callbacks that path has aborted the process (0xc0000409).
///
/// Same-origin `http://tauri.localhost/...` must stay in the webview. Treating
/// every `https?:` href as external opened the OS browser (host is not DNS-public).
const DESKTOP_EXTERNAL_LINK_GUARD_JS: &str = r##"
(function () {
  if (window.__cosExternalLinkGuard) return;
  window.__cosExternalLinkGuard = true;
  function tauriHost(h) {
    return h === "tauri.localhost" || h === "ipc.localhost";
  }
  function parseUrl(raw) {
    try { return new URL(String(raw), location.href); } catch (_e) { return null; }
  }
  function shouldOpenExternal(raw) {
    if (!raw) return false;
    var s = String(raw).trim();
    if (!s || s.charAt(0) === "#" || /^javascript:/i.test(s)) return false;
    if (/^mailto:/i.test(s)) return true;
    var u = parseUrl(s);
    if (!u) return false;
    if (u.protocol === "mailto:") return true;
    if (u.protocol !== "http:" && u.protocol !== "https:") return false;
    if (tauriHost(u.hostname)) return false;
    return u.origin !== location.origin;
  }
  function stripExt(seg) {
    if (seg.slice(-5) === ".html") return { base: seg.slice(0, -5), suffix: ".html" };
    if (seg.slice(-4) === ".txt") return { base: seg.slice(0, -4), suffix: ".txt" };
    return { base: seg, suffix: "" };
  }
  function reservedDash(base) {
    return base === "analytics" || base === "_placeholder" || base.indexOf("__next") === 0;
  }
  function mapInApp(raw) {
    var u = parseUrl(raw);
    if (!u || !tauriHost(u.hostname) && u.origin !== location.origin) return null;
    var parts = u.pathname.split("/");
    if (parts[1] !== "dashboard" || !parts[2]) return null;
    var se = stripExt(parts[2]);
    if (reservedDash(se.base)) return null;
    u.searchParams.set("ws", se.base);
    parts[2] = "_placeholder" + se.suffix;
    u.pathname = parts.join("/");
    return u.pathname + u.search + u.hash;
  }
  function openExternal(url) {
    if (!url) return;
    var u = String(url);
    if (!shouldOpenExternal(u)) return;
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
  function stayInWebview(raw) {
    var mapped = mapInApp(raw);
    if (mapped) {
      location.assign(mapped);
      return;
    }
    var u = parseUrl(raw);
    if (u) location.assign(u.pathname + u.search + u.hash);
  }
  try {
    window.open = function (url) {
      if (!url) return null;
      if (shouldOpenExternal(url)) {
        openExternal(url);
        return null;
      }
      stayInWebview(url);
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
      if (shouldOpenExternal(href)) {
        ev.preventDefault();
        ev.stopPropagation();
        openExternal(href);
        return;
      }
      var mapped = mapInApp(href);
      if (mapped) {
        ev.preventDefault();
        ev.stopPropagation();
        location.assign(mapped);
        return;
      }
      if (target === "_blank") {
        ev.preventDefault();
        ev.stopPropagation();
        stayInWebview(href);
      }
    },
    true
  );
  try {
    var origFetch = window.fetch;
    if (typeof origFetch === "function") {
      window.fetch = function (input, init) {
        var raw = typeof input === "string" ? input : (input && input.url);
        var mapped = raw ? mapInApp(raw) : null;
        if (mapped) {
          if (typeof input === "string") return origFetch.call(this, mapped, init);
          return origFetch.call(this, new Request(mapped, input), init);
        }
        return origFetch.call(this, input, init);
      };
    }
  } catch (_e4) {}
})();
"##;

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
            upload_bytes,
            get_device_id,
            start_trial,
            activate_license,
            get_license_status,
            heartbeat_license,
            revoke_this_device,
            open_in_browser,
            get_local_stack_status,
            get_client_runtime_config,
            ensure_local_stack,
            stop_local_stack,
            get_docker_status,
            get_local_product_status,
            ensure_local_product,
            restart_local_product,
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // Closed product: tear down data plane + product when the shell exits.
            // Covers window close, process quit, and OS shutdown of the app.
            if let tauri::RunEvent::Exit = event {
                let log = commands::lifecycle::shutdown_all_local_runtime();
                eprintln!("{log}");
            }
        });
}
