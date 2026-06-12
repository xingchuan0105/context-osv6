# Ingestion Routing v2 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the binary EdgeParse/VisualRaster PDF routing with a four-path model (Fast/Figure/Slow/Fallback), add PaddleOCR for scanned pages, and fix `image_heavy` misrouting.

**Architecture:** Per-page probe produces `readable_ratio` + quality signals → page-level `RouteDecision` (A/B/C/C′/Fallback) → grouped dispatch (EdgeParse for A+B, PaddleOCR for C+C′, VisualRaster for fallback) → merged `DocumentIr` with page-level status metadata.

**Tech Stack:** Rust, lopdf, reqwest (PaddleOCR HTTP), serde, tokio

**Spec:** `docs/ingestion-routing-discussion-2026-06-10.md`

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `crates/ingestion/src/parser/probe.rs` | Modify | Add `readable_ratio`, `bigram_repeat_ratio`, `unique_token_ratio`, `watermark_hit` to `PdfPageProbeResult` |
| `crates/ingestion/src/parser/router.rs` | Modify | Add `PaddleOcr` backend, `RouteDecision` enum, new `route_page()` logic |
| `crates/ingestion/src/ir.rs` | Modify | Add `PaddleOcrPdf` variant to `ParseBackend` |
| `crates/ingestion/src/parser/paddle_ocr.rs` | Create | `PaddleOcrClient` — submit job, poll, parse JSONL result |
| `crates/ingestion/src/parser/mod.rs` | Modify | Add `pub mod paddle_ocr` |
| `bins/worker/src/main.rs` | Modify | Refactor `execute_pdf_parse` for v2 dispatch |
| `.env.example` | Modify | Already has `PADDLE_OCR_*` — no changes needed |

---

## Task 1: Probe — Add Quality Signals (ING-1b-α)

**Covers:** §2.7 (readable_ratio), §1.5 (constants), §1.6 (route_page pseudocode)

**Files:**
- Modify: `crates/ingestion/src/parser/probe.rs`
- Test: same file (inline `#[cfg(test)]`)

### Step 1.1: Add constants

Add to `probe.rs` after the imports:

```rust
/// readable_ratio below this → route to OCR.
const TEXT_QUAL_THRESHOLD: f32 = 0.3;
/// Bigram repeat ratio above this → watermark/garbage page.
const BIGRAM_REPEAT_THRESHOLD: f32 = 0.30;
/// Unique token ratio below this → watermark/garbage page.
const UNIQUE_TOKEN_THRESHOLD: f32 = 0.4;
/// Pages with fewer chars than this AND low readable_ratio → OCR.
const PAGE_TEXT_THRESHOLD: usize = 100;

/// Watermark substrings (case-insensitive match via to_lowercase).
const WATERMARK_PATTERNS: &[&str] = &[
    "epub converter",
    "processtext.com",
    "watermark",
    "processed by",
];
```

No new dependencies needed — watermark detection uses plain string matching.

### Step 1.2: Extend `PdfPageProbeResult`

Replace the existing struct (L35-42):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfPageProbeResult {
    pub page_number: u32,
    pub extracted_text_chars: usize,
    pub image_hint_count: usize,
    pub table_hint_count: usize,
    pub likely_scanned: bool,
    // --- v2 quality signals (non-breaking: Option = None when not computed) ---
    pub readable_ratio: Option<f32>,
    pub bigram_repeat_ratio: Option<f32>,
    pub unique_token_ratio: Option<f32>,
    pub watermark_hit: bool,
}
```

### Step 1.3: Implement `compute_quality_signals` function

Add a standalone function (no `&self` needed — pure computation on extracted text):

```rust
fn compute_quality_signals(text: &str) -> (f32, f32, f32, bool) {
    if text.is_empty() {
        return (0.0, 0.0, 0.0, false);
    }

    // Tokenize: CJK single chars OR latin words ≥2 chars
    let tokens: Vec<&str> = text
        .split(|c: char| c.is_ascii_whitespace() || c.is_ascii_punctuation())
        .filter(|t| {
            (t.len() >= 2 && t.chars().all(|c| c.is_ascii_alphabetic()))
                || (t.chars().count() == 1 && !t.chars().next().unwrap().is_ascii())
        })
        .collect();

    let total_words = text.split_whitespace().count().max(1);
    let readable_ratio = tokens.len() as f32 / total_words as f32;

    // Bigram repeat ratio
    let bigrams: Vec<(char, char)> = text.chars().zip(text.chars().skip(1)).collect();
    let bigram_count = bigrams.len().max(1);
    let mut bigram_freq: std::collections::HashMap<(char, char), usize> =
        std::collections::HashMap::new();
    for bg in &bigrams {
        *bigram_freq.entry(*bg).or_insert(0) += 1;
    }
    let max_bigram_count = bigram_freq.values().max().copied().unwrap_or(0);
    let bigram_repeat_ratio = max_bigram_count as f32 / bigram_count as f32;

    // Unique token ratio
    let unique_tokens: std::collections::HashSet<&str> = tokens.iter().copied().collect();
    let unique_token_ratio = if tokens.is_empty() {
        0.0
    } else {
        unique_tokens.len() as f32 / tokens.len() as f32
    };

    // Watermark detection (case-insensitive substring match)
    let text_lower = text.to_lowercase();
    let watermark_hit = WATERMARK_PATTERNS.iter().any(|pat| text_lower.contains(pat));

    (readable_ratio, bigram_repeat_ratio, unique_token_ratio, watermark_hit)
}
```

### Step 1.4: Integrate into `probe_pdf`

In the per-page loop (around L140-170), after extracting `page_text_chars` as a string (not just `.len()`), call `compute_quality_signals` and populate the new fields.

Change the text extraction from:
```rust
let page_text_chars = doc
    .extract_text(&[page_number])
    .map(|c| c.len())
    .unwrap_or(0);
