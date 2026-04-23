use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::probe::{ParseProbe, ParseProbeResult, PdfPageProbeResult};

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
pub enum RouteReason {
    TextFile,
    ImageFile,
    OfficeDocument,
    PresentationFile,
    SimplePdf,
    ComplexPdf,
    ScannedPdf,
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
    MineruOcr,
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
                let probe_result = ParseProbe::probe(bytes, filename)
                    .map_err(|error| ParseRouteError::unsupported(error.to_string()))?;
                let plan = ParsePlan::Pdf(build_pdf_parse_plan(&probe_result));
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

fn build_pdf_parse_plan(probe_result: &ParseProbeResult) -> PdfParsePlan {
    let fallback_backend = if probe_result.likely_scanned
        || probe_result.image_hint_count > 5
        || probe_result.table_hint_count > 10
    {
        PdfPageBackend::MineruOcr
    } else {
        PdfPageBackend::EdgeParse
    };
    let fallback_reason = if probe_result.likely_scanned {
        RouteReason::ScannedPdf
    } else if probe_result.image_hint_count > 5 || probe_result.table_hint_count > 10 {
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
            .map(build_pdf_page_plan)
            .collect()
    };

    PdfParsePlan { pages }
}

fn build_pdf_page_plan(page_probe: &PdfPageProbeResult) -> PdfPagePlan {
    let (backend, reason) = if page_probe.likely_scanned {
        (PdfPageBackend::MineruOcr, RouteReason::ScannedPdf)
    } else if page_probe.image_hint_count > 5 || page_probe.table_hint_count > 10 {
        (PdfPageBackend::MineruOcr, RouteReason::ComplexPdf)
    } else {
        (PdfPageBackend::EdgeParse, RouteReason::SimplePdf)
    };

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

    if pdf_plan
        .pages
        .iter()
        .any(|page| page.backend == PdfPageBackend::MineruOcr)
    {
        if probe_result.likely_scanned
            || pdf_plan
                .pages
                .iter()
                .all(|page| page.reason == RouteReason::ScannedPdf)
        {
            RouteReason::ScannedPdf
        } else {
            RouteReason::ComplexPdf
        }
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
    use super::super::probe::PdfPageProbeResult;
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
            },
            PdfPageProbeResult {
                page_number: 2,
                extracted_text_chars: 10,
                image_hint_count: 0,
                table_hint_count: 0,
                likely_scanned: true,
            },
            PdfPageProbeResult {
                page_number: 3,
                extracted_text_chars: 350,
                image_hint_count: 0,
                table_hint_count: 0,
                likely_scanned: false,
            },
        ];

        let plan = build_pdf_parse_plan(&probe_result);
        assert_eq!(plan.pages.len(), 3);
        assert_eq!(plan.pages[0].backend, PdfPageBackend::EdgeParse);
        assert_eq!(plan.pages[1].backend, PdfPageBackend::MineruOcr);
        assert_eq!(plan.pages[2].backend, PdfPageBackend::EdgeParse);

        let reason = summarize_pdf_reason(&probe_result, &ParsePlan::Pdf(plan));
        assert!(matches!(reason, RouteReason::ComplexPdf));
    }
}
