use std::collections::{BTreeMap, HashSet};

use ingestion::parser::{
    append_liteparse_blocks_to_ir, blocks_to_document_ir, page_has_searchable_text,
    LiteParseService, PageRouteKind, ParsedPdfSnapshot, PdfPageBackend, PdfParsePlan,
    VisualPdfParser,
};
use ingestion::{
    DocumentIr, IngestionError, LiteParseConfig, PageParseStatus, PageStatusEntry, ParseBackend,
    ParseWarning,
};
use uuid::Uuid;

use super::b_class::enrich_b_class_figures;
use super::context::PdfParseContext;
use super::merge::merge_pdf_ir;
use super::paddle::{execute_paddle_ocr_per_page, PaddlePerPageOcrOutcome};

// ---------------------------------------------------------------------------
// route: page sets derived from the parse plan
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PdfPageRoutes {
    pub text_pages: Vec<u32>,
    pub ocr_pages: Vec<u32>,
    pub table_ocr_pages: HashSet<u32>,
}

pub fn collect_page_routes(plan: &PdfParsePlan) -> PdfPageRoutes {
    let text_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| {
            p.route_kinds.iter().any(|k| {
                matches!(k, PageRouteKind::Text | PageRouteKind::Figure)
            })
        })
        .map(|p| p.page_number)
        .collect();

    let ocr_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| {
            p.route_kinds.iter().any(|k| {
                matches!(k, PageRouteKind::ScanOcr | PageRouteKind::TableOcr)
            })
        })
        .map(|p| p.page_number)
        .collect();

    let table_ocr_pages: HashSet<u32> = plan
        .pages
        .iter()
        .filter(|p| p.route_kinds.contains(&PageRouteKind::TableOcr))
        .map(|p| p.page_number)
        .collect();

    PdfPageRoutes {
        text_pages,
        ocr_pages,
        table_ocr_pages,
    }
}

// ---------------------------------------------------------------------------
// probe: LiteParse dimensions + digital text extraction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PdfProbeOutcome {
    pub page_dimensions: BTreeMap<u32, (f32, f32)>,
    pub digital_ir: Option<DocumentIr>,
}

pub fn probe_pdf_content_from_snapshot(
    snapshot: &ParsedPdfSnapshot,
    filename: &str,
    document_id: Uuid,
    routes: &PdfPageRoutes,
) -> PdfProbeOutcome {
    let page_dimensions = snapshot.page_dimensions().clone();

    let digital_ir = if routes.text_pages.is_empty() {
        None
    } else {
        let blocks = snapshot.extract_blocks_for_pages(&routes.text_pages);
        Some(blocks_to_document_ir(
            document_id,
            filename,
            &blocks,
            &page_dimensions,
        ))
    };

    PdfProbeOutcome {
        page_dimensions,
        digital_ir,
    }
}

#[allow(dead_code)]
pub async fn probe_pdf_content(
    service: &LiteParseService,
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    routes: &PdfPageRoutes,
) -> Result<PdfProbeOutcome, IngestionError> {
    let snapshot = service.parse_pdf_document(bytes).await.map_err(|e| {
        IngestionError::StateSink(format!("LiteParse parse failed for {filename}: {e}"))
    })?;
    Ok(probe_pdf_content_from_snapshot(
        &snapshot,
        filename,
        document_id,
        routes,
    ))
}

// ---------------------------------------------------------------------------
// execute: Paddle Jobs OCR per routed page
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OcrPagesOutcome {
    pub paddle_ir: Option<DocumentIr>,
    pub successful_pages: HashSet<u32>,
    pub budget_skipped: Vec<u32>,
    pub failed_pages: Vec<u32>,
    pub jobs_submitted: usize,
    pub cache_hits: usize,
}