```

To:
```rust
let page_text = doc.extract_text(&[page_number]).unwrap_or_default();
let page_text_chars = page_text.len();
```

Then compute signals and build the probe result:
```rust
let (readable_ratio, bigram_repeat_ratio, unique_token_ratio, watermark_hit) =
    compute_quality_signals(&page_text);

page_probes.push(PdfPageProbeResult {
    page_number,
    extracted_text_chars: page_text_chars,
    image_hint_count: page_image_count,
    table_hint_count: page_table_count,
    likely_scanned: page_text_chars < config.scanned_page_threshold,
    readable_ratio: Some(readable_ratio),
    bigram_repeat_ratio: Some(bigram_repeat_ratio),
    unique_token_ratio: Some(unique_token_ratio),
    watermark_hit,
});
```

### Step 1.5: Write tests

```rust
#[test]
fn test_compute_quality_signals_empty() {
    let (rr, br, ut, wm) = compute_quality_signals("");
    assert_eq!(rr, 0.0);
    assert!(!wm);
}

#[test]
fn test_compute_quality_signals_normal_text() {
    let text = "The Black Swan is a book about uncertainty and rare events.";
    let (rr, _br, ut, wm) = compute_quality_signals(text);
    assert!(rr > 0.3, "normal text should have readable_ratio > 0.3: got {}", rr);
    assert!(ut > 0.4, "normal text should have unique_token_ratio > 0.4: got {}", ut);
    assert!(!wm);
}

#[test]
fn test_compute_quality_signals_watermark() {
    let text = "ePub Converter processtext.com some garbage here";
    let (_rr, _br, _ut, wm) = compute_quality_signals(text);
    assert!(wm, "should detect watermark pattern");
}

#[test]
fn test_probe_result_has_quality_signals() {
    let probe = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 500,
        image_hint_count: 0,
        table_hint_count: 0,
        likely_scanned: false,
        readable_ratio: Some(0.8),
        bigram_repeat_ratio: Some(0.1),
        unique_token_ratio: Some(0.7),
        watermark_hit: false,
    };
    assert_eq!(probe.readable_ratio, Some(0.8));
    assert!(!probe.watermark_hit);
}
```

### Step 1.6: Verify

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p ingestion --lib parser::probe -- --nocapture
```

### Step 1.7: Commit

```bash
git add crates/ingestion/src/parser/probe.rs crates/ingestion/Cargo.toml
git commit -m "feat(probe): add readable_ratio, watermark, unique_token quality signals (ING-1b-α)"
```

---

## Task 2: Router — New Routing Logic (ING-1)

**Covers:** §1.4 (page-level route table v2), §1.6 (route_page pseudocode), §2.5 (PdfPageBackend enum), D3 (image_heavy → Figure pipeline, not VisualRaster)

**Files:**
- Modify: `crates/ingestion/src/parser/router.rs`
- Modify: `crates/ingestion/src/ir.rs`
- Test: inline in `router.rs`

