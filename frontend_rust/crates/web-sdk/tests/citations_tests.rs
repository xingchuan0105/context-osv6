use serde_json::json;
use web_sdk::{CitationView, render_assistant_answer, render_assistant_markdown};

fn handbook() -> CitationView {
    CitationView::from_value(&json!({
        "citation_id": 1,
        "doc_id": "doc-handbook",
        "chunk_id": "chunk-a",
        "doc_name": "手册",
        "preview": "背压与窗口",
        "score": 0.6
    }))
}

fn web_source() -> CitationView {
    CitationView::from_value(&json!({
        "citation_id": 2,
        "doc_id": "https://ok.example/doc",
        "doc_name": "网页",
        "preview": "摘要",
        "layer": "search",
        "chunk_type": "web",
        "source_locator": { "url": "https://ok.example/doc" }
    }))
}

#[test]
fn empty_answer_without_citations_is_empty() {
    let rendered = render_assistant_answer("", &[]);
    assert_eq!(rendered.html, "");
    assert!(rendered.cards.is_empty());
}

#[test]
fn numeric_and_cite_markers_become_chips() {
    let src = "结论见 [[1]] 与 [[cite:chunk-a]]。";
    let rendered = render_assistant_answer(src, &[handbook()]);
    assert!(rendered.html.contains("data-testid=\"citation-chip\""), "{}", rendered.html);
    assert!(rendered.html.contains(">1<"), "{}", rendered.html);
    assert!(!rendered.html.contains("[[1]]"), "{}", rendered.html);
    assert!(!rendered.html.contains("[[cite:chunk-a]]"), "{}", rendered.html);
    assert_eq!(rendered.cards.len(), 1);
    assert_eq!(rendered.cards[0].title, "手册");
    assert_eq!(rendered.cards[0].seq, 1);
    assert_eq!(rendered.cards[0].preview, "背压与窗口");
}

#[test]
fn same_source_reuses_sequential_number() {
    let src = "先 [[1]] 再 [[cite:chunk-a]]。";
    let rendered = render_assistant_answer(src, &[handbook()]);
    let chip_ones = rendered.html.matches(">1<").count();
    assert_eq!(chip_ones, 2, "{}", rendered.html);
    assert!(!rendered.html.contains(">2<"), "{}", rendered.html);
}

#[test]
fn web_marker_resolves_by_citation_id() {
    let src = "见 [[web:2]]。";
    let rendered = render_assistant_answer(src, &[handbook(), web_source()]);
    assert!(rendered.html.contains("来源 1 网页"), "{}", rendered.html);
    assert_eq!(rendered.cards[0].title, "网页");
    assert_eq!(
        rendered.cards[0].href.as_deref(),
        Some("https://ok.example/doc")
    );
}

#[test]
fn unresolved_marker_is_fallback_span_not_stripped() {
    let rendered = render_assistant_answer("未见 [[9]]。", &[handbook()]);
    assert!(rendered.html.contains("chat-cite-chip-fallback"), "{}", rendered.html);
    assert!(rendered.html.contains(">1<"), "{}", rendered.html);
    assert!(!rendered.html.contains("<button"), "{}", rendered.html);
}

#[test]
fn image_marker_stays_literal() {
    let rendered = render_assistant_answer("图 [[image:chunk-a]]。", &[handbook()]);
    assert!(rendered.html.contains("[[image:chunk-a]]"), "{}", rendered.html);
}

#[test]
fn malicious_marker_does_not_enter_dom() {
    let src = "x [[cite:<script>alert(1)</script>]] y";
    let rendered = render_assistant_answer(src, &[]);
    let lower = rendered.html.to_ascii_lowercase();
    assert!(!lower.contains("<script"), "{}", rendered.html);
    assert!(!lower.contains("onerror"), "{}", rendered.html);
    assert!(rendered.html.contains("chat-cite-chip-fallback"), "{}", rendered.html);
}

#[test]
fn javascript_url_on_source_locator_is_dropped() {
    let dirty = CitationView::from_value(&json!({
        "citation_id": 3,
        "doc_id": "javascript:alert(1)",
        "doc_name": "坏链",
        "source_locator": { "url": "javascript:alert(1)" }
    }));
    assert!(dirty.url.is_none());
    let rendered = render_assistant_answer("见 [[3]]。", &[dirty]);
    assert!(!rendered.html.to_ascii_lowercase().contains("javascript:"), "{}", rendered.html);
    assert!(rendered.cards[0].href.is_none());
}

#[test]
fn markdown_without_markers_matches_plain_render() {
    let src = "# 标题\n\n安全段落。";
    assert_eq!(
        render_assistant_answer(src, &[]).html,
        render_assistant_markdown(src)
    );
}
