use crate::platform::text_layout::{TypographyProfile, estimate_shell_height};
use crate::state::chat::{ChatMessage, ChatRole};

const CHAT_ROW_GAP_PX: f64 = 32.0;
const MIN_TEXT_WRAP_CHARS: usize = 56;

#[derive(Clone, Debug, PartialEq)]
pub struct ChatVirtualItem {
    pub id: String,
    pub text_body: String,
    pub pinned_tail: bool,
    pub profile: TypographyProfile,
}

impl ChatVirtualItem {
    pub fn predicted_height_px(&self) -> f64 {
        let line_count = predicted_line_count(&self.text_body).max(1);
        let text_height_px = line_count as f64 * self.profile.line_height_px;
        estimate_shell_height(text_height_px, &self.profile, 1) + CHAT_ROW_GAP_PX
    }
}

pub fn chat_style_profile(role: ChatRole) -> TypographyProfile {
    TypographyProfile {
        font_css: "16px Inter".to_string(),
        line_height_px: 28.0,
        horizontal_padding_px: 24.0,
        vertical_padding_px: 20.0,
        block_gap_px: 12.0,
        reserved_width_px: if role == ChatRole::Assistant {
            48.0
        } else {
            40.0
        },
    }
}

pub fn chat_message_to_virtual_item(message: &ChatMessage, pinned_tail: bool) -> ChatVirtualItem {
    ChatVirtualItem {
        id: message.id.clone(),
        text_body: message.content.clone(),
        pinned_tail,
        profile: chat_style_profile(message.role),
    }
}

fn predicted_line_count(text: &str) -> usize {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut total_lines = 0usize;

    for line in normalized.split('\n') {
        let chars = line.chars().count().max(1);
        total_lines += chars.div_ceil(MIN_TEXT_WRAP_CHARS);
    }

    total_lines.max(1)
}
