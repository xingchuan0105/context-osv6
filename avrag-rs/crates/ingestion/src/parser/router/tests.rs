use super::*;

fn assert_markitdown(decision: &ParseRouteDecision) {
    assert_eq!(decision.route, ParseRoute::Local);
    assert!(matches!(
        decision.plan,
        ParsePlan::Local(LocalParsePlan {
            kind: LocalParseKind::Markitdown
        })
    ));
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

#[test]
fn docx_file_routing_uses_markitdown() {
    let decision = ParseRouter::route(
        b"fake docx",
        "test.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    )
    .unwrap();
    assert_markitdown(&decision);
    assert!(matches!(decision.reason, RouteReason::OfficeDocument));
}

#[test]
fn pptx_file_routing_uses_markitdown() {
    let decision = ParseRouter::route(
        b"fake pptx",
        "test.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    )
    .unwrap();
    assert_markitdown(&decision);
    assert!(matches!(decision.reason, RouteReason::PresentationFile));
}

/// markitdown 唯一文档解析器：xlsx/xls 不再走 calamine 进程内解析。
#[test]
fn excel_routing_uses_markitdown() {
    let decision = ParseRouter::route(
        b"fake xlsx",
        "ipd.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    )
    .unwrap();
    assert_markitdown(&decision);

    let decision =
        ParseRouter::route(b"fake xls", "legacy.xls", "application/vnd.ms-excel").unwrap();
    assert_markitdown(&decision);
}

/// markitdown 唯一文档解析器：pdf 不再走 liteparse/VLM 页路由管线。
#[test]
fn pdf_routing_uses_markitdown() {
    let decision = ParseRouter::route(b"%PDF-1.7 fake", "report.pdf", "application/pdf").unwrap();
    assert_markitdown(&decision);
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
