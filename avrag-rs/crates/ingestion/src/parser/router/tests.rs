use super::*;
use crate::parser::probe::{ParseProbeConfig, PdfPageProbeResult};
use super::pdf_plan::{build_pdf_parse_plan, route_page, summarize_pdf_reason};

#[test]
fn text_file_routing_uses_local_text_parser() {
    let decision = ParseRouter::route(b"hello world", "test.txt", "text/plain").unwrap();
    assert_eq!(decision.route, ParseRoute::Local);
    assert!(matches!(decision.reason, RouteReason::TextFile));
    assert!(matches!(
        decision.plan,
        ParsePlan::Local(LocalParsePlan {
            kind: LocalParseKind::Text
        })
    ));
}

#[test]
fn image_file_routing_uses_mineru_image_route() {
    let decision = ParseRouter::route(b"fake image", "test.png", "image/png").unwrap();
    assert_eq!(decision.route, ParseRoute::MineruImage);
    assert!(matches!(decision.reason, RouteReason::ImageFile));
}

#[test]
fn presentation_file_routing_uses_office_service() {
    let decision = ParseRouter::route(
        b"fake ppt",
        "test.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    )
    .unwrap();
    assert_eq!(decision.route, ParseRoute::OfficeService);
    assert!(matches!(decision.reason, RouteReason::PresentationFile));
    assert!(matches!(
        decision.plan,
        ParsePlan::Office(OfficeParsePlan {
            doc_type: OfficeDocType::Pptx
        })
    ));
}

#[test]
fn route_rejects_missing_extension() {
    let error = ParseRouter::route(b"hello", "README", "text/plain").expect_err("should fail");
    assert_eq!(error.code(), "unsupported_file_type");
}

#[test]
fn route_rejects_unknown_mime_type() {
    let error = ParseRouter::route(b"hello", "notes.txt", "application/octet-stream")
        .expect_err("should fail");
    assert_eq!(error.code(), "unsupported_file_type");
}

#[test]
fn pdf_page_plan_routes_each_page_independently() {
    let mut probe_result = ParseProbeResult::new("application/pdf".to_string(), "pdf".to_string());
    probe_result.page_count = Some(3);
    probe_result.pdf_page_probes = vec![
        PdfPageProbeResult {
            page_number: 1,
            extracted_text_chars: 400,
            image_hint_count: 0,
            table_hint_count: 0,
            likely_scanned: false,
            readable_ratio: Some(0.8),
            bigram_repeat_ratio: Some(0.1),
            unique_token_ratio: Some(0.7),
            watermark_hit: false,
            figure_area_ratio: None,
            non_decorative_image_count: None,
            table_garbled_ratio: None,
        },
        PdfPageProbeResult {
            page_number: 2,
            extracted_text_chars: 10,
            image_hint_count: 0,
            table_hint_count: 0,
            likely_scanned: true,
            readable_ratio: Some(0.2),
            bigram_repeat_ratio: Some(0.1),
            unique_token_ratio: Some(0.5),
            watermark_hit: false,
            figure_area_ratio: None,
            non_decorative_image_count: None,
            table_garbled_ratio: None,
        },
        PdfPageProbeResult {
            page_number: 3,
            extracted_text_chars: 350,
            image_hint_count: 0,
            table_hint_count: 0,
            likely_scanned: false,
            readable_ratio: Some(0.7),
            bigram_repeat_ratio: Some(0.1),
            unique_token_ratio: Some(0.6),
            watermark_hit: false,
            figure_area_ratio: None,
            non_decorative_image_count: None,
            table_garbled_ratio: None,
        },
    ];

    let plan = build_pdf_parse_plan(&probe_result, &ParseProbeConfig::default());
    assert_eq!(plan.pages.len(), 3);
    assert_eq!(plan.pages[0].backend, PdfPageBackend::EdgeParse);
    assert_eq!(plan.pages[1].backend, PdfPageBackend::PaddleOcr);
    assert_eq!(plan.pages[2].backend, PdfPageBackend::EdgeParse);

    let reason = summarize_pdf_reason(&probe_result, &ParsePlan::Pdf(plan));
    assert!(matches!(reason, RouteReason::ScannedPdf));
}

#[test]
fn route_page_scanned_goes_to_paddle() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 0,
        image_hint_count: 0,
        table_hint_count: 0,
        likely_scanned: true,
        readable_ratio: Some(0.0),
        bigram_repeat_ratio: Some(0.0),
        unique_token_ratio: Some(0.0),
        watermark_hit: false,
        figure_area_ratio: None,
        non_decorative_image_count: None,
        table_garbled_ratio: None,
    };
    let (_backend, decision, _reason) = route_page(&page);
    assert_eq!(decision, RouteDecision::SlowOcr);
}

