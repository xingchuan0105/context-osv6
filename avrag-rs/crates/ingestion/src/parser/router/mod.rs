use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::probe::{ParseProbe, ParseProbeConfig, ParseProbeResult};
mod mime;
mod pdf_plan;
mod stages;
use mime::{
    is_code_extension, is_supported_extension, mime_matches_extension, normalize_extension,
    normalize_mime_type,
};
use pdf_plan::{build_pdf_parse_plan, summarize_pdf_reason};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseRoute {
    Local,
    OfficeService,
    Pdf,
    MineruImage,
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

#[cfg(test)]
mod tests;