pub async fn run_ocr_pages(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    routes: &PdfPageRoutes,
) -> OcrPagesOutcome {
    if routes.ocr_pages.is_empty() {
        return OcrPagesOutcome {
            paddle_ir: None,
            successful_pages: HashSet::new(),
            budget_skipped: Vec::new(),
            failed_pages: Vec::new(),
            jobs_submitted: 0,
            cache_hits: 0,
        };
    }

    match execute_paddle_ocr_per_page(
        bytes,
        filename,
        document_id,
        &routes.ocr_pages,
        &routes.table_ocr_pages,
    )
    .await
    {
        Ok(PaddlePerPageOcrOutcome {
            ir,
            successful_pages,
            failed_pages,
            budget_skipped_pages,
            jobs_submitted,
            cache_hits,
        }) => OcrPagesOutcome {
            paddle_ir: Some(ir),
            successful_pages,
            budget_skipped: budget_skipped_pages,
            failed_pages,
            jobs_submitted,
            cache_hits,
        },
        Err(e) => {
            tracing::warn!(filename, error = %e, "Paddle Jobs OCR setup failed");
            OcrPagesOutcome {
                paddle_ir: None,
                successful_pages: HashSet::new(),
                budget_skipped: Vec::new(),
                failed_pages: routes.ocr_pages.clone(),
                jobs_submitted: 0,
                cache_hits: 0,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// merge prep: budget degrade, LiteParse fallback, visual fallback
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TextFallbackOutcome {
    pub digital_ir: Option<DocumentIr>,
    pub visual_ir: Option<DocumentIr>,
    #[allow(dead_code)]
    pub paddle_needs_fallback: Vec<u32>,
}

pub async fn apply_text_fallbacks(
    ctx: &PdfParseContext,
    snapshot: &ParsedPdfSnapshot,
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    routes: &PdfPageRoutes,
    page_dimensions: &BTreeMap<u32, (f32, f32)>,
    ocr: &OcrPagesOutcome,
    mut digital_ir: Option<DocumentIr>,
) -> Result<TextFallbackOutcome, IngestionError> {
    if !ocr.budget_skipped.is_empty() {
        let degrade_blocks = snapshot.extract_blocks_for_pages(&ocr.budget_skipped);
        if digital_ir.is_none() && !degrade_blocks.is_empty() {
            digital_ir = Some(blocks_to_document_ir(
                document_id,
                filename,
                &degrade_blocks,
                page_dimensions,
            ));
        } else if let Some(ir) = digital_ir.as_mut() {
            append_liteparse_blocks_to_ir(ir, &degrade_blocks);
        }
    }

    let mut paddle_needs_fallback: Vec<u32> = routes
        .ocr_pages
        .iter()
        .filter(|p| !ocr.successful_pages.contains(p))
        .copied()
        .collect();
    for page in &ocr.budget_skipped {
        let has_text = digital_ir
            .as_ref()
            .is_some_and(|ir| page_has_searchable_text(ir, *page));
        if !has_text && !paddle_needs_fallback.contains(page) {
            paddle_needs_fallback.push(*page);
        }
    }
    paddle_needs_fallback.sort();
    paddle_needs_fallback.dedup();

    if !paddle_needs_fallback.is_empty() {
        let lp_fallback_blocks = snapshot.extract_blocks_for_pages(&paddle_needs_fallback);
        if !lp_fallback_blocks.is_empty() {
            if let Some(ir) = digital_ir.as_mut() {
                append_liteparse_blocks_to_ir(ir, &lp_fallback_blocks);
            } else {
                digital_ir = Some(blocks_to_document_ir(
                    document_id,
                    filename,
                    &lp_fallback_blocks,
                    page_dimensions,
                ));
            }
        }
        paddle_needs_fallback.retain(|page| {
            !digital_ir
                .as_ref()
                .is_some_and(|ir| page_has_searchable_text(ir, *page))
        });
    }

    let visual_ir = if paddle_needs_fallback.is_empty() {
        None
    } else {
        let renderer = ctx.pdf_renderer_client.as_ref().ok_or_else(|| {
            IngestionError::StateSink(format!(
                "PDF E-class fallback for {filename}, but PDF_RENDERER_BASE_URL is not configured"
            ))
        })?;
        let parser = VisualPdfParser::new(renderer.clone());
        Some(
            parser
                .parse_pages(bytes, filename, document_id, &paddle_needs_fallback)
                .await
                .map_err(|error| {
                    IngestionError::StateSink(format!(
                        "visual pdf fallback failed for {filename}: {error}"
                    ))
                })?,
        )
    };

    Ok(TextFallbackOutcome {
        digital_ir,
        visual_ir,
        paddle_needs_fallback,
    })
}

// ---------------------------------------------------------------------------
// merge + metadata: IR merge, ingest metadata, page status
// ---------------------------------------------------------------------------

pub async fn attach_ingest_metadata_and_status(
    ctx: &PdfParseContext,
    bytes: &[u8],
    plan: &PdfParsePlan,
    routes: &PdfPageRoutes,
    ocr: &OcrPagesOutcome,
    mut merged: DocumentIr,
) -> DocumentIr {
    merged
        .metadata
        .insert("ingest_route_version".to_string(), "liteparse-v1".to_string());
    merged
        .metadata
        .insert("pdf_route_mode".to_string(), "liteparse_hybrid".to_string());
    if !routes.ocr_pages.is_empty() {
        merged.metadata.insert(
            "paddle_jobs_requested".to_string(),
            routes.ocr_pages.len().to_string(),
        );
        merged.metadata.insert(
            "paddle_jobs_count".to_string(),
            ocr.jobs_submitted.to_string(),
        );
        merged
            .metadata
            .insert("paddle_jobs_used".to_string(), ocr.jobs_submitted.to_string());
        merged.metadata.insert(
            "ocr_backend".to_string(),
            "paddle_jobs".to_string(),
        );
        if ocr.cache_hits > 0 {
            merged.metadata.insert(
                "paddle_cache_hits".to_string(),
                ocr.cache_hits.to_string(),
            );
        }
        if !ocr.budget_skipped.is_empty() {
            merged.metadata.insert(
                "paddle_jobs_budget_skipped".to_string(),
                ocr.budget_skipped.len().to_string(),
            );
            for page in &ocr.budget_skipped {
                merged.warnings.push(ParseWarning {
                    code: "paddle_job_budget_exhausted".to_string(),
                    message: format!(
                        "page {page} skipped Paddle Job due to document budget; degraded to LiteParse or visual"
                    ),
                    page: Some(*page),
                    backend: ParseBackend::PaddleOcrPdf,
                });
            }
        }
        for page in &ocr.failed_pages {
            merged.warnings.push(ParseWarning {
                code: "paddle_job_failed".to_string(),
                message: format!(
                    "page {page} Paddle Job failed; degraded to LiteParse or visual"
                ),
                page: Some(*page),
                backend: ParseBackend::PaddleOcrPdf,
            });
        }
        if !ocr.failed_pages.is_empty() {
            merged
                .metadata
                .insert("degraded".to_string(), "true".to_string());
        }
    }

    enrich_b_class_figures(ctx, bytes, &mut merged, plan).await;
    let failed_pages_set: HashSet<u32> = ocr.failed_pages.iter().copied().collect();
    attach_page_status(
        &mut merged,
        plan,
        &ocr.successful_pages,
        &ocr.budget_skipped,
        &failed_pages_set,
    );
    merged
}

pub async fn execute_pdf_parse(
    ctx: &PdfParseContext,
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    plan: &PdfParsePlan,
    liteparse_snapshot: Option<&ParsedPdfSnapshot>,
) -> Result<DocumentIr, IngestionError> {
    // LiteParse happy path: routing already parsed once; worker reuses `liteparse_snapshot`
    // (see `probe_pdf_hybrid` + `ParseRouteDecision.liteparse_snapshot`). Office→PDF re-route
    // may omit the snapshot; we parse at most once locally in that case.
    let routes = collect_page_routes(plan);
    let lp_config = LiteParseConfig::from_env();
    let service = LiteParseService::new(lp_config);
    let owned_snapshot;
    let snapshot = match liteparse_snapshot {
        Some(snapshot) => snapshot,
        None => {
            owned_snapshot = service.parse_pdf_document(bytes).await.map_err(|e| {
                IngestionError::StateSink(format!("LiteParse parse failed for {filename}: {e}"))
            })?;
            &owned_snapshot
        }
    };

    let probe = probe_pdf_content_from_snapshot(snapshot, filename, document_id, &routes);
    let page_dimensions = probe.page_dimensions;
    let digital_ir = probe.digital_ir;
    let ocr = run_ocr_pages(bytes, filename, document_id, &routes).await;
    let fallbacks = apply_text_fallbacks(
        ctx,
        snapshot,
        bytes,
        filename,
        document_id,
        &routes,
        &page_dimensions,
        &ocr,
        digital_ir,
    )
    .await?;

    let ocr_meta = OcrPagesOutcome {
        paddle_ir: None,
        successful_pages: ocr.successful_pages.clone(),
        budget_skipped: ocr.budget_skipped.clone(),
        failed_pages: ocr.failed_pages.clone(),
        jobs_submitted: ocr.jobs_submitted,
        cache_hits: ocr.cache_hits,
    };

    let merged = merge_pdf_ir(
        document_id,
        filename,
        plan,
        fallbacks.digital_ir,
        ocr.paddle_ir,
        fallbacks.visual_ir,
        &ocr_meta.successful_pages,
    )?;

    Ok(attach_ingest_metadata_and_status(
        ctx, bytes, plan, &routes, &ocr_meta, merged,
    )
    .await)
}

fn page_route_label(plan_page: &ingestion::parser::PdfPagePlan) -> String {
    if plan_page.route_kinds.is_empty() {
        plan_page.reason.route_label().to_string()
    } else {
        plan_page
            .route_kinds
            .iter()
            .map(|k| k.as_label())
            .collect::<Vec<_>>()
            .join("+")
    }
}

fn attach_page_status(
    merged: &mut DocumentIr,
    plan: &PdfParsePlan,
    paddle_successful: &HashSet<u32>,
    budget_skipped: &[u32],
    failed_pages: &HashSet<u32>,
) {
    let budget_skipped: HashSet<u32> = budget_skipped.iter().copied().collect();
    let page_status: Vec<PageStatusEntry> = plan
        .pages
        .iter()
        .map(|p| {
            let needs_ocr = p.route_kinds.iter().any(|k| {
                matches!(k, PageRouteKind::ScanOcr | PageRouteKind::TableOcr)
            }) || p.backend == PdfPageBackend::PaddleOcr;
            let actual_page = merged.pages.iter().find(|mp| mp.page_number == p.page_number);
            let status = if budget_skipped.contains(&p.page_number) {
                if page_has_searchable_text(merged, p.page_number) || actual_page.is_some() {
                    PageParseStatus::Partial
                } else {
                    PageParseStatus::OcrFail
                }
            } else if failed_pages.contains(&p.page_number) {
                if page_has_searchable_text(merged, p.page_number) || actual_page.is_some() {
                    PageParseStatus::Partial
                } else {
                    PageParseStatus::OcrFail
                }
            } else if needs_ocr && !paddle_successful.contains(&p.page_number) {
                match actual_page {
                    Some(pg) if pg.backend == ParseBackend::PaddleOcrPdf => PageParseStatus::Ok,
                    Some(_) if page_has_searchable_text(merged, p.page_number) => {
                        PageParseStatus::Partial
                    }
                    Some(_) => PageParseStatus::Partial,
                    None => PageParseStatus::OcrFail,
                }
            } else if actual_page.is_some() {
                PageParseStatus::Ok
            } else {
                PageParseStatus::Missing
            };
            PageStatusEntry {
                page_no: p.page_number,
                status,
                route: page_route_label(p),
                duration_ms: None,
            }
        })
        .collect();
    merged.metadata.insert(
        "page_status".to_string(),
        serde_json::to_string(&page_status).unwrap_or_default(),
    );
}
