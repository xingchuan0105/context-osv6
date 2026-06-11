use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::probe::{
    ParseProbe, ParseProbeConfig, ParseProbeResult, PdfPageProbeResult, BIGRAM_REPEAT_THRESHOLD,
    PAGE_TEXT_THRESHOLD, TEXT_QUAL_THRESHOLD, UNIQUE_TOKEN_THRESHOLD,
};

const FIG_COUNT_THRESHOLD: usize = 2;
const TABLE_GARBLE_THRESHOLD: f32 = 0.30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseRoute {
    Local,
    OfficeService,
    Pdf,
    MineruImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteDecision {
    FastText,
    FastWithFigures,
    SlowOcr,
    SlowOcrSinglePage,
    Fallback,
}

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
    FastText,
    FastWithFigures,
    SlowOcr,
    SlowOcrSinglePage,
    OcrFallback,
}

impl std::fmt::Display for RouteReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteReason::TextFile => write!(f, "text_file"),
            RouteReason::ImageFile => write!(f, "image_file"),
            RouteReason::OfficeDocument => write!(f, "office_document"),
            RouteReason::PresentationFile => write!(f, "presentation_file"),
            RouteReason::SimplePdf => write!(f, "simple_pdf"),
            RouteReason::ComplexPdf => write!(f, "complex_pdf"),
            RouteReason::ScannedPdf => write!(f, "scanned_pdf"),
            RouteReason::FastText => write!(f, "fast_text"),
            RouteReason::FastWithFigures => write!(f, "fast_with_figures"),
            RouteReason::SlowOcr => write!(f, "slow_ocr"),
            RouteReason::SlowOcrSinglePage => write!(f, "slow_ocr_single_page"),
            RouteReason::OcrFallback => write!(f, "ocr_fallback"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct ParseRouteError {
    code: &'static str,
    message: String,
}

impl ParseRouteError {
    pub fn code(&self) -> &'static str {
        self.code
    }

    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            code: "unsupported_file_type",
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseRouteDecision {
    pub route: ParseRoute,
    pub reason: RouteReason,
    pub probe_result: Option<ParseProbeResult>,
    pub plan: ParsePlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalParseKind {
    Text,
    Html,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalParsePlan {
    pub kind: LocalParseKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfficeDocType {
    Doc,
    Docx,
    Xls,
    Xlsx,
    Ppt,
    Pptx,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficeParsePlan {
    pub doc_type: OfficeDocType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfPageBackend {
    EdgeParse,
    PaddleOcr,
    VisualRaster,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPagePlan {
    pub page_number: u32,
    pub backend: PdfPageBackend,
    pub reason: RouteReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfParsePlan {
    pub pages: Vec<PdfPagePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalParseKind {
    MineruImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalParsePlan {
    pub kind: ExternalParseKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParsePlan {
    Pdf(PdfParsePlan),
    Office(OfficeParsePlan),
    Local(LocalParsePlan),
    External(ExternalParsePlan),
}

pub struct ParseRouter;

impl ParseRouter {
    pub fn ensure_supported_file_type(
        filename: &str,
        mime_type: &str,
    ) -> Result<(), ParseRouteError> {
        let extension = normalize_extension(filename).ok_or_else(|| {
            ParseRouteError::unsupported(format!(
                "file {filename} is missing a supported extension"
            ))
        })?;
        let normalized_mime = normalize_mime_type(mime_type);

        if normalized_mime.is_empty() {
            return Err(ParseRouteError::unsupported(format!(
                "file {filename} is missing a supported MIME type"
            )));
        }

        if !is_supported_extension(&extension) {
            return Err(ParseRouteError::unsupported(format!(
                "file {filename} uses unsupported extension .{extension}"
            )));
        }

        if !mime_matches_extension(&extension, &normalized_mime) {
            return Err(ParseRouteError::unsupported(format!(
                "file {filename} with MIME type {normalized_mime} is not supported"
            )));
        }

        Ok(())
    }

    pub fn route(
        bytes: &[u8],
        filename: &str,
        mime_type: &str,
    ) -> Result<ParseRouteDecision, ParseRouteError> {
        Self::route_with_config(bytes, filename, mime_type, &ParseProbeConfig::default())
    }

    pub fn route_with_config(
        bytes: &[u8],
        filename: &str,
        mime_type: &str,
        config: &ParseProbeConfig,
    ) -> Result<ParseRouteDecision, ParseRouteError> {
        Self::ensure_supported_file_type(filename, mime_type)?;
        let extension =
            normalize_extension(filename).expect("validated file types must retain an extension");

        match extension.as_str() {
            "txt" | "md" | "rst" | "csv" | "json" | "toml" | "yaml" | "yml" => {
                Ok(ParseRouteDecision {
                    route: ParseRoute::Local,
                    reason: RouteReason::TextFile,
                    probe_result: None,
                    plan: ParsePlan::Local(LocalParsePlan {
                        kind: LocalParseKind::Text,
                    }),
                })
            }
            "html" | "htm" => Ok(ParseRouteDecision {
                route: ParseRoute::Local,
                reason: RouteReason::TextFile,
                probe_result: None,
                plan: ParsePlan::Local(LocalParsePlan {
                    kind: LocalParseKind::Html,
                }),
            }),
            _ if is_code_extension(&extension) => Ok(ParseRouteDecision {
                route: ParseRoute::Local,
                reason: RouteReason::TextFile,
                probe_result: None,
                plan: ParsePlan::Local(LocalParsePlan {
                    kind: LocalParseKind::Code,
                }),
            }),
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Ok(ParseRouteDecision {
                route: ParseRoute::MineruImage,
                reason: RouteReason::ImageFile,
                probe_result: None,
                plan: ParsePlan::External(ExternalParsePlan {
                    kind: ExternalParseKind::MineruImage,
                }),
            }),
            "ppt" | "pptx" => Ok(ParseRouteDecision {
                route: ParseRoute::OfficeService,
                reason: RouteReason::PresentationFile,
                probe_result: None,
                plan: ParsePlan::Office(OfficeParsePlan {
                    doc_type: if extension == "ppt" {
                        OfficeDocType::Ppt
                    } else {
                        OfficeDocType::Pptx
                    },
                }),
            }),
            "pdf" => {
                let probe_result = ParseProbe::probe_with_config(bytes, filename, config)
                    .map_err(|error| ParseRouteError::unsupported(error.to_string()))?;
                let plan = ParsePlan::Pdf(build_pdf_parse_plan(&probe_result, config));
                Ok(ParseRouteDecision {
                    route: ParseRoute::Pdf,
                    reason: summarize_pdf_reason(&probe_result, &plan),
                    probe_result: Some(probe_result),
                    plan,
                })
            }
            "doc" | "docx" => Ok(ParseRouteDecision {
                route: ParseRoute::OfficeService,
                reason: RouteReason::OfficeDocument,
                probe_result: None,
                plan: ParsePlan::Office(OfficeParsePlan {
                    doc_type: if extension == "doc" {
                        OfficeDocType::Doc
                    } else {
                        OfficeDocType::Docx
                    },
                }),
            }),
            "xls" | "xlsx" => Ok(ParseRouteDecision {
                route: ParseRoute::OfficeService,
                reason: RouteReason::OfficeDocument,
                probe_result: None,
                plan: ParsePlan::Office(OfficeParsePlan {
                    doc_type: if extension == "xls" {
                        OfficeDocType::Xls
                    } else {
                        OfficeDocType::Xlsx
                    },
                }),
            }),
            _ => Err(ParseRouteError::unsupported(format!(
                "file {filename} uses unsupported extension .{extension}"
            ))),
        }
    }
}

fn build_pdf_parse_plan(
    probe_result: &ParseProbeResult,
    config: &ParseProbeConfig,
) -> PdfParsePlan {
    let fallback_backend = if probe_result.likely_scanned
        || probe_result.image_hint_count > config.image_heavy_threshold
        || probe_result.table_hint_count > config.table_heavy_threshold
    {
        PdfPageBackend::VisualRaster
    } else {
        PdfPageBackend::EdgeParse
    };
    let fallback_reason = if probe_result.likely_scanned {
        RouteReason::ScannedPdf
    } else if probe_result.image_hint_count > config.image_heavy_threshold
        || probe_result.table_hint_count > config.table_heavy_threshold
    {
        RouteReason::ComplexPdf
    } else {
        RouteReason::SimplePdf
    };

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

/// Per-page routing decision based on quality signals (v2 model).
fn route_page(page: &PdfPageProbeResult) -> (PdfPageBackend, RouteDecision, RouteReason) {
    let readable = page.readable_ratio.unwrap_or(1.0);
    let bigram = page.bigram_repeat_ratio.unwrap_or(0.0);
    let unique = page.unique_token_ratio.unwrap_or(1.0);

    // C: no text or garbage quality → PaddleOCR slow path
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

    // C′: has text + table hints but table content is garbled → single-page PaddleOCR upgrade
    if page.table_hint_count > 0 {
        let garbled = page.table_garbled_ratio.unwrap_or(0.0);
        if garbled > TABLE_GARBLE_THRESHOLD {
            return (
                PdfPageBackend::PaddleOcr,
                RouteDecision::SlowOcrSinglePage,
                RouteReason::SlowOcrSinglePage,
            );
        }
    }

    // B: has figures AND has text → EdgeParse + figure pipeline
    // Use figure_area_ratio when available (ING-1b-β), fallback to image count
    let has_figures = if let Some(ratio) = page.figure_area_ratio {
        let non_deco = page.non_decorative_image_count.unwrap_or(0);
        ratio > 0.15 && non_deco >= 2
    } else {
        page.image_hint_count >= FIG_COUNT_THRESHOLD
    };

    if has_figures {
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

fn build_pdf_page_plan(page_probe: &PdfPageProbeResult, _config: &ParseProbeConfig) -> PdfPagePlan {
    let (backend, _decision, reason) = route_page(page_probe);

    PdfPagePlan {
        page_number: page_probe.page_number,
        backend,
        reason,
    }
}

fn summarize_pdf_reason(probe_result: &ParseProbeResult, plan: &ParsePlan) -> RouteReason {
    let ParsePlan::Pdf(pdf_plan) = plan else {
        return RouteReason::SimplePdf;
    };

    let has_ocr = pdf_plan
        .pages
        .iter()
        .any(|p| p.backend == PdfPageBackend::PaddleOcr);
    let has_visual = pdf_plan
        .pages
        .iter()
        .any(|p| p.backend == PdfPageBackend::VisualRaster);
    let has_figures = pdf_plan
        .pages
        .iter()
        .any(|p| p.reason == RouteReason::FastWithFigures);

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

fn normalize_extension(filename: &str) -> Option<String> {
    let (_, extension) = filename.rsplit_once('.')?;
    let normalized = extension.trim().to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_mime_type(mime_type: &str) -> String {
    mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn is_supported_extension(extension: &str) -> bool {
    matches!(
        extension,
        "txt"
            | "md"
            | "rst"
            | "csv"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "html"
            | "htm"
            | "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "gif"
            | "bmp"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
    ) || is_code_extension(extension)
}

fn is_code_extension(extension: &str) -> bool {
    matches!(
        extension,
        "rs" | "py"
            | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "kt"
            | "scala"
            | "r"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "ps1"
            | "sql"
            | "xml"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "vue"
            | "svelte"
            | "graphql"
            | "proto"
            | "gradle"
            | "cmake"
            | "makefile"
            | "dockerfile"
            | "tf"
            | "hcl"
    )
}

fn mime_matches_extension(extension: &str, mime_type: &str) -> bool {
    match extension {
        "pdf" => mime_type == "application/pdf",
        "png" => mime_type == "image/png",
        "jpg" | "jpeg" => mime_type == "image/jpeg",
        "webp" => mime_type == "image/webp",
        "gif" => mime_type == "image/gif",
        "bmp" => mime_type == "image/bmp",
        "doc" => mime_type == "application/msword",
        "docx" => {
            mime_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        "xls" => mime_type == "application/vnd.ms-excel",
        "xlsx" => mime_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => mime_type == "application/vnd.ms-powerpoint",
        "pptx" => {
            mime_type == "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        "html" | "htm" => {
            mime_type == "application/xhtml+xml"
                || mime_type == "application/xml"
                || is_generic_text_mime(mime_type)
        }
        "json" => {
            mime_type == "application/json"
                || mime_type == "text/json"
                || mime_type == "application/ld+json"
                || mime_type == "text/plain"
        }
        "csv" => {
            mime_type == "text/csv" || mime_type == "application/csv" || mime_type == "text/plain"
        }
        "md" => mime_type == "text/markdown" || mime_type == "text/plain",
        "toml" => mime_type == "application/toml" || mime_type == "text/plain",
        "yaml" | "yml" => matches!(
            mime_type,
            "application/yaml" | "application/x-yaml" | "text/yaml" | "text/x-yaml" | "text/plain"
        ),
        "txt" | "rst" => mime_type == "text/plain",
        _ if is_code_extension(extension) => {
            is_generic_text_mime(mime_type)
                || matches!(
                    mime_type,
                    "application/javascript"
                        | "application/typescript"
                        | "application/xml"
                        | "application/sql"
                )
        }
        _ => false,
    }
}

fn is_generic_text_mime(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
}

#[cfg(test)]
mod tests {
    use super::super::probe::{ParseProbeConfig, PdfPageProbeResult};
    use super::*;

    #[test]
    fn text_file_routing_uses_local_text_parser() {
        let decision = ParseRouter::route(b"hello world", "test.txt", "text/plain").unwrap();
        assert_eq!(decision.route, ParseRoute::Local);
        assert!(matches!(decision.reason, RouteReason::TextFile));
        assert!(matches!(
            decision.plan,
            ParsePlan::Local(LocalParsePlan {
                kind: LocalParseKind::Text
            })
        ));
    }

    #[test]
    fn image_file_routing_uses_mineru_image_route() {
        let decision = ParseRouter::route(b"fake image", "test.png", "image/png").unwrap();
        assert_eq!(decision.route, ParseRoute::MineruImage);
        assert!(matches!(decision.reason, RouteReason::ImageFile));
    }

    #[test]
    fn presentation_file_routing_uses_office_service() {
        let decision = ParseRouter::route(
            b"fake ppt",
            "test.pptx",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        )
        .unwrap();
        assert_eq!(decision.route, ParseRoute::OfficeService);
        assert!(matches!(decision.reason, RouteReason::PresentationFile));
        assert!(matches!(
            decision.plan,
            ParsePlan::Office(OfficeParsePlan {
                doc_type: OfficeDocType::Pptx
            })
        ));
    }

    #[test]
    fn route_rejects_missing_extension() {
        let error = ParseRouter::route(b"hello", "README", "text/plain").expect_err("should fail");
        assert_eq!(error.code(), "unsupported_file_type");
    }

    #[test]
    fn route_rejects_unknown_mime_type() {
        let error = ParseRouter::route(b"hello", "notes.txt", "application/octet-stream")
            .expect_err("should fail");
        assert_eq!(error.code(), "unsupported_file_type");
    }

    #[test]
    fn pdf_page_plan_routes_each_page_independently() {
        let mut probe_result =
            ParseProbeResult::new("application/pdf".to_string(), "pdf".to_string());
        probe_result.page_count = Some(3);
        probe_result.pdf_page_probes = vec![
            PdfPageProbeResult {
                page_number: 1,
                extracted_text_chars: 400,
                image_hint_count: 0,
                table_hint_count: 0,
                likely_scanned: false,
                readable_ratio: Some(0.8),
                bigram_repeat_ratio: Some(0.1),
                unique_token_ratio: Some(0.7),
                watermark_hit: false,
                figure_area_ratio: None,
                non_decorative_image_count: None,
                table_garbled_ratio: None,
            },
            PdfPageProbeResult {
                page_number: 2,
                extracted_text_chars: 10,
                image_hint_count: 0,
                table_hint_count: 0,
                likely_scanned: true,
                readable_ratio: Some(0.2),
                bigram_repeat_ratio: Some(0.1),
                unique_token_ratio: Some(0.5),
                watermark_hit: false,
                figure_area_ratio: None,
                non_decorative_image_count: None,
                table_garbled_ratio: None,
            },
            PdfPageProbeResult {
                page_number: 3,
                extracted_text_chars: 350,
                image_hint_count: 0,
                table_hint_count: 0,
                likely_scanned: false,
                readable_ratio: Some(0.7),
                bigram_repeat_ratio: Some(0.1),
                unique_token_ratio: Some(0.6),
                watermark_hit: false,
                figure_area_ratio: None,
                non_decorative_image_count: None,
                table_garbled_ratio: None,
            },
        ];

        let plan = build_pdf_parse_plan(&probe_result, &ParseProbeConfig::default());
        assert_eq!(plan.pages.len(), 3);
        assert_eq!(plan.pages[0].backend, PdfPageBackend::EdgeParse);
        assert_eq!(plan.pages[1].backend, PdfPageBackend::PaddleOcr);
        assert_eq!(plan.pages[2].backend, PdfPageBackend::EdgeParse);

        let reason = summarize_pdf_reason(&probe_result, &ParsePlan::Pdf(plan));
        assert!(matches!(reason, RouteReason::ScannedPdf));
    }

    #[test]
    fn route_page_scanned_goes_to_paddle() {
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
                figure_area_ratio: None,
                non_decorative_image_count: None,
                table_garbled_ratio: None,
        };
        let (_backend, decision, _reason) = route_page(&page);
        assert_eq!(decision, RouteDecision::SlowOcr);
    }

    #[test]
    fn route_page_image_heavy_with_text_goes_to_b() {
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
                figure_area_ratio: None,
                non_decorative_image_count: None,
                table_garbled_ratio: None,
        };
        let (backend, decision, _reason) = route_page(&page);
        assert_eq!(backend, PdfPageBackend::EdgeParse);
        assert_eq!(decision, RouteDecision::FastWithFigures);
    }

    #[test]
    fn route_page_watermark_goes_to_ocr() {
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
            figure_area_ratio: None,
            non_decorative_image_count: None,
            table_garbled_ratio: None,
        };
        let (_backend, decision, _reason) = route_page(&page);
        assert_eq!(decision, RouteDecision::SlowOcr);
    }

    #[test]
    fn route_page_clean_text_goes_to_a() {
        let page = PdfPageProbeResult {
            page_number: 1,
            extracted_text_chars: 500,
            image_hint_count: 0,
            table_hint_count: 0,
            likely_scanned: false,
            readable_ratio: Some(0.8),
            bigram_repeat_ratio: Some(0.1),
            unique_token_ratio: Some(0.7),
            watermark_hit: false,
            figure_area_ratio: None,
            non_decorative_image_count: None,
            table_garbled_ratio: None,
        };
        let (backend, decision, _reason) = route_page(&page);
        assert_eq!(backend, PdfPageBackend::EdgeParse);
        assert_eq!(decision, RouteDecision::FastText);
    }

    #[test]
    fn route_page_garbled_table_goes_to_c_prime() {
        let page = PdfPageProbeResult {
            page_number: 1,
            extracted_text_chars: 300,
            image_hint_count: 0,
            table_hint_count: 5,
            likely_scanned: false,
            readable_ratio: Some(0.5),
            bigram_repeat_ratio: Some(0.1),
            unique_token_ratio: Some(0.6),
            watermark_hit: false,
            figure_area_ratio: None,
            non_decorative_image_count: None,
            table_garbled_ratio: Some(0.45),
        };
        let (backend, decision, _reason) = route_page(&page);
        assert_eq!(backend, PdfPageBackend::PaddleOcr);
        assert_eq!(decision, RouteDecision::SlowOcrSinglePage);
    }
}
