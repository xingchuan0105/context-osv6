use web_ui::routes::search::{search_answer_item_text, search_source_preview_text};
use web_ui::routes::shared::{shared_answer_item_text, shared_source_preview_text};

#[test]
fn search_answer_uses_full_answer_text() {
    assert_eq!(search_answer_item_text("hello"), "hello");
}

#[test]
fn shared_answer_uses_streaming_text_when_present() {
    assert_eq!(
        shared_answer_item_text("stream chunk", "final answer"),
        "stream chunk"
    );
    assert_eq!(shared_answer_item_text("", "final answer"), "final answer");
}

#[test]
fn source_preview_helpers_prefer_preview_text() {
    assert_eq!(shared_source_preview_text(Some("preview"), None), "preview");
    assert_eq!(search_source_preview_text(Some("snippet"), None), "snippet");
}
