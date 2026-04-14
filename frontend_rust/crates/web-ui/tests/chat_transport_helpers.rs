use serde_json::json;
use web_ui::routes::shared::{shared_chat_sources_from_citations, typed_citations_from_values};

#[test]
fn typed_citations_drive_shared_source_cards() {
    let citations = typed_citations_from_values(vec![json!({
        "citation_id": 41,
        "doc_id": "doc-7",
        "chunk_id": "chunk-9",
        "page": 3,
        "doc_name": "Quarterly Report",
        "preview": "Revenue grew 12% year over year.",
        "content": "Full excerpt",
        "score": 0.91,
        "layer": "rerank"
    })]);

    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].citation_id, 41);
    assert_eq!(citations[0].doc_name, "Quarterly Report");
    assert_eq!(
        citations[0].preview.as_deref(),
        Some("Revenue grew 12% year over year.")
    );

    let sources = shared_chat_sources_from_citations(&citations);
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, "chunk-9");
    assert_eq!(sources[0].title, "Quarterly Report");
    assert_eq!(
        sources[0].snippet.as_deref(),
        Some("Revenue grew 12% year over year.")
    );
    assert_eq!(sources[0].doc_id.as_deref(), Some("doc-7"));
    assert_eq!(sources[0].page, Some(3));
}
