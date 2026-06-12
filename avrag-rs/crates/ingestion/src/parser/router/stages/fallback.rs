use super::super::{PdfPageBackend, RouteDecision, RouteReason};
use super::super::super::probe::{
    PdfPageProbeResult, BIGRAM_REPEAT_THRESHOLD, PAGE_TEXT_THRESHOLD, TEXT_QUAL_THRESHOLD,
    UNIQUE_TOKEN_THRESHOLD,
};

pub(in crate::parser::router) fn route(
    page: &PdfPageProbeResult,
) -> Option<(PdfPageBackend, RouteDecision, RouteReason)> {
    let readable = page.readable_ratio.unwrap_or(1.0);
    let bigram = page.bigram_repeat_ratio.unwrap_or(0.0);
    let unique = page.unique_token_ratio.unwrap_or(1.0);

    if page.extracted_text_chars == 0
        || readable < TEXT_QUAL_THRESHOLD
        || bigram > BIGRAM_REPEAT_THRESHOLD
        || page.watermark_hit
        || (page.extracted_text_chars < PAGE_TEXT_THRESHOLD && readable < 0.5)
        || unique < UNIQUE_TOKEN_THRESHOLD
    {
        return Some((
            PdfPageBackend::PaddleOcr,
            RouteDecision::SlowOcr,
            RouteReason::SlowOcr,
        ));
    }

    None
}
