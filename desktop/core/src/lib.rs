//! Context-OS Desktop shared core library (Tauri & GPUI agnostic).
//!
//! D0.2 宿主抽库：本地数据栈（native PG+Redis）、本机产品进程生命周期、
//! 本地 B2C 会话、进程安全工具全部在此，宿主（Tauri / GPUI）只做 IPC 绑定。
//! 无 tauri / gpui 依赖；host 通过参数注入 device_id 与云中继块。

pub mod api_proxy;
pub mod chat_stream;
pub mod documents;
pub mod docker_status;
pub mod host_error;
pub mod lifecycle;
pub mod local_product;
pub mod local_session;
pub mod local_stack;
pub mod native_stack;
pub mod secret_fs;
pub mod win_cmd;

pub use api_proxy::{api_call, assert_desktop_upload_url, upload_bytes};
pub use chat_stream::{DesktopStreamError, decode_stream_chunks, stream_chat_sse};
pub use documents::reindex_local_documents;
pub use host_error::HostError;
pub use lifecycle::shutdown_all_local_runtime;
pub use local_product::{
    EnsureLocalProductResult, LocalProductStatus, ensure_local_product, product_api_base_url,
    restart_local_product, stop_local_product,
};
pub use local_session::{
    LocalAuthUser, LocalSessionStatus, ensure_local_session, get_local_session,
    local_session_token,
};
pub use local_stack::{
    ClientRuntimeConfig, EnsureLocalStackResult, LocalStackStatus, ensure_local_stack,
    get_client_runtime_config, stop_local_stack,
};
pub use native_stack::{NativeEnsureReport, ensure_native, native_tools_available, stop_native};
