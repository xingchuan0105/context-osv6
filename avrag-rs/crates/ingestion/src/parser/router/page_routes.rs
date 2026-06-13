use super::{PageRouteKind, PdfPageBackend, RouteDecision, RouteReason};
use super::super::probe::{
    PdfPageProbeResult, BIGRAM_REPEAT_THRESHOLD, PAGE_TEXT_THRESHOLD, UNIQUE_TOKEN_THRESHOLD,
};
use super::super::ParseProbeConfig;

/// §5.2 priority: D → C → B → A (combinable).
pub(in crate::parser::router) fn classify_page_routes(
    page: &PdfPageProbeResult,
    config: &ParseProbeConfig,
) -> Vec<PageRouteKind> {
    let mut routes = Vec::new();

    let readable = page.readable_ratio.unwrap_or(1.0);
    let bigram = page.bigram_repeat_ratio.unwrap_or(0.0);
    let unique = page.unique_token_ratio.unwrap_or(1.0);

    let is_scan = page.extracted_text_chars < config.scanned_page_threshold
        || readable < config.text_qual_threshold
        || page.extracted_text_chars == 0
        || bigram > BIGRAM_REPEAT_THRESHOLD
        || page.watermark_hit
        || (page.extracted_text_chars < PAGE_TEXT_THRESHOLD && readable < 0.5)
        || unique < UNIQUE_TOKEN_THRESHOLD;

    if is_scan {
        routes.push(PageRouteKind::ScanOcr);
    }

    let garbled = page.table_garbled_ratio.unwrap_or(0.0);
    let table_garble_heavy =
        page.table_hint_count > 0 && garbled > config.table_garble_threshold;
    let table_ocr = table_garble_heavy
        || (!is_scan && page.table_hint_count > config.table_heavy_threshold);
    if table_ocr {
        routes.push(PageRouteKind::TableOcr);
    }

    let has_figures = if let Some(ratio) = page.figure_area_ratio {
        let non_deco = page.non_decorative_image_count.unwrap_or(0);
        ratio > config.fig_ratio_threshold && non_deco >= config.fig_count_threshold
    } else {
        page.image_hint_count >= config.fig_count_threshold
    };
    if has_figures && !is_scan {
        routes.push(PageRouteKind::Figure);
    }

    if !is_scan && !table_garble_heavy {
        routes.push(PageRouteKind::Text);
    }

    if routes.is_empty() {
        routes.push(PageRouteKind::ScanOcr);
    }

    routes
}

pub(in crate::parser::router) fn routes_to_backend(routes: &[PageRouteKind]) -> PdfPageBackend {
    if routes.iter().any(|r| matches!(r, PageRouteKind::ScanOcr | PageRouteKind::TableOcr)) {
        if routes.iter().any(|r| matches!(r, PageRouteKind::Text | PageRouteKind::Figure)) {
            // Combined page: text via LiteParse, OCR via paddle in worker
            PdfPageBackend::EdgeParse
        } else {
            PdfPageBackend::PaddleOcr
        }
    } else {
        PdfPageBackend::EdgeParse
    }
}

pub(in crate::parser::router) fn routes_to_decision(routes: &[PageRouteKind]) -> RouteDecision {
    if routes.contains(&PageRouteKind::ScanOcr) {
        RouteDecision::SlowOcr
    } else if routes.contains(&PageRouteKind::TableOcr) {
        RouteDecision::SlowOcrSinglePage
    } else if routes.contains(&PageRouteKind::Figure) {
        RouteDecision::FastWithFigures
    } else {
        RouteDecision::FastText
    }
}

pub(in crate::parser::router) fn routes_to_reason(routes: &[PageRouteKind]) -> RouteReason {
    if routes.contains(&PageRouteKind::ScanOcr) {
        RouteReason::SlowOcr
    } else if routes.contains(&PageRouteKind::TableOcr) {
        RouteReason::SlowOcrSinglePage
    } else if routes.contains(&PageRouteKind::Figure) {
        RouteReason::FastWithFigures
    } else {
        RouteReason::FastText
    }
}

pub(in crate::parser::router) fn route_page_with_config(
    page: &PdfPageProbeResult,
    config: &ParseProbeConfig,
) -> (
    PdfPageBackend,
    RouteDecision,
    RouteReason,
    Vec<PageRouteKind>,
) {
    let routes = classify_page_routes(page, config);
    (
        routes_to_backend(&routes),
        routes_to_decision(&routes),
        routes_to_reason(&routes),
        routes,
    )
}
