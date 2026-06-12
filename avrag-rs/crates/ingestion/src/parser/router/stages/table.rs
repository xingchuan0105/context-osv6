use super::super::{PdfPageBackend, RouteDecision, RouteReason};
use super::super::super::probe::PdfPageProbeResult;

const TABLE_GARBLE_THRESHOLD: f32 = 0.30;
#[allow(dead_code)]
const TABLE_QUAL_THRESHOLD: f32 = 0.6;

pub(in crate::parser::router) fn route(
    page: &PdfPageProbeResult,
) -> Option<(PdfPageBackend, RouteDecision, RouteReason)> {
    if page.table_hint_count > 0 {
        let garbled = page.table_garbled_ratio.unwrap_or(0.0);
        if garbled > TABLE_GARBLE_THRESHOLD {
            return Some((
                PdfPageBackend::PaddleOcr,
                RouteDecision::SlowOcrSinglePage,
                RouteReason::SlowOcrSinglePage,
            ));
        }
    }

    None
}