### Step 2.1: Add `RouteDecision` enum to `router.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteDecision {
    /// A: Has text, no figures → EdgeParse only
    FastText,
    /// B: Has text + figures → EdgeParse + figure pipeline
    FastWithFigures,
    /// C: No text or garbage → PaddleOCR (slow path)
    SlowOcr,
    /// C′: Has text but table garbled → single-page PaddleOCR upgrade
    SlowOcrSinglePage,
    /// Fallback: OCR failed → page raster multimodal
    Fallback,
}
```

### Step 2.2: Extend `PdfPageBackend` enum

Replace (L100-103):
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfPageBackend {
    EdgeParse,
    PaddleOcr,
    VisualRaster,
}
```

### Step 2.3: Add new `RouteReason` variants

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteReason {
    TextFile,
    ImageFile,
    OfficeDocument,
    PresentationFile,
    SimplePdf,
    ComplexPdf,
    ScannedPdf,
    // v2 additions
    FastText,
    FastWithFigures,
    SlowOcr,
    SlowOcrSinglePage,
    OcrFallback,
}
```

Update the `Display` impl accordingly.

### Step 2.4: Add routing constants

```rust
const FIG_COUNT_THRESHOLD: usize = 2;
const TABLE_GARBLE_THRESHOLD: f32 = 0.30;
const TABLE_QUAL_THRESHOLD: f32 = 0.6;
```

### Step 2.5: Add `route_page` function

```rust
fn route_page(page: &PdfPageProbeResult, config: &ParseProbeConfig) -> (PdfPageBackend, RouteDecision, RouteReason) {
    // C: no text or garbage quality
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
        return (
            PdfPageBackend::PaddleOcr,
            RouteDecision::SlowOcr,
            RouteReason::SlowOcr,
        );
    }

    // C′: table garbled (one-day — edgeparse_table_quality always 1.0, so only garbled_ratio matters)
    // Note: table_garbled_ratio not yet in probe; placeholder for future
    if page.table_hint_count > 0 && page.extracted_text_chars < PAGE_TEXT_THRESHOLD {
        return (
            PdfPageBackend::PaddleOcr,
            RouteDecision::SlowOcrSinglePage,
            RouteReason::SlowOcrSinglePage,
        );
    }

    // B: has images AND has text → EdgeParse + figure pipeline
    // Phase 1 (ING-1b-α): use image_hint_count as proxy (≥2 = likely has figures)
    if page.image_hint_count >= FIG_COUNT_THRESHOLD {
        return (
            PdfPageBackend::EdgeParse,
            RouteDecision::FastWithFigures,
            RouteReason::FastWithFigures,
        );
    }

    // A: clean text page
    (
        PdfPageBackend::EdgeParse,
        RouteDecision::FastText,
        RouteReason::FastText,
    )
}
```

### Step 2.6: Rewrite `build_pdf_page_plan` to use `route_page`

```rust
fn build_pdf_page_plan(page_probe: &PdfPageProbeResult, config: &ParseProbeConfig) -> PdfPagePlan {
    let (backend, _decision, reason) = route_page(page_probe, config);

    PdfPagePlan {
        page_number: page_probe.page_number,
        backend,
        reason,
    }
}
```

### Step 2.7: Add `PaddleOcrPdf` to `ParseBackend` in `ir.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ParseBackend {
    EdgeParsePdf,
    VisualRasterPdf,
    PaddleOcrPdf,      // ← new
    MineruPdfOcr,
    // ... rest unchanged
}
```

Update `as_str()`:
```rust
Self::PaddleOcrPdf => "paddle_ocr_pdf",
```

### Step 2.8: Update `summarize_pdf_reason` for new backends

The existing `summarize_pdf_reason` checks for `VisualRaster`. Update it to also handle `PaddleOcr`:

```rust
fn summarize_pdf_reason(probe_result: &ParseProbeResult, plan: &ParsePlan) -> RouteReason {
    let ParsePlan::Pdf(pdf_plan) = plan else {
        return RouteReason::SimplePdf;
    };

    let has_ocr = pdf_plan.pages.iter().any(|p| p.backend == PdfPageBackend::PaddleOcr);
    let has_visual = pdf_plan.pages.iter().any(|p| p.backend == PdfPageBackend::VisualRaster);

    if has_ocr {
        RouteReason::ScannedPdf
    } else if has_visual || probe_result.likely_scanned {
        RouteReason::ComplexPdf
    } else if pdf_plan.pages.iter().any(|p| p.reason == RouteReason::FastWithFigures) {
        RouteReason::ComplexPdf
    } else {
        RouteReason::SimplePdf
    }
}
```

### Step 2.9: Update existing tests + add new ones

Update the test `pdf_page_plan_routes_each_page_independently` — the scanned page (10 chars, `likely_scanned: true`) should now route to `PaddleOcr` instead of `VisualRaster`.

Add a new test for the routing decision:

```rust
#[test]
fn test_route_page_scanned_goes_to_paddle() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 0,
        image_hint_count: 0,
        table_hint_count: 0,
        likely_scanned: true,
        readable_ratio: Some(0.0),
        bigram_repeat_ratio: Some(0.0),
        unique_token_ratio: Some(0.0),
        watermark_hit: false,
    };
    let (_backend, decision, _reason) = route_page(&page, &ParseProbeConfig::default());
    assert_eq!(decision, RouteDecision::SlowOcr);
}

