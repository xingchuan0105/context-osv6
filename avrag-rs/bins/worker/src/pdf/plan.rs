use ingestion::parser::PdfParsePlan;
use tracing::info;

pub fn ingestion_pdf_max_pages() -> Option<usize> {
    std::env::var("INGESTION_PDF_MAX_PAGES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|pages| *pages > 0)
}

pub fn maybe_truncate_pdf_plan(plan: &mut PdfParsePlan) {
    let Some(max_pages) = ingestion_pdf_max_pages() else {
        return;
    };
    if plan.pages.len() <= max_pages {
        return;
    }
    plan.pages.truncate(max_pages);
    info!(
        max_pages,
        retained_pages = plan.pages.len(),
        "PDF plan truncated for ingestion page limit"
    );
}
