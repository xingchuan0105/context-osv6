use ingestion::parser::{
    PageRouteKind, PdfPageBackend, PdfPagePlan, PdfParsePlan, RouteReason,
};
use ingestion::{
    BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, LiteParseService, PageIr,
    ParseBackend,
};
use uuid::Uuid;

use super::merge::merge_pdf_ir;
use super::paddle::group_contiguous_pages;
use super::parse::{collect_page_routes, probe_pdf_content_from_snapshot, PdfPageRoutes};

#[test]
fn test_group_contiguous_pages() {
    assert_eq!(group_contiguous_pages(&[]), Vec::<(u32, u32)>::new());
    assert_eq!(group_contiguous_pages(&[1]), vec![(1, 1)]);
    assert_eq!(group_contiguous_pages(&[1, 2, 3, 4, 5]), vec![(1, 5)]);
    assert_eq!(
        group_contiguous_pages(&[1, 2, 3, 10, 11, 20]),
        vec![(1, 3), (10, 11), (20, 20)]
    );
}

#[test]
fn merge_pdf_ir_paddle_fallback_to_visual() {
    let doc_id = Uuid::new_v4();
    let plan = PdfParsePlan {
        pages: vec![
            PdfPagePlan {
                page_number: 1,
                backend: PdfPageBackend::PaddleOcr,
                reason: RouteReason::ScannedPdf,
                route_kinds: vec![],
            },
            PdfPagePlan {
                page_number: 2,
                backend: PdfPageBackend::PaddleOcr,
                reason: RouteReason::ScannedPdf,
                route_kinds: vec![],
            },
        ],
    };

    let mut visual_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::VisualRasterPdf,
    );
    visual_ir.pages.push(PageIr {
        page_number: 1,
        backend: ParseBackend::VisualRasterPdf,
        text_char_count: 100,
        ..Default::default()
    });
    visual_ir.pages.push(PageIr {
        page_number: 2,
        backend: ParseBackend::VisualRasterPdf,
        text_char_count: 200,
        ..Default::default()
    });
    visual_ir.blocks.push(BlockIr {
        block_id: "v1".to_string(),
        page: Some(1),
        block_type: BlockType::PageRaster,
        modality: BlockModality::ImageWithContext,
        text: "page 1 raster".to_string(),
        parser_backend: ParseBackend::VisualRasterPdf,
        ..Default::default()
    });

    let merged = merge_pdf_ir(
        doc_id,
        "test.pdf",
        &plan,
        None,
        None,
        Some(visual_ir),
        &std::collections::HashSet::new(),
    )
    .unwrap();

    assert_eq!(merged.pages.len(), 2, "should have 2 pages from visual fallback");
    assert_eq!(merged.blocks.len(), 1, "should have 1 block from visual fallback");
    assert_eq!(merged.pages[0].backend, ParseBackend::VisualRasterPdf);
}

#[test]
fn merge_pdf_ir_paddle_success_overrides() {
    let doc_id = Uuid::new_v4();
    let plan = PdfParsePlan {
        pages: vec![PdfPagePlan {
            page_number: 1,
            backend: PdfPageBackend::PaddleOcr,
            reason: RouteReason::ScannedPdf,
            route_kinds: vec![],
        }],
    };

    let mut paddle_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    paddle_ir.pages.push(PageIr {
        page_number: 1,
        backend: ParseBackend::PaddleOcrPdf,
        text_char_count: 500,
        ..Default::default()
    });
    paddle_ir.blocks.push(BlockIr {
        block_id: "p1".to_string(),
        page: Some(1),
        block_type: BlockType::Paragraph,
        modality: BlockModality::TextOnly,
        text: "OCR text".to_string(),
        parser_backend: ParseBackend::PaddleOcrPdf,
        ..Default::default()
    });

    let merged = merge_pdf_ir(
        doc_id,
        "test.pdf",
        &plan,
        None,
        Some(paddle_ir),
        None,
        &std::collections::HashSet::from([1]),
    )
    .unwrap();

    assert_eq!(merged.pages.len(), 1);
    assert_eq!(merged.pages[0].backend, ParseBackend::PaddleOcrPdf);
    assert_eq!(merged.blocks[0].text, "OCR text");
}

