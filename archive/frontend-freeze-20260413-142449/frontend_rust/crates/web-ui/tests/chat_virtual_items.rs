use web_ui::components::chat::virtual_items::{chat_message_to_virtual_item, chat_style_profile};
use web_ui::state::chat::{ChatMessage, ChatRole};

fn message(id: &str, role: ChatRole, content: &str) -> ChatMessage {
    ChatMessage {
        id: id.to_string(),
        role,
        content: content.to_string(),
        answer_blocks: Vec::new(),
        citations: Vec::new(),
        session_id: None,
        server_message_id: None,
    }
}

#[test]
fn assistant_message_becomes_predictable_virtual_item() {
    let item =
        chat_message_to_virtual_item(&message("m1", ChatRole::Assistant, "long answer"), false);
    assert_eq!(item.id, "m1");
    assert_eq!(item.text_body, "long answer");
    assert!(!item.pinned_tail);
}

#[test]
fn streaming_tail_is_pinned() {
    let item = chat_message_to_virtual_item(&message("m2", ChatRole::Assistant, "stream"), true);
    assert!(item.pinned_tail);
}

#[test]
fn chat_style_profile_reserves_avatar_width() {
    let profile = chat_style_profile(ChatRole::Assistant);
    assert!(profile.reserved_width_px >= 32.0);
}
