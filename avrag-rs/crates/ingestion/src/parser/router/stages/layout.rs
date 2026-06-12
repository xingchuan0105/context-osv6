use super::super::{PdfPageBackend, RouteDecision, RouteReason};
use super::super::super::probe::PdfPageProbeResult;

pub(in crate::parser::router) fn route(page: &PdfPageProbeResult) -> (PdfPageBackend, RouteDecision, RouteReason) {
    let _ = page;
    (
        PdfPageBackend::EdgeParse,
        RouteDecision::FastText,
        RouteReason::FastText,
    )
}