#[test]
fn merge_pdf_ir_hybrid_v2_metadata() {
    let doc_id = Uuid::new_v4();
    let plan = PdfParsePlan {
        pages: vec![
            PdfPagePlan {
                page_number: 1,
                backend: PdfPageBackend::EdgeParse,
                reason: RouteReason::FastText,
                route_kinds: vec![],
            },
            PdfPagePlan {
                page_number: 2,
                backend: PdfPageBackend::PaddleOcr,
                reason: RouteReason::ScannedPdf,
                route_kinds: vec![],
            },
        ],
    };

    let mut digital_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::EdgeParsePdf,
    );
    digital_ir.pages.push(PageIr {
        page_number: 1,
        backend: ParseBackend::EdgeParsePdf,
        ..Default::default()
    });

    let mut paddle_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    paddle_ir.pages.push(PageIr {
        page_number: 2,
        backend: ParseBackend::PaddleOcrPdf,
        ..Default::default()
    });

    let merged = merge_pdf_ir(
        doc_id,
        "test.pdf",
        &plan,
        Some(digital_ir),
        Some(paddle_ir),
        None,
        &std::collections::HashSet::from([2]),
    )
    .unwrap();

    assert_eq!(
        merged.metadata.get("pdf_route_mode").map(|s| s.as_str()),
        Some("hybrid_v2")
    );
}

#[test]
fn merge_pdf_ir_partial_paddle_success() {
    let doc_id = Uuid::new_v4();
    let plan = PdfParsePlan {
        pages: vec![
            PdfPagePlan {
                page_number: 1,
                backend: PdfPageBackend::PaddleOcr,
                reason: RouteReason::ScannedPdf,
                route_kinds: vec![],
            },
            PdfPagePlan {
                page_number: 2,
                backend: PdfPageBackend::PaddleOcr,
                reason: RouteReason::ScannedPdf,
                route_kinds: vec![],
            },
        ],
    };

    let mut paddle_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    paddle_ir.pages.push(PageIr {
        page_number: 1,
        backend: ParseBackend::PaddleOcrPdf,
        text_char_count: 500,
        ..Default::default()
    });
    paddle_ir.blocks.push(BlockIr {
        block_id: "p1".to_string(),
        page: Some(1),
        block_type: BlockType::Paragraph,
        modality: BlockModality::TextOnly,
        text: "OCR text page 1".to_string(),
        parser_backend: ParseBackend::PaddleOcrPdf,
        ..Default::default()
    });

    let mut visual_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::VisualRasterPdf,
    );
    visual_ir.pages.push(PageIr {
        page_number: 2,
        backend: ParseBackend::VisualRasterPdf,
        text_char_count: 200,
        ..Default::default()
    });
    visual_ir.blocks.push(BlockIr {
        block_id: "v2".to_string(),
        page: Some(2),
        block_type: BlockType::PageRaster,
        modality: BlockModality::ImageWithContext,
        text: "visual page 2".to_string(),
        parser_backend: ParseBackend::VisualRasterPdf,
        ..Default::default()
    });

    let paddle_successful = std::collections::HashSet::from([1]);
    let merged = merge_pdf_ir(
        doc_id,
        "test.pdf",
        &plan,
        None,
        Some(paddle_ir),
        Some(visual_ir),
        &paddle_successful,
    )
    .unwrap();

    assert_eq!(merged.pages.len(), 2, "should have 2 pages");
    assert_eq!(
        merged.pages[0].backend,
        ParseBackend::PaddleOcrPdf,
        "page 1 should be paddle"
    );
    assert_eq!(
        merged.pages[1].backend,
        ParseBackend::VisualRasterPdf,
        "page 2 should fall back to visual"
    );
    assert_eq!(merged.blocks[0].text, "OCR text page 1");
    assert_eq!(merged.blocks[1].text, "visual page 2");
}

