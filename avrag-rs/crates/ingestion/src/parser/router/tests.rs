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

fn assert_anydoc(decision: &ParseRouteDecision) {
    assert_local_kind(decision, LocalParseKind::Anydoc);
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

/// Office / spreadsheet → anydoc.
#[test]
fn docx_file_routing_uses_anydoc() {
    let decision = ParseRouter::route(
        b"fake docx",
        "test.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    )
    .unwrap();
    assert_anydoc(&decision);
    assert!(matches!(decision.reason, RouteReason::OfficeDocument));
}

/// Presentations → anydoc.
#[test]
fn pptx_file_routing_uses_anydoc() {
    let decision = ParseRouter::route(
        b"fake pptx",
        "test.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    )
    .unwrap();
    assert_anydoc(&decision);
    assert!(matches!(decision.reason, RouteReason::PresentationFile));
}

/// Excel → anydoc（含 xls）。
#[test]
fn excel_routing_uses_anydoc() {
    let decision = ParseRouter::route(
        b"fake xlsx",
        "ipd.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    )
    .unwrap();
    assert_anydoc(&decision);

    let decision =
        ParseRouter::route(b"fake xls", "legacy.xls", "application/vnd.ms-excel").unwrap();
    assert_anydoc(&decision);
}

/// PDF 走 liteparse（liteparse_v2_pdf）—— 永不 anydoc。
#[test]
fn pdf_routing_uses_liteparse_v2_pdf() {
    let decision = ParseRouter::route(b"%PDF-1.7 fake", "report.pdf", "application/pdf").unwrap();
    assert_liteparse_v2_pdf(&decision);
    assert!(matches!(decision.reason, RouteReason::OfficeDocument));
}

/// 旧二进制 doc/ppt → anydoc。
#[test]
fn doc_ppt_routing_uses_anydoc() {
    let decision = ParseRouter::route(b"fake doc", "legacy.doc", "application/msword").unwrap();
    assert_anydoc(&decision);

    let decision = ParseRouter::route(
        b"fake ppt",
        "legacy.ppt",
        "application/vnd.ms-powerpoint",
    )
    .unwrap();
    assert_anydoc(&decision);
    assert!(matches!(decision.reason, RouteReason::PresentationFile));
}

/// 扩展 anydoc 面：csv / odt / rtf / epub / docm / xlsm。
#[test]
fn expanded_anydoc_formats_route_to_anydoc() {
    let cases: &[(&str, &str, RouteReason)] = &[
        ("a.csv", "text/csv", RouteReason::OfficeDocument),
        (
            "a.odt",
            "application/vnd.oasis.opendocument.text",
            RouteReason::OfficeDocument,
        ),
        ("a.rtf", "application/rtf", RouteReason::OfficeDocument),
        ("a.epub", "application/epub+zip", RouteReason::OfficeDocument),
        (
            "a.docm",
            "application/vnd.ms-word.document.macroenabled.12",
            RouteReason::OfficeDocument,
        ),
        (
            "a.xlsm",
            "application/vnd.ms-excel.sheet.macroenabled.12",
            RouteReason::OfficeDocument,
        ),
        (
            "a.odp",
            "application/vnd.oasis.opendocument.presentation",
            RouteReason::PresentationFile,
        ),
    ];
    for (name, mime, reason) in cases {
        let decision = ParseRouter::route(b"x", name, mime).unwrap();
        assert_anydoc(&decision);
        assert_eq!(
            std::mem::discriminant(&decision.reason),
            std::mem::discriminant(reason),
            "{name}"
        );
    }
}

/// TSV 仍 markitdown（anydoc 不宣称支持）。
#[test]
fn tsv_stays_markitdown() {
    let decision =
        ParseRouter::route(b"a\tb", "t.tsv", "text/tab-separated-values").unwrap();
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
