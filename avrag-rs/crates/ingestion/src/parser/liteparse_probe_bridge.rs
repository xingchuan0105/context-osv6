//! Hybrid PDF probe: LiteParse text signals + lopdf structure hints.

use anyhow::{Context, Result};

use super::liteparse::{LiteParseService, ParsedPdfSnapshot};
use super::liteparse_ir::LiteParsePageProbe;
use super::probe::{ParseProbe, ParseProbeConfig, ParseProbeResult, PdfPageProbeResult};

/// Hybrid probe output: lopdf routing signals plus a reusable LiteParse parse snapshot.
#[derive(Debug, Clone)]
pub struct HybridPdfProbeOutcome {
    pub probe_result: ParseProbeResult,
    pub liteparse_snapshot: ParsedPdfSnapshot,
}

/// Probe PDF using lopdf structure hints overlaid with LiteParse text extraction.
pub fn probe_pdf_hybrid(
    bytes: &[u8],
    filename: &str,
    config: &ParseProbeConfig,
) -> Result<HybridPdfProbeOutcome> {
    let mut probe_result = ParseProbe::probe_with_config(bytes, filename, config)?;
    let liteparse_snapshot = run_liteparse_snapshot_blocking(bytes)?;
    overlay_liteparse_signals(&mut probe_result, liteparse_snapshot.probes(), config);
    Ok(HybridPdfProbeOutcome {
        probe_result,
        liteparse_snapshot,
    })
}

fn run_liteparse_snapshot_blocking(bytes: &[u8]) -> Result<ParsedPdfSnapshot> {
    let pdf_bytes = bytes.to_vec();
    std::thread::spawn(move || -> Result<ParsedPdfSnapshot> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime for liteparse probe")?;
        rt.block_on(async { LiteParseService::from_env().parse_pdf_document(&pdf_bytes).await })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("liteparse probe thread panicked"))?
}

pub fn overlay_liteparse_signals(
    result: &mut ParseProbeResult,
    lp_pages: &[LiteParsePageProbe],
    config: &ParseProbeConfig,
) {
    if lp_pages.is_empty() {
        return;
    }

    let lp_by_page: std::collections::HashMap<u32, &LiteParsePageProbe> = lp_pages
        .iter()
        .map(|p| (p.page_number, p))
        .collect();

    let mut total_text_chars = 0usize;
    let mut scanned_pages = 0usize;

    if result.pdf_page_probes.is_empty() {
        result.pdf_page_probes = lp_pages
            .iter()
            .map(|lp| PdfPageProbeResult {
                page_number: lp.page_number,
                extracted_text_chars: lp.extracted_text_chars,
                image_hint_count: lp.image_hint_count,
                table_hint_count: lp.table_hint_count,
                likely_scanned: lp.likely_scanned,
                readable_ratio: lp.readable_ratio,
                bigram_repeat_ratio: lp.bigram_repeat_ratio,
                unique_token_ratio: lp.unique_token_ratio,
                watermark_hit: lp.watermark_hit,
                figure_area_ratio: lp.figure_area_ratio,
                non_decorative_image_count: lp.non_decorative_image_count,
                table_garbled_ratio: lp.table_garbled_ratio,
            })
            .collect();
    } else {
        for page_probe in &mut result.pdf_page_probes {
            if let Some(lp) = lp_by_page.get(&page_probe.page_number) {
                page_probe.extracted_text_chars = lp.extracted_text_chars;
                page_probe.likely_scanned =
                    lp.extracted_text_chars < config.scanned_page_threshold;
            }
        }
    }

    for page_probe in &result.pdf_page_probes {
        total_text_chars += page_probe.extracted_text_chars;
        if page_probe.likely_scanned {
            scanned_pages += 1;
        }
    }

    result.extracted_text_chars = total_text_chars;
    result.likely_scanned = scanned_pages > result.pdf_page_probes.len() / 2;
    if result.page_count.is_none() {
        result.page_count = Some(lp_pages.len() as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn phase0_mini_pdf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/spike/fixtures/phase0-mini.pdf")
    }

    #[test]
    fn hybrid_probe_overlays_liteparse_text_chars() {
        let path = phase0_mini_pdf();
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(path).expect("read fixture");
        let config = ParseProbeConfig::from_env();

        let lopdf_only = ParseProbe::probe_with_config(&bytes, "phase0-mini.pdf", &config)
            .expect("lopdf probe");

        let hybrid = probe_pdf_hybrid(&bytes, "phase0-mini.pdf", &config).expect("hybrid probe");

        assert!(
            !hybrid.probe_result.pdf_page_probes.is_empty(),
            "hybrid probe should return page probes"
        );
        assert_eq!(
            hybrid.probe_result.pdf_page_probes.len(),
            lopdf_only.pdf_page_probes.len(),
            "hybrid should preserve lopdf page count"
        );
        assert!(
            hybrid.probe_result.extracted_text_chars >= lopdf_only.extracted_text_chars
                || hybrid
                    .probe_result
                    .pdf_page_probes
                    .iter()
                    .any(|p| p.extracted_text_chars > 0),
            "liteparse overlay should contribute text char signals"
        );
        assert!(!hybrid.liteparse_snapshot.text_blocks().is_empty());
    }
}
