use std::collections::BTreeMap;

use ingestion::parser::{LiteParseTextBlock, blocks_to_document_ir};
use ingestion::{DocumentType, ParseBackend};
use uuid::Uuid;

#[test]
fn liteparse_document_ir_contract() {
    let blocks = vec![
        LiteParseTextBlock {
            page: 1,
            text: "Title".to_string(),
            bbox: [72.0, 72.0, 200.0, 90.0],
            block_type: "heading".to_string(),
        },
        LiteParseTextBlock {
            page: 1,
            text: "Body paragraph.".to_string(),
            bbox: [72.0, 100.0, 500.0, 120.0],
            block_type: "paragraph".to_string(),
        },
    ];
    let ir = blocks_to_document_ir(
        Uuid::nil(),
        "sample.pdf",
        &blocks,
        &BTreeMap::from([(1, (612.0, 792.0))]),
    );

    assert_eq!(ir.doc_type, DocumentType::Pdf);
    assert_eq!(ir.primary_backend, ParseBackend::LiteParsePdf);
    assert_eq!(
        ir.metadata.get("ingest_route_version").map(String::as_str),
        Some("liteparse-v1")
    );
    assert_eq!(ir.blocks.len(), 2);
    assert!(ir.blocks.iter().all(|b| b.source_locator.bbox.is_some()));
}

#[test]
fn page_route_kind_labels() {
    use ingestion::parser::PageRouteKind;
    assert_eq!(PageRouteKind::Text.as_label(), "A");
    assert_eq!(PageRouteKind::Figure.as_label(), "B");
    assert_eq!(PageRouteKind::TableOcr.as_label(), "C");
    assert_eq!(PageRouteKind::ScanOcr.as_label(), "D");
}
