// release 构建下未擦除组件的 view! 嵌套类型深度会超出默认查询深度上限
#![recursion_limit = "512"]

pub mod app;
pub mod components;
pub mod reducer;
pub mod routes;
pub mod session;

pub use app::{App, shell};
pub use components::chat::{ChatCanvasModel, PreparedUserTurn, StreamScope};
pub use reducer::{ActivityEntry, ChatTurnState, TurnStatus, reduce_chat_event};
pub use routes::AppRoute;
pub use session::{ActiveConversation, ConversationManager, ConversationMessage, MessageRole};

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
