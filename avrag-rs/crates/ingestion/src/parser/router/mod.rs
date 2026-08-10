use serde::{Deserialize, Serialize};
use thiserror::Error;

mod mime;
use mime::{
    is_code_extension, is_supported_extension, mime_matches_extension, normalize_extension,
    normalize_mime_type,
};

/// 解析路由（2026-08-05 起：PDF→liteparse、Office/ODF/RTF/EPUB/CSV→anydoc、
/// 文本/代码→markitdown；standalone 图片 PaddleOCR）。
/// 见 `docs/plans/2026-08-05-parser-pipeline-anydoc.md`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseRoute {
    Local,
    PaddleOcrImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteReason {
    TextFile,
    ImageFile,
    OfficeDocument,
    PresentationFile,
}

impl std::fmt::Display for RouteReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteReason::TextFile => write!(f, "text_file"),
            RouteReason::ImageFile => write!(f, "image_file"),
            RouteReason::OfficeDocument => write!(f, "office_document"),
            RouteReason::PresentationFile => write!(f, "presentation_file"),
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
    pub plan: ParsePlan,
}

/// 文档类本地解析路径（子进程 → markdown → `blocks_from_markdown`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalParseKind {
    /// markitdown 子进程（anydoc 不支持的文本/代码长尾）。
    Markitdown,
    /// liteparse PDFium 原生解析（PDF 路径）。
    LiteparseV2Pdf,
    /// anydoc 子进程（Office/ODF/RTF/EPUB/CSV 等；非 PDF）。
    Anydoc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalParsePlan {
    pub kind: LocalParseKind,
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
        if !is_supported_extension(&extension) {
            return Err(ParseRouteError::unsupported(format!(
                "file {filename} uses unsupported extension .{extension}"
            )));
        }

        let normalized_mime = normalize_mime_type(mime_type);
        // Empty / application/octet-stream: browsers often omit real MIME for .md and
        // other text files. Extension is already validated — accept and route by ext.
        if normalized_mime.is_empty() || normalized_mime == "application/octet-stream" {
            return Ok(());
        }

        if !mime_matches_extension(&extension, &normalized_mime) {
            return Err(ParseRouteError::unsupported(format!(
                "file {filename} with MIME type {normalized_mime} is not supported"
            )));
        }

        Ok(())
    }

    pub fn route(
        _bytes: &[u8],
        filename: &str,
        mime_type: &str,
    ) -> Result<ParseRouteDecision, ParseRouteError> {
        Self::ensure_supported_file_type(filename, mime_type)?;
        let extension =
            normalize_extension(filename).expect("validated file types must retain an extension");

        let local = |kind: LocalParseKind, reason: RouteReason| {
            Ok(ParseRouteDecision {
                route: ParseRoute::Local,
                reason,
                plan: ParsePlan::Local(LocalParsePlan { kind }),
            })
        };

        match extension.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Ok(ParseRouteDecision {
                route: ParseRoute::PaddleOcrImage,
                reason: RouteReason::ImageFile,
                plan: ParsePlan::External(ExternalParsePlan {
                    kind: ExternalParseKind::PaddleOcrImage,
                }),
            }),
            "pdf" => local(
                LocalParseKind::LiteparseV2Pdf,
                RouteReason::OfficeDocument,
            ),
            // anydoc 广覆盖（除 PDF）：Word / Excel / ODF text-spreadsheet / RTF / EPUB / CSV
            "doc" | "docx" | "docm" | "xls" | "xlsx" | "xlsm" | "xlsb" | "odt" | "ods" | "rtf"
            | "epub" | "csv" => local(LocalParseKind::Anydoc, RouteReason::OfficeDocument),
            // 演示文稿族
            "ppt" | "pps" | "pot" | "pptx" | "pptm" | "ppsx" | "ppsm" | "odp" => {
                local(LocalParseKind::Anydoc, RouteReason::PresentationFile)
            }
            // anydoc 不支持的文本/代码长尾
            "txt" | "md" | "rst" | "tsv" | "json" | "toml" | "yaml" | "yml" | "html" | "htm" => {
                local(LocalParseKind::Markitdown, RouteReason::TextFile)
            }
            _ if is_code_extension(&extension) => {
                local(LocalParseKind::Markitdown, RouteReason::TextFile)
            }
            _ => Err(ParseRouteError::unsupported(format!(
                "file {filename} uses unsupported extension .{extension}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests;
