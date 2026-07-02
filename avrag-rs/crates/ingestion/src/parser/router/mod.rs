use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::probe::{ParseProbeConfig, ParseProbeResult};
mod mime;
mod page_routes;
mod pdf_plan;
use mime::{
    is_code_extension, is_supported_extension, mime_matches_extension, normalize_extension,
    normalize_mime_type,
};
use pdf_plan::{build_pdf_parse_plan, summarize_pdf_reason};

pub use pdf_plan::pdf_page_route_labels;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseRoute {
    Local,
    OfficeService,
    Pdf,
    PaddleOcrImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Canonical per-page route label for metadata (`page_routes`, `page_status`).
pub fn page_route_label(decision: RouteDecision) -> &'static str {
    match decision {
        RouteDecision::FastText => "A",
        RouteDecision::FastWithFigures => "B",
        RouteDecision::SlowOcr => "C",
        RouteDecision::SlowOcrSinglePage => "C_prime",
        RouteDecision::Fallback => "fallback",
    }
}

impl RouteReason {
    /// Short per-page route label for metadata (`page_routes`, `page_status`).
    pub fn route_label(&self) -> &'static str {
        match self {
            RouteReason::FastText => page_route_label(RouteDecision::FastText),
            RouteReason::FastWithFigures => page_route_label(RouteDecision::FastWithFigures),
            RouteReason::SlowOcr => page_route_label(RouteDecision::SlowOcr),
            RouteReason::SlowOcrSinglePage => page_route_label(RouteDecision::SlowOcrSinglePage),
            RouteReason::OcrFallback => page_route_label(RouteDecision::Fallback),
            _ => "unknown",
        }
    }
}

impl RouteDecision {
    /// Short per-page route label aligned with [`RouteReason::route_label`].
    pub fn route_label(&self) -> &'static str {
        page_route_label(*self)
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
    /// Single LiteParse parse pass from routing; worker reuses instead of re-parsing.
    #[serde(skip)]
    pub liteparse_snapshot: Option<super::liteparse::ParsedPdfSnapshot>,
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

/// Per-page PDF routing backend (serialized in parse plans and worker metadata).
///
/// Wire names are stable for stored plans; see [`Self::LITEPARSE_TEXT`] for post-P4 semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfPageBackend {
    /// Wire name `edge_parse`: LiteParse digital text path (not lopdf primary parse).
    EdgeParse,
    PaddleOcr,
    VisualRaster,
}

impl PdfPageBackend {
    /// Post-P4 alias for [`Self::EdgeParse`] — LiteParse text extraction on A/B routes.
    pub const LITEPARSE_TEXT: Self = Self::EdgeParse;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageRouteKind {
    Text,
    Figure,
    TableOcr,
    ScanOcr,
}

impl PageRouteKind {
    pub fn as_label(&self) -> &'static str {
        match self {
            Self::Text => "A",
            Self::Figure => "B",
            Self::TableOcr => "C",
            Self::ScanOcr => "D",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPagePlan {
    pub page_number: u32,
    pub backend: PdfPageBackend,
    pub reason: RouteReason,
    /// Composable page routes (A/B/C/D may combine).
    #[serde(default)]
    pub route_kinds: Vec<PageRouteKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfParsePlan {
    pub pages: Vec<PdfPagePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalParseKind {
    PaddleOcrImage,
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
        Self::route_with_config(bytes, filename, mime_type, &ParseProbeConfig::from_env())
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
                    liteparse_snapshot: None,
                })
            }
            "html" | "htm" => Ok(ParseRouteDecision {
                route: ParseRoute::Local,
                reason: RouteReason::TextFile,
                probe_result: None,
                plan: ParsePlan::Local(LocalParsePlan {
                    kind: LocalParseKind::Html,
                }),
                liteparse_snapshot: None,
            }),
            _ if is_code_extension(&extension) => Ok(ParseRouteDecision {
                route: ParseRoute::Local,
                reason: RouteReason::TextFile,
                probe_result: None,
                plan: ParsePlan::Local(LocalParsePlan {
                    kind: LocalParseKind::Code,
                }),
                liteparse_snapshot: None,
            }),
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Ok(ParseRouteDecision {
                route: ParseRoute::PaddleOcrImage,
                reason: RouteReason::ImageFile,
                probe_result: None,
                plan: ParsePlan::External(ExternalParsePlan {
                    kind: ExternalParseKind::PaddleOcrImage,
                }),
                liteparse_snapshot: None,
            }),
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
                liteparse_snapshot: None,
            }),
            "ppt" | "pptx" => Ok(ParseRouteDecision {
                route: ParseRoute::OfficeService,
                reason: RouteReason::OfficeDocument,
                probe_result: None,
                plan: ParsePlan::Office(OfficeParsePlan {
                    doc_type: if extension == "ppt" {
                        OfficeDocType::Ppt
                    } else {
                        OfficeDocType::Pptx
                    },
                }),
                liteparse_snapshot: None,
            }),
            "pdf" => {
                let hybrid =
                    super::liteparse_probe_bridge::probe_pdf_hybrid(bytes, filename, config)
                        .map_err(|error| ParseRouteError::unsupported(error.to_string()))?;
                let plan = ParsePlan::Pdf(build_pdf_parse_plan(&hybrid.probe_result, config));
                Ok(ParseRouteDecision {
                    route: ParseRoute::Pdf,
                    reason: summarize_pdf_reason(&hybrid.probe_result, &plan),
                    probe_result: Some(hybrid.probe_result),
                    plan,
                    liteparse_snapshot: Some(hybrid.liteparse_snapshot),
                })
            }
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
                liteparse_snapshot: None,
            }),
            _ => Err(ParseRouteError::unsupported(format!(
                "file {filename} uses unsupported extension .{extension}"
            ))),
        }
    }
}

/// Build a PDF parse plan from probe signals (used by tests and tooling).
pub fn pdf_parse_plan_for_probe(
    probe_result: &ParseProbeResult,
    config: &ParseProbeConfig,
) -> PdfParsePlan {
    build_pdf_parse_plan(probe_result, config)
}

#[cfg(test)]
mod tests;