#[test]
fn test_route_page_image_heavy_with_text_goes_to_b() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 500,
        image_hint_count: 3,
        table_hint_count: 0,
        likely_scanned: false,
        readable_ratio: Some(0.7),
        bigram_repeat_ratio: Some(0.1),
        unique_token_ratio: Some(0.6),
        watermark_hit: false,
    };
    let (backend, decision, _reason) = route_page(&page, &ParseProbeConfig::default());
    assert_eq!(backend, PdfPageBackend::EdgeParse);
    assert_eq!(decision, RouteDecision::FastWithFigures);
}

#[test]
fn test_route_page_watermark_goes_to_ocr() {
    let page = PdfPageProbeResult {
        page_number: 1,
        extracted_text_chars: 500,
        image_hint_count: 0,
        table_hint_count: 0,
        likely_scanned: false,
        readable_ratio: Some(0.9),
        bigram_repeat_ratio: Some(0.1),
        unique_token_ratio: Some(0.8),
        watermark_hit: true,
    };
    let (_backend, decision, _reason) = route_page(&page, &ParseProbeConfig::default());
    assert_eq!(decision, RouteDecision::SlowOcr);
}
```

### Step 2.10: Verify

```bash
cargo test -p ingestion --lib parser::router -- --nocapture
```

### Step 2.11: Commit

```bash
git add crates/ingestion/src/parser/router.rs crates/ingestion/src/ir.rs
git commit -m "feat(router): add page-level route_page() with PaddleOcr backend (ING-1)"
```

---

## Task 3: PaddleOCR Client (ING-2)

**Covers:** §2.1-2.3 (Paddle HTTP client), §2.5 (dispatch skeleton)

**Files:**
- Create: `crates/ingestion/src/parser/paddle_ocr.rs`
- Modify: `crates/ingestion/src/parser/mod.rs`
- Modify: `crates/ingestion/Cargo.toml` (add `reqwest` if not present)

### Step 3.1: Create `paddle_ocr.rs`

```rust
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct PaddleOcrConfig {
    pub base_url: String,
    pub api_token: String,
    pub model: String,
    pub poll_interval_secs: u64,
    pub job_timeout_secs: u64,
}

impl PaddleOcrConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            base_url: std::env::var("PADDLE_OCR_BASE_URL")
                .unwrap_or_else(|_| "https://paddleocr.aistudio-app.com/api/v2/ocr".to_string()),
            api_token: std::env::var("PADDLE_OCR_API_TOKEN")
                .context("PADDLE_OCR_API_TOKEN not set")?,
            model: std::env::var("PADDLE_OCR_MODEL")
                .unwrap_or_else(|_| "PaddleOCR-VL-1.6".to_string()),
            poll_interval_secs: std::env::var("PADDLE_OCR_POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            job_timeout_secs: std::env::var("PADDLE_OCR_JOB_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
        })
    }
}

#[derive(Debug, Serialize)]
struct SubmitJobRequest {
    url: Option<String>,
    file: Option<String>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_async: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SubmitJobResponse {
    job_id: String,
}

#[derive(Debug, Deserialize)]
struct JobStatusResponse {
    state: String,
    #[serde(default)]
    result_url: Option<ResultUrl>,
}

#[derive(Debug, Deserialize)]
struct ResultUrl {
    #[serde(rename = "jsonUrl")]
    json_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OcrPageResult {
    #[serde(rename = "layoutParsingResults", default)]
    layout_parsing_results: Vec<LayoutParsingResult>,
}

#[derive(Debug, Deserialize)]
struct LayoutParsingResult {
    #[serde(default)]
    markdown: Option<MarkdownContent>,
}

#[derive(Debug, Deserialize)]
struct MarkdownContent {
    #[serde(default)]
    text: Option<String>,
}

/// Per-page OCR output from PaddleOCR.
#[derive(Debug, Clone)]
pub struct PaddleOcrPageResult {
    pub page_number: u32,
    pub text: String,
}

pub struct PaddleOcrClient {
    config: PaddleOcrConfig,
    http: Client,
}

impl PaddleOcrClient {
    pub fn new(config: PaddleOcrConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .trust_env(false) // WSL proxy bypass
            .build()
            .expect("failed to build HTTP client");
        Self { config, http }
    }

