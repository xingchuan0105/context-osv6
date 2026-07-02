use super::super::probe::{ParseProbeConfig, ParseProbeResult, PdfPageProbeResult};
#[cfg(test)]
use super::RouteDecision;
use super::page_routes::route_page_with_config;
use super::{PageRouteKind, ParsePlan, PdfPageBackend, PdfPagePlan, PdfParsePlan, RouteReason};

pub(super) fn build_pdf_parse_plan(
    probe_result: &ParseProbeResult,
    config: &ParseProbeConfig,
) -> PdfParsePlan {
    let fallback_backend = PdfPageBackend::PaddleOcr;
    let fallback_reason = RouteReason::ScannedPdf;

    let pages = if probe_result.pdf_page_probes.is_empty() {
        let page_count = probe_result.page_count.unwrap_or(1).max(1);
        (1..=page_count)
            .map(|page_number| PdfPagePlan {
                page_number,
                backend: fallback_backend.clone(),
                reason: fallback_reason.clone(),
                route_kinds: vec![PageRouteKind::ScanOcr],
            })
            .collect()
    } else {
        probe_result
            .pdf_page_probes
            .iter()
            .map(|p| build_pdf_page_plan(p, config))
            .collect()
    };

    PdfParsePlan { pages }
}

#[cfg(test)]
pub(super) fn route_page(
    page: &PdfPageProbeResult,
) -> (PdfPageBackend, RouteDecision, RouteReason) {
    let (backend, decision, reason, _routes) =
        route_page_with_config(page, &ParseProbeConfig::default());
    (backend, decision, reason)
}

fn build_pdf_page_plan(page_probe: &PdfPageProbeResult, config: &ParseProbeConfig) -> PdfPagePlan {
    let (backend, _, reason, route_kinds) = route_page_with_config(page_probe, config);
    PdfPagePlan {
        page_number: page_probe.page_number,
        backend,
        reason,
        route_kinds,
    }
}

pub(super) fn summarize_pdf_reason(
    probe_result: &ParseProbeResult,
    plan: &ParsePlan,
) -> RouteReason {
    let ParsePlan::Pdf(pdf_plan) = plan else {
        return RouteReason::SimplePdf;
    };

    let has_ocr = pdf_plan.pages.iter().any(|p| {
        p.route_kinds
            .iter()
            .any(|k| matches!(k, PageRouteKind::ScanOcr | PageRouteKind::TableOcr))
    });
    let has_visual = pdf_plan
        .pages
        .iter()
        .any(|p| p.backend == PdfPageBackend::VisualRaster);
    let has_figures = pdf_plan
        .pages
        .iter()
        .any(|p| p.route_kinds.contains(&PageRouteKind::Figure));

    if has_ocr {
        RouteReason::ScannedPdf
    } else if has_visual || probe_result.likely_scanned {
        RouteReason::ComplexPdf
    } else if has_figures {
        RouteReason::ComplexPdf
    } else {
        RouteReason::SimplePdf
    }
}

pub fn pdf_page_route_labels(plan: &PdfPagePlan) -> String {
    let mut labels: Vec<&str> = plan.route_kinds.iter().map(|k| k.as_label()).collect();
    if labels.is_empty() {
        labels.push(plan.reason.route_label());
    }
    labels.join("+")
}
