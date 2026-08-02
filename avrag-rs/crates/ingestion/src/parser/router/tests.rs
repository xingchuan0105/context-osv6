use super::*;

fn assert_local_kind(decision: &ParseRouteDecision, kind: LocalParseKind) {
    assert_eq!(decision.route, ParseRoute::Local);
    assert!(matches!(
        decision.plan,
        ParsePlan::Local(LocalParsePlan { kind: ref k }) if *k == kind
    ));
}

fn assert_markitdown(decision: &ParseRouteDecision) {
    assert_local_kind(decision, LocalParseKind::Markitdown);
}

fn assert_office_direct(decision: &ParseRouteDecision) {
    assert_local_kind(decision, LocalParseKind::OfficeDirect);
}

fn assert_liteparse_v2_pdf(decision: &ParseRouteDecision) {
    assert_local_kind(decision, LocalParseKind::LiteparseV2Pdf);
}

#[test]
fn text_file_routing_uses_markitdown() {
    let decision = ParseRouter::route(b"hello world", "test.txt", "text/plain").unwrap();
    assert_markitdown(&decision);
    assert!(matches!(decision.reason, RouteReason::TextFile));
}

#[test]
fn markdown_file_routing_uses_markitdown() {
    let decision = ParseRouter::route(b"# t", "notes.md", "text/markdown").unwrap();
    assert_markitdown(&decision);
    assert!(matches!(decision.reason, RouteReason::TextFile));
}

#[test]
fn image_file_routing_uses_paddle_ocr_image_route() {
    let decision = ParseRouter::route(b"fake image", "test.png", "image/png").unwrap();
    assert_eq!(decision.route, ParseRoute::PaddleOcrImage);
    assert!(matches!(decision.reason, RouteReason::ImageFile));
    assert!(matches!(
        decision.plan,
        ParsePlan::External(ExternalParsePlan {
            kind: ExternalParseKind::PaddleOcrImage
        })
    ));
}

/// Office 直读：docx/doc/xls/xlsx 走 OfficeDirect。
#[test]
fn docx_file_routing_uses_office_direct() {
    let decision = ParseRouter::route(
        b"fake docx",
        "test.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    )
    .unwrap();
    assert_office_direct(&decision);
    assert!(matches!(decision.reason, RouteReason::OfficeDocument));
}

/// Office 直读：pptx/ppt 走 OfficeDirect。
#[test]
fn pptx_file_routing_uses_office_direct() {
    let decision = ParseRouter::route(
        b"fake pptx",
        "test.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    )
    .unwrap();
    assert_office_direct(&decision);
    assert!(matches!(decision.reason, RouteReason::PresentationFile));
}

/// Office 直读：xlsx/xls 走 OfficeDirect（不再经 markitdown/calamine）。
#[test]
fn excel_routing_uses_office_direct() {
    let decision = ParseRouter::route(
        b"fake xlsx",
        "ipd.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    )
    .unwrap();
    assert_office_direct(&decision);

    let decision =
        ParseRouter::route(b"fake xls", "legacy.xls", "application/vnd.ms-excel").unwrap();
    assert_office_direct(&decision);
}

/// PDF 走 liteparse（liteparse_v2_pdf）。
#[test]
fn pdf_routing_uses_liteparse_v2_pdf() {
    let decision = ParseRouter::route(b"%PDF-1.7 fake", "report.pdf", "application/pdf").unwrap();
    assert_liteparse_v2_pdf(&decision);
    assert!(matches!(decision.reason, RouteReason::OfficeDocument));
}

/// 旧二进制 doc/ppt 走 OfficeDirect。
#[test]
fn doc_ppt_routing_uses_office_direct() {
    let decision = ParseRouter::route(
        b"fake doc",
        "legacy.doc",
        "application/msword",
    )
    .unwrap();
    assert_office_direct(&decision);

    let decision = ParseRouter::route(
        b"fake ppt",
        "legacy.ppt",
        "application/vnd.ms-powerpoint",
    )
    .unwrap();
    assert_office_direct(&decision);
    assert!(matches!(decision.reason, RouteReason::PresentationFile));
}

#[test]
fn code_file_routing_uses_markitdown() {
    let decision = ParseRouter::route(b"fn main() {}", "main.rs", "text/x-rust").unwrap();
    assert_markitdown(&decision);
    assert!(matches!(decision.reason, RouteReason::TextFile));
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
fn route_rejects_mismatched_mime_type() {
    let error =
        ParseRouter::route(b"hello", "notes.txt", "application/pdf").expect_err("should fail");
    assert_eq!(error.code(), "unsupported_file_type");
}