#[test]
fn merge_pdf_ir_combined_text_and_table_ocr() {
    let doc_id = Uuid::new_v4();
    let plan = PdfParsePlan {
        pages: vec![PdfPagePlan {
            page_number: 1,
            backend: PdfPageBackend::PaddleOcr,
            reason: RouteReason::SlowOcr,
            route_kinds: vec![PageRouteKind::Text, PageRouteKind::TableOcr],
        }],
    };

    let mut digital_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::LiteParsePdf,
    );
    digital_ir.pages.push(PageIr {
        page_number: 1,
        backend: ParseBackend::LiteParsePdf,
        text_char_count: 120,
        ..Default::default()
    });
    digital_ir.blocks.push(BlockIr {
        block_id: "lp1".to_string(),
        page: Some(1),
        block_type: BlockType::Paragraph,
        modality: BlockModality::TextOnly,
        text: "Digital paragraph".to_string(),
        parser_backend: ParseBackend::LiteParsePdf,
        ..Default::default()
    });

    let mut paddle_ir = DocumentIr::new(
        doc_id.to_string(),
        "test.pdf".to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    paddle_ir.pages.push(PageIr {
        page_number: 1,
        backend: ParseBackend::PaddleOcrPdf,
        text_char_count: 80,
        ..Default::default()
    });
    paddle_ir.blocks.push(BlockIr {
        block_id: "paddle-table".to_string(),
        page: Some(1),
        block_type: BlockType::Table,
        modality: BlockModality::TextOnly,
        text: "| col1 | col2 |\n| a | b |".to_string(),
        parser_backend: ParseBackend::PaddleOcrPdf,
        ..Default::default()
    });

    let merged = merge_pdf_ir(
        doc_id,
        "test.pdf",
        &plan,
        Some(digital_ir),
        Some(paddle_ir),
        None,
        &std::collections::HashSet::from([1]),
    )
    .unwrap();

    assert_eq!(merged.pages.len(), 1, "combined page should merge to one page row");
    assert_eq!(merged.pages[0].backend, ParseBackend::PaddleOcrPdf);
    assert_eq!(merged.blocks.len(), 2, "A+C page should keep digital text + paddle table");
    assert!(
        merged
            .blocks
            .iter()
            .any(|b| b.block_type == BlockType::Paragraph && b.text.contains("Digital")),
        "digital text block should remain when paddle only supplies table"
    );
    assert!(
        merged
            .blocks
            .iter()
            .any(|b| b.block_type == BlockType::Table),
        "paddle table block should be present"
    );
    assert_eq!(
        merged.metadata.get("pdf_route_mode").map(String::as_str),
        Some("liteparse_hybrid")
    );
}

#[test]
fn collect_page_routes_splits_text_and_ocr_pages() {
    let plan = PdfParsePlan {
        pages: vec![
            PdfPagePlan {
                page_number: 1,
                backend: PdfPageBackend::EdgeParse,
                reason: RouteReason::SimplePdf,
                route_kinds: vec![PageRouteKind::Text],
            },
            PdfPagePlan {
                page_number: 2,
                backend: PdfPageBackend::PaddleOcr,
                reason: RouteReason::ScannedPdf,
                route_kinds: vec![PageRouteKind::ScanOcr],
            },
            PdfPagePlan {
                page_number: 3,
                backend: PdfPageBackend::PaddleOcr,
                reason: RouteReason::SlowOcr,
                route_kinds: vec![PageRouteKind::TableOcr],
            },
        ],
    };

    let routes = collect_page_routes(&plan);
    assert_eq!(routes.text_pages, vec![1]);
    assert_eq!(routes.ocr_pages, vec![2, 3]);
    assert_eq!(routes.table_ocr_pages, std::collections::HashSet::from([3]));
}

#[tokio::test]
async fn probe_pdf_content_from_snapshot_builds_digital_ir_without_reparse() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/spike/fixtures/phase0-mini.pdf");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(path).expect("read fixture");
    let snapshot = LiteParseService::from_env()
        .parse_pdf_document(&bytes)
        .await
        .expect("parse snapshot once");

    let routes = PdfPageRoutes {
        text_pages: snapshot
            .probes()
            .iter()
            .map(|probe| probe.page_number)
            .collect(),
        ocr_pages: Vec::new(),
        table_ocr_pages: std::collections::HashSet::new(),
    };
    let document_id = Uuid::new_v4();
    let outcome = probe_pdf_content_from_snapshot(
        &snapshot,
        "phase0-mini.pdf",
        document_id,
        &routes,
    );

    assert_eq!(
        outcome.page_dimensions.len(),
        snapshot.page_dimensions().len(),
        "dimensions should come from cached snapshot"
    );
    let digital_ir = outcome
        .digital_ir
        .expect("digital IR from snapshot blocks");
    assert!(
        !digital_ir.blocks.is_empty(),
        "text blocks should be materialized from snapshot without a second parse"
    );
}
