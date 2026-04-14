use std::sync::atomic::{AtomicU64, Ordering};

pub mod capabilities;
pub mod text_layout;

pub use capabilities::{UI_CAPABILITIES, UiCapabilities, ui_capabilities};

static NEXT_CLIENT_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_client_id(prefix: &str) -> String {
    let sequence = NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{sequence}", now_millis())
}

#[cfg(target_arch = "wasm32")]
fn now_millis() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
