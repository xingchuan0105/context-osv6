use std::path::PathBuf;

use ingestion::parser::{ParsePlan, ParseRoute, ParseRouter, PdfPageBackend};
use ingestion::LiteParseService;

fn phase0_mini_pdf() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/spike/fixtures/phase0-mini.pdf")
}

#[test]
fn router_builds_per_page_plan_for_mini_pdf() {
    let path = phase0_mini_pdf();
    assert!(path.exists(), "missing fixture at {}", path.display());
    let bytes = std::fs::read(&path).expect("read fixture pdf");

    let decision =
        ParseRouter::route(&bytes, "phase0-mini.pdf", "application/pdf").expect("route pdf");
    assert_eq!(decision.route, ParseRoute::Pdf);
    let ParsePlan::Pdf(plan) = decision.plan else {
        panic!("expected pdf plan");
    };
    assert!(!plan.pages.is_empty(), "expected per-page routing");
    assert!(
        plan.pages
            .iter()
            .any(|p| p.backend == PdfPageBackend::EdgeParse),
        "digital text pages should stay on edge/liteparse path"
    );
}

#[tokio::test]
async fn liteparse_probe_and_extract_mini_pdf() {
    let path = phase0_mini_pdf();
    let bytes = std::fs::read(&path).expect("read fixture pdf");
    let service = LiteParseService::from_env();

    let probes = service.probe(&bytes).await.expect("liteparse probe");
    assert!(!probes.is_empty(), "probe should return page signals");
    assert!(
        probes.iter().any(|p| p.extracted_text_chars > 0),
        "mini pdf should contain extractable text"
    );

    let blocks = service
        .extract_blocks(&bytes, &[])
        .await
        .expect("liteparse extract");
    assert!(!blocks.is_empty(), "extract should yield text blocks");
    assert!(
        blocks.iter().all(|b| b.bbox[2] >= b.bbox[0] && b.bbox[3] >= b.bbox[1]),
        "bbox should be normalized x0,y0,x1,y1"
    );
}

#[test]
fn mini_pdf_page_routes_include_scan_and_text_pages() {
    use ingestion::parser::{PageRouteKind, ParsePlan, ParseRouter};

    let bytes = std::fs::read(phase0_mini_pdf()).expect("read fixture pdf");
    let decision =
        ParseRouter::route(&bytes, "phase0-mini.pdf", "application/pdf").expect("route pdf");
    let ParsePlan::Pdf(plan) = decision.plan else {
        panic!("expected pdf plan");
    };
    let scan_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.route_kinds.contains(&PageRouteKind::ScanOcr))
        .map(|p| p.page_number)
        .collect();
    assert!(
        !scan_pages.is_empty(),
        "fixture should include at least one scan-routed page for paddle/visual fallback tests"
    );
}
