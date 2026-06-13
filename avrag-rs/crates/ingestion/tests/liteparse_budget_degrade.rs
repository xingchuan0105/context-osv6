use std::collections::BTreeMap;

use ingestion::parser::{
    append_liteparse_blocks_to_ir, blocks_to_document_ir, page_has_searchable_text,
    LiteParseTextBlock,
};
use ingestion::{DocumentType, ParseBackend};
use uuid::Uuid;

#[test]
fn append_liteparse_blocks_adds_budget_degraded_text() {
    let mut ir = blocks_to_document_ir(
        Uuid::nil(),
        "budget.pdf",
        &[LiteParseTextBlock {
            page: 1,
            text: "existing".to_string(),
            bbox: [0.0, 0.0, 10.0, 10.0],
            block_type: "paragraph".to_string(),
        }],
        &BTreeMap::new(),
    );

    append_liteparse_blocks_to_ir(
        &mut ir,
        &[LiteParseTextBlock {
            page: 2,
            text: "degraded scan page".to_string(),
            bbox: [0.0, 0.0, 10.0, 10.0],
            block_type: "paragraph".to_string(),
        }],
    );

    assert_eq!(ir.pages.len(), 2);
    assert!(page_has_searchable_text(&ir, 1));
    assert!(page_has_searchable_text(&ir, 2));
    assert_eq!(ir.primary_backend, ParseBackend::LiteParsePdf);
    assert_eq!(ir.doc_type, DocumentType::Pdf);
}