#[test]
fn route_page_image_heavy_with_text_goes_to_b() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 500,
        image_hint_count: 3,
        table_hint_count: 0,
        likely_scanned: false,
        readable_ratio: Some(0.7),
        bigram_repeat_ratio: Some(0.1),
        unique_token_ratio: Some(0.6),
        watermark_hit: false,
        figure_area_ratio: None,
        non_decorative_image_count: None,
        table_garbled_ratio: None,
    };
    let (backend, decision, _reason) = route_page(&page);
    assert_eq!(backend, PdfPageBackend::EdgeParse);
    assert_eq!(decision, RouteDecision::FastWithFigures);
}

#[test]
fn route_page_watermark_goes_to_ocr() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 500,
        image_hint_count: 0,
        table_hint_count: 0,
        likely_scanned: false,
        readable_ratio: Some(0.9),
        bigram_repeat_ratio: Some(0.1),
        unique_token_ratio: Some(0.8),
        watermark_hit: true,
        figure_area_ratio: None,
        non_decorative_image_count: None,
        table_garbled_ratio: None,
    };
    let (_backend, decision, _reason) = route_page(&page);
    assert_eq!(decision, RouteDecision::SlowOcr);
}

#[test]
fn route_page_clean_text_goes_to_a() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 500,
        image_hint_count: 0,
        table_hint_count: 0,
        likely_scanned: false,
        readable_ratio: Some(0.8),
        bigram_repeat_ratio: Some(0.1),
        unique_token_ratio: Some(0.7),
        watermark_hit: false,
        figure_area_ratio: None,
        non_decorative_image_count: None,
        table_garbled_ratio: None,
    };
    let (backend, decision, _reason) = route_page(&page);
    assert_eq!(backend, PdfPageBackend::EdgeParse);
    assert_eq!(decision, RouteDecision::FastText);
}

#[test]
fn route_page_garbled_table_goes_to_c_prime() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 300,
        image_hint_count: 0,
        table_hint_count: 5,
        likely_scanned: false,
        readable_ratio: Some(0.5),
        bigram_repeat_ratio: Some(0.1),
        unique_token_ratio: Some(0.6),
        watermark_hit: false,
        figure_area_ratio: None,
        non_decorative_image_count: None,
        table_garbled_ratio: Some(0.45),
    };
    let (backend, decision, _reason) = route_page(&page);
    assert_eq!(backend, PdfPageBackend::PaddleOcr);
    assert_eq!(decision, RouteDecision::SlowOcrSinglePage);
}

#[test]
fn route_reason_labels_match_page_classes() {
    assert_eq!(RouteReason::FastText.route_label(), "A");
    assert_eq!(RouteReason::FastWithFigures.route_label(), "B");
    assert_eq!(RouteReason::SlowOcr.route_label(), "C");
    assert_eq!(RouteReason::SlowOcrSinglePage.route_label(), "C_prime");
    assert_eq!(RouteReason::OcrFallback.route_label(), "fallback");
    assert_eq!(RouteReason::SimplePdf.route_label(), "unknown");
}

#[test]
fn route_decision_labels_match_reason() {
    assert_eq!(RouteDecision::FastText.route_label(), "A");
    assert_eq!(RouteDecision::FastWithFigures.route_label(), "B");
    assert_eq!(RouteDecision::SlowOcr.route_label(), "C");
    assert_eq!(RouteDecision::SlowOcrSinglePage.route_label(), "C_prime");
    assert_eq!(RouteDecision::Fallback.route_label(), "fallback");
}
