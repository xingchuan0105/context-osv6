use super::super::{PdfPageBackend, RouteDecision, RouteReason};
use super::super::super::probe::PdfPageProbeResult;

const FIG_COUNT_THRESHOLD: usize = 2;
const FIG_RATIO_THRESHOLD: f32 = 0.15;

pub(in crate::parser::router) fn route(
    page: &PdfPageProbeResult,
) -> Option<(PdfPageBackend, RouteDecision, RouteReason)> {
    let has_figures = if let Some(ratio) = page.figure_area_ratio {
        let non_deco = page.non_decorative_image_count.unwrap_or(0);
        (ratio > FIG_RATIO_THRESHOLD && non_deco >= 2)
            || (ratio > 0.10 && page.image_hint_count >= 2)
    } else {
        page.image_hint_count >= FIG_COUNT_THRESHOLD
    };

    if has_figures {
        return Some((
            PdfPageBackend::EdgeParse,
            RouteDecision::FastWithFigures,
            RouteReason::FastWithFigures,
        ));
    }

    None
}