    /// Submit a PDF (bytes) to PaddleOCR and poll until done.
    /// Returns per-page OCR text results.
    pub async fn ocr_pdf_bytes(&self, pdf_bytes: &[u8], start_page: u32) -> Result<Vec<PaddleOcrPageResult>> {
        let job_id = self.submit_job(pdf_bytes).await?;
        info!(job_id = %job_id, start_page, "PaddleOCR job submitted");

        let result_url = self.poll_job(&job_id).await?;
        let json_url = result_url
            .json_url
            .context("PaddleOCR job done but no jsonUrl")?;

        let pages = self.fetch_and_parse_result(&json_url, start_page).await?;
        Ok(pages)
    }

    async fn submit_job(&self, pdf_bytes: &[u8]) -> Result<String> {
        let url = format!("{}/jobs", self.config.base_url);

        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
                    .file_name("document.pdf")
                    .mime_str("application/pdf")?,
            )
            .text("model", self.config.model.clone());

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .multipart(form)
            .send()
            .await
            .context("PaddleOCR submit request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("PaddleOCR submit failed ({status}): {body}");
        }

        let job: SubmitJobResponse = resp.json().await.context("invalid submit response")?;
        Ok(job.job_id)
    }

    async fn poll_job(&self, job_id: &str) -> Result<ResultUrl> {
        let url = format!("{}/jobs/{}", self.config.base_url, job_id);
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(self.config.job_timeout_secs);

        loop {
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("PaddleOCR job {job_id} timed out after {}s", self.config.job_timeout_secs);
            }

            let resp = self
                .http
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_token))
                .send()
                .await
                .context("PaddleOCR poll request failed")?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(job_id, %status, body, "PaddleOCR poll non-200");
                sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                continue;
            }

            let status: JobStatusResponse = resp.json().await.context("invalid poll response")?;
            debug!(job_id, state = %status.state, "PaddleOCR poll");

            match status.state.as_str() {
                "done" | "success" | "completed" => {
                    return status
                        .result_url
                        .context("job done but no result_url");
                }
                "failed" | "error" => {
                    anyhow::bail!("PaddleOCR job {job_id} failed");
                }
                _ => {
                    sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                }
            }
        }
    }

    async fn fetch_and_parse_result(&self, json_url: &str, start_page: u32) -> Result<Vec<PaddleOcrPageResult>> {
        let resp = self
            .http
            .get(json_url)
            .send()
            .await
            .context("fetch OCR result JSON failed")?;

        let pages_raw: Vec<OcrPageResult> = resp.json().await.context("parse OCR result JSON failed")?;

        let pages = pages_raw
            .into_iter()
            .enumerate()
            .map(|(i, page)| {
                let text = page
                    .layout_parsing_results
                    .iter()
                    .filter_map(|l| l.markdown.as_ref()?.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                PaddleOcrPageResult {
                    page_number: start_page + i as u32,
                    text,
                }
            })
            .collect();

        Ok(pages)
    }
}
```

### Step 3.2: Register module in `mod.rs`

Add to `crates/ingestion/src/parser/mod.rs`:
```rust
pub mod paddle_ocr;
```

### Step 3.3: Verify it compiles

```bash
cargo check -p ingestion
```

### Step 3.4: Commit

```bash
git add crates/ingestion/src/parser/paddle_ocr.rs crates/ingestion/src/parser/mod.rs
git commit -m "feat(ocr): add PaddleOcrClient for scene C scanned PDF OCR (ING-2)"
```

---

## Task 4: Worker — v2 Dispatch Skeleton (ING-2b)

**Covers:** §2.5 (execute_pdf_parse_v2), §2.6 (multi-batch Paddle)

**Files:**
- Modify: `bins/worker/src/main.rs` (execute_pdf_parse function, L2378-2545)

### Step 4.1: Add PaddleOCR imports

At the top of `main.rs`, add:
```rust
use ingestion::parser::paddle_ocr::{PaddleOcrClient, PaddleOcrConfig};
use ingestion::parser::router::RouteDecision;
```

### Step 4.2: Refactor `execute_pdf_parse`

Replace the function body (L2378-2545) with the v2 dispatch logic:

```rust
async fn execute_pdf_parse(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    _object_path: &str,
    document_id: Uuid,
    plan: &ingestion::parser::PdfParsePlan,
) -> Result<DocumentIr, IngestionError> {
    // Group pages by backend
    let edge_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.backend == PdfPageBackend::EdgeParse)
        .map(|p| p.page_number)
        .collect();
    let paddle_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.backend == PdfPageBackend::PaddleOcr)
        .map(|p| p.page_number)
        .collect();
    let visual_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.backend == PdfPageBackend::VisualRaster)
        .map(|p| p.page_number)
        .collect();

    // 1. EdgeParse (A + B pages)
    let digital_ir = if edge_pages.is_empty() {
        None
    } else {
        let parsed = PdfParser
            .parse_pages(bytes, filename, &edge_pages)
            .await
            .map_err(|error| {
                IngestionError::StateSink(format!("pdf digital parse failed for {filename}: {error}"))
            })?;
        Some(
            document_ir_from_parsed_document(
                document_id,
                filename,
                DocumentType::Pdf,
                ParseBackend::EdgeParsePdf,
                parsed,
            )
            .with_pdf_defaults(ParseBackend::EdgeParsePdf),
        )
    };

    // 2. PaddleOCR (C + C′ pages)
    let paddle_ir = if paddle_pages.is_empty() {
        None
    } else {
        let config = PaddleOcrConfig::from_env().map_err(|e| {
            IngestionError::StateSink(format!("PaddleOCR config error: {e}"))
        })?;
        let client = PaddleOcrClient::new(config);
        let ocr_pages = build_paddle_ocr_segments(&client, bytes, &paddle_pages).await?;
        Some(build_document_ir_from_paddle_results(document_id, filename, ocr_pages))
    };

    // 3. VisualRaster (fallback only)
    let visual_ir = if visual_pages.is_empty() {
        None
    } else {
        let renderer = processor.pdf_renderer_client.as_ref().ok_or_else(|| {
            IngestionError::StateSink(format!(
                "PDF visual raster selected for {filename}, but PDF_RENDERER_BASE_URL is not configured"
            ))
        })?;
        let parser = VisualPdfParser::new(renderer.clone());
        Some(
            parser
                .parse_pages(bytes, filename, document_id, &visual_pages)
                .await
                .map_err(|error| {
                    IngestionError::StateSink(format!("visual pdf parse failed for {filename}: {error}"))
                })?,
        )
    };

    // 4. Merge
    merge_pdf_ir(
        document_id,
        filename,
        plan,
        digital_ir,
        paddle_ir,
        visual_ir,
    )
}
```

### Step 4.3: Add helper functions

```rust
/// Build PaddleOCR segments from contiguous page runs, then dispatch.
async fn build_paddle_ocr_segments(
    client: &PaddleOcrClient,
    bytes: &[u8],
    paddle_pages: &[u32],
) -> Result<Vec<PaddleOcrPageResult>, IngestionError> {
    if paddle_pages.is_empty() {
        return Ok(Vec::new());
    }

    // Group contiguous pages into segments
    let segments = group_contiguous_pages(paddle_pages);
    let batch_pages: usize = std::env::var("PADDLE_OCR_BATCH_PAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(80);

    let mut all_results = Vec::new();

    for seg in &segments {
        // Split segment if larger than batch_pages
        for chunk_start in (seg.0..=seg.1).step_by(batch_pages) {
            let chunk_end = (chunk_start + batch_pages as u32 - 1).min(seg.1);
            let page_count = (chunk_end - chunk_start + 1) as usize;

            // Extract PDF slice using lopdf
            let pdf_slice = extract_pdf_slice(bytes, chunk_start, chunk_end)
                .map_err(|e| IngestionError::StateSink(format!("PDF slice extraction failed: {e}")))?;

            let results = client
                .ocr_pdf_bytes(&pdf_slice, chunk_start)
                .await
                .map_err(|e| IngestionError::StateSink(format!("PaddleOCR failed: {e}")))?;

            all_results.extend(results);
        }
    }

    Ok(all_results)
}

/// Group sorted page numbers into contiguous (start, end) ranges.
fn group_contiguous_pages(pages: &[u32]) -> Vec<(u32, u32)> {
    if pages.is_empty() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    let mut start = pages[0];
    let mut end = pages[0];

    for &page in &pages[1..] {
        if page == end + 1 {
            end = page;
        } else {
            segments.push((start, end));
            start = page;
            end = page;
        }
    }
    segments.push((start, end));
    segments
}

fn build_document_ir_from_paddle_results(
    document_id: Uuid,
    filename: &str,
    pages: Vec<PaddleOcrPageResult>,
) -> DocumentIr {
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    ir.metadata.insert("ocr_backend".to_string(), "paddle".to_string());

    for page in &pages {
        ir.pages.push(PageIr {
            page_number: page.page_number,
            width: None,
            height: None,
            backend: ParseBackend::PaddleOcrPdf,
            text_char_count: page.text.len(),
            image_count: 0,
            metadata: BTreeMap::new(),
        });

        if !page.text.is_empty() {
            ir.blocks.push(BlockIr {
                block_id: format!("paddle-p{}-text", page.page_number),
                page: Some(page.page_number),
                block_type: BlockType::Paragraph,
                modality: BlockModality::TextOnly,
                text: page.text.clone(),
                alt_text: None,
                asset_refs: Vec::new(),
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(page.page_number),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::PaddleOcrPdf,
                metadata: BTreeMap::new(),
            });
        }
    }

    ir
}

fn merge_pdf_ir(
    document_id: Uuid,
    filename: &str,
    plan: &ingestion::parser::PdfParsePlan,
    digital_ir: Option<DocumentIr>,
    paddle_ir: Option<DocumentIr>,
    visual_ir: Option<DocumentIr>,
) -> Result<DocumentIr, IngestionError> {
    let title = digital_ir.as_ref().map(|d| d.title.clone()).filter(|t| !t.trim().is_empty())
        .or_else(|| paddle_ir.as_ref().map(|d| d.title.clone()).filter(|t| !t.trim().is_empty()))
        .or_else(|| visual_ir.as_ref().map(|d| d.title.clone()).filter(|t| !t.trim().is_empty()))
        .unwrap_or_else(|| filename.to_string());

    let has_edge = digital_ir.is_some();
    let has_paddle = paddle_ir.is_some();
    let has_visual = visual_ir.is_some();

    let primary_backend = if has_edge {
        ParseBackend::EdgeParsePdf
    } else if has_paddle {
        ParseBackend::PaddleOcrPdf
    } else {
        ParseBackend::VisualRasterPdf
    };

    let mut merged = DocumentIr::new(
        document_id.to_string(),
        title,
        DocumentType::Pdf,
        primary_backend,
    );

    // Route mode metadata
    let mode_count = [has_edge, has_paddle, has_visual].iter().filter(|&&x| x).count();
    if mode_count > 1 {
        let mode = match (has_edge, has_paddle, has_visual) {
            (true, true, true) => "hybrid_v2",
            (true, true, false) => "hybrid_v2",
            (true, false, true) => "hybrid",
            (false, true, true) => "hybrid_v2",
            _ => "hybrid_v2",
        };
        merged.metadata.insert("pdf_route_mode".to_string(), mode.to_string());
    }

    // Collect metadata from any source
    for ir in [&digital_ir, &paddle_ir, &visual_ir].iter().filter_map(|x| x.as_ref()) {
        if merged.metadata.len() <= 1 {
            merged.metadata.extend(ir.metadata.clone());
        }
        merged.warnings.extend(ir.warnings.clone());
    }

    // Page-level assembly: for each page in plan, pick the right IR source
    for page_plan in &plan.pages {
        let source_ir = match page_plan.backend {
            PdfPageBackend::EdgeParse => digital_ir.as_ref(),
            PdfPageBackend::PaddleOcr => paddle_ir.as_ref(),
            PdfPageBackend::VisualRaster => visual_ir.as_ref(),
        };
        let page_backend = match page_plan.backend {
            PdfPageBackend::EdgeParse => ParseBackend::EdgeParsePdf,
            PdfPageBackend::PaddleOcr => ParseBackend::PaddleOcrPdf,
            PdfPageBackend::VisualRaster => ParseBackend::VisualRasterPdf,
        };

        let Some(source_ir) = source_ir else {
            continue;
        };

        let page_data = filter_document_ir_to_page(source_ir, page_plan.page_number);
        let mut page_row = page_data.pages.into_iter().next().unwrap_or(PageIr {
            page_number: page_plan.page_number,
            width: None,
            height: None,
            backend: page_backend.clone(),
            text_char_count: 0,
            image_count: 0,
            metadata: Default::default(),
        });
        page_row.page_number = page_plan.page_number;
        page_row.backend = page_backend.clone();
        merged.pages.push(page_row);

        merged.blocks.extend(page_data.blocks.into_iter().map(|mut block| {
            block.page = Some(page_plan.page_number);
            block.source_locator.page = Some(page_plan.page_number);
            block.parser_backend = page_backend.clone();
            block
        }));
        merged.assets.extend(page_data.assets.into_iter().map(|mut asset| {
            asset.page = Some(page_plan.page_number);
            asset.parser_backend = page_backend.clone();
            asset
        }));
    }

    Ok(merged)
}
```

### Step 4.4: Add `extract_pdf_slice` function

Uses `lopdf` to extract a page range from a PDF into a new in-memory PDF:

```rust
fn extract_pdf_slice(bytes: &[u8], start_page: u32, end_page: u32) -> Result<Vec<u8>> {
    let doc = lopdf::Document::load_mem(bytes)?;
    let all_pages = doc.get_pages();
    let page_ids: Vec<lopdf::ObjectId> = all_pages
        .iter()
        .filter(|(num, _)| **num >= start_page && **num <= end_page)
        .map(|(_, id)| *id)
        .collect();

    if page_ids.is_empty() {
        anyhow::bail!("no pages in range {start_page}-{end_page}");
    }

    // Use lopdf's page extraction
    let mut new_doc = lopdf::Document::with_version("1.5");
    new_doc.add_objects(doc.get_object_ids().to_vec());
    new_doc.delete_pages(&page_ids);
    // Actually we need the inverse — keep only the selected pages
    // lopdf doesn't have a direct "keep pages" method, so we use a different approach

    // Simpler approach: use the page tree
    let pages_to_remove: Vec<u32> = all_pages
        .iter()
        .filter(|(num, _)| *num < start_page || *num > end_page)
        .map(|(num, _)| *num)
        .collect();

    let mut slice_doc = lopdf::Document::load_mem(bytes)?;
    slice_doc.delete_pages(&pages_to_remove);
    slice_doc.renumber_objects();

    let mut buf = Vec::new();
    slice_doc.save_to(&mut buf)?;
    Ok(buf)
}
```

### Step 4.5: Add tests for `group_contiguous_pages`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_contiguous_pages() {
        assert_eq!(group_contiguous_pages(&[]), vec![]);
        assert_eq!(group_contiguous_pages(&[1]), vec![(1, 1)]);
        assert_eq!(group_contiguous_pages(&[1, 2, 3, 4, 5]), vec![(1, 5)]);
        assert_eq!(
            group_contiguous_pages(&[1, 2, 3, 10, 11, 20]),
            vec![(1, 3), (10, 11), (20, 20)]
        );
    }
}
```

### Step 4.6: Verify

```bash
cargo check -p worker
cargo test -p worker -- group_contiguous --nocapture
```

### Step 4.7: Commit

```bash
git add bins/worker/src/main.rs
git commit -m "feat(worker): v2 PDF dispatch with PaddleOCR scene C support (ING-2b)"
```

---

## Task 5: Closes Default Page Raster for OCR Pages (ING-4)

**Covers:** §2.3 P2 (INGESTION_PAGE_RASTER_WITH_OCR)

**Files:**
- Modify: `bins/worker/src/main.rs`

### Step 5.1: Add env check

In the `maybe_enrich_visual_multimodal_summaries` function (or wherever page_raster is created for OCR pages), add a gate:

```rust
let page_raster_with_ocr = std::env::var("INGESTION_PAGE_RASTER_WITH_OCR")
    .ok()
    .and_then(|v| v.parse::<bool>().ok())
    .unwrap_or(false);
```

When the primary backend is `PaddleOcrPdf` and `page_raster_with_ocr` is false, skip page raster creation for those pages.

### Step 5.2: Commit

```bash
git add bins/worker/src/main.rs
git commit -m "feat(worker): disable page_raster for OCR-successful pages by default (ING-4)"
```

---

## Task 6: Verify Full Build + Integration

**Covers:** All tasks

### Step 6.1: Full check

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo check --workspace
```

### Step 6.2: Full test

```bash
cargo test -p ingestion --lib -- --nocapture
cargo test -p worker -- --nocapture
```

### Step 6.3: Clippy

```bash
cargo clippy -p ingestion -p worker -- -D warnings
```
