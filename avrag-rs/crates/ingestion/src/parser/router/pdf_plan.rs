use super::stages;
use super::{
    ParsePlan, PdfPageBackend, PdfPagePlan, PdfParsePlan, RouteDecision, RouteReason,
};
use super::super::probe::{ParseProbeConfig, ParseProbeResult, PdfPageProbeResult};

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

pub(super) fn route_page(
    page: &PdfPageProbeResult,
) -> (PdfPageBackend, RouteDecision, RouteReason) {
    if let Some(result) = stages::fallback::route(page) {
        return result;
    }
    if let Some(result) = stages::table::route(page) {
        return result;
    }
    if let Some(result) = stages::figure::route(page) {
        return result;
    }
    stages::layout::route(page)
}

fn build_pdf_page_plan(page_probe: &PdfPageProbeResult, _config: &ParseProbeConfig) -> PdfPagePlan {
    let (backend, _, reason) = route_page(page_probe);
    PdfPagePlan {
        page_number: page_probe.page_number,
        backend,
        reason,
    }
}

pub(super) fn summarize_pdf_reason(probe_result: &ParseProbeResult, plan: &ParsePlan) -> RouteReason {
    let ParsePlan::Pdf(pdf_plan) = plan else {
        return RouteReason::SimplePdf;
    };

    let has_ocr = pdf_plan.pages.iter().any(|p| p.backend == PdfPageBackend::PaddleOcr);
    let has_visual = pdf_plan.pages.iter().any(|p| p.backend == PdfPageBackend::VisualRaster);
    let has_figures = pdf_plan.pages.iter().any(|p| p.reason == RouteReason::FastWithFigures);

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
