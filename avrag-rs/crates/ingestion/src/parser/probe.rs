use anyhow::Result;
use lopdf::Document;
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPageProbeResult {
    pub page_number: u32,
    pub extracted_text_chars: usize,
    pub image_hint_count: usize,
    pub table_hint_count: usize,
    pub likely_scanned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseProbeResult {
    pub mime_type: String,
    pub extension: String,
    pub extracted_text_chars: usize,
    pub page_count: Option<u32>,
    pub image_hint_count: usize,
    pub table_hint_count: usize,
    pub likely_scanned: bool,
    pub likely_presentation: bool,
    pub pdf_page_probes: Vec<PdfPageProbeResult>,
}

impl ParseProbeResult {
    pub fn new(mime_type: String, extension: String) -> Self {
        Self {
            mime_type,
            extension,
            extracted_text_chars: 0,
            page_count: None,
            image_hint_count: 0,
            table_hint_count: 0,
            likely_scanned: false,
            likely_presentation: false,
            pdf_page_probes: Vec::new(),
        }
    }
}

pub struct ParseProbe;

impl ParseProbe {
    pub fn probe(bytes: &[u8], filename: &str) -> Result<ParseProbeResult> {
        let extension = filename
            .rsplit('.')
            .next()
            .unwrap_or("unknown")
            .to_lowercase();

        let mime_type = Self::guess_mime_type(&extension);

        match extension.as_str() {
            "pdf" => Self::probe_pdf(bytes, &extension, &mime_type),
            "ppt" | "pptx" => Self::probe_presentation(bytes, &extension, &mime_type),
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => {
                Self::probe_image(bytes, &extension, &mime_type)
            }
            _ => Ok(ParseProbeResult::new(mime_type, extension)),
        }
    }

    fn probe_pdf(bytes: &[u8], extension: &str, mime_type: &str) -> Result<ParseProbeResult> {
        let mut result = ParseProbeResult::new(mime_type.to_string(), extension.to_string());

        let doc = match Document::load_mem(bytes) {
            Ok(doc) => doc,
            Err(e) => {
                debug!(error = %e, "Failed to load PDF for probing");
                result.likely_scanned = true;
                return Ok(result);
            }
        };

        let pages = doc.get_pages();
        result.page_count = Some(pages.len() as u32);

        let mut total_text_chars = 0;
        let mut image_hint_count = 0;
        let mut table_hint_count = 0;
        let mut page_probes = Vec::with_capacity(pages.len());

        for (page_num, (page_id, _)) in pages.iter().enumerate() {
            let page_number = page_num as u32 + 1;
            let (page_text_chars, page_image_hints, page_table_hints) =
                if let Ok(content) = doc.extract_text(&[*page_id]) {
                    let lower_content = content.to_lowercase();
                    (
                        content.len(),
                        lower_content.matches("figure").count()
                            + lower_content.matches("image").count()
                            + lower_content.matches("chart").count()
                            + lower_content.matches("diagram").count(),
                        lower_content.matches("table").count()
                            + lower_content.matches("|").count()
                            + lower_content.matches("─────").count(),
                    )
                } else {
                    (0, 0, 0)
                };

            if page_num < 3 {
                total_text_chars += page_text_chars;
                image_hint_count += page_image_hints;
                table_hint_count += page_table_hints;
            }

            page_probes.push(PdfPageProbeResult {
                page_number,
                extracted_text_chars: page_text_chars,
                image_hint_count: page_image_hints,
                table_hint_count: page_table_hints,
                likely_scanned: page_text_chars < 100,
            });
        }

        result.extracted_text_chars = total_text_chars;
        result.image_hint_count = image_hint_count;
        result.table_hint_count = table_hint_count;
        result.pdf_page_probes = page_probes;

        let inspected_pages = pages.len().min(3);
        let avg_text_per_page = if inspected_pages > 0 {
            total_text_chars / inspected_pages
        } else {
            0
        };
        result.likely_scanned = avg_text_per_page < 100;

        Ok(result)
    }

    fn probe_presentation(
        _bytes: &[u8],
        extension: &str,
        mime_type: &str,
    ) -> Result<ParseProbeResult> {
        let mut result = ParseProbeResult::new(mime_type.to_string(), extension.to_string());
        result.likely_presentation = true;
        Ok(result)
    }

    fn probe_image(_bytes: &[u8], extension: &str, mime_type: &str) -> Result<ParseProbeResult> {
        let mut result = ParseProbeResult::new(mime_type.to_string(), extension.to_string());
        result.likely_presentation = true;
        Ok(result)
    }

    fn guess_mime_type(extension: &str) -> String {
        match extension {
            "pdf" => "application/pdf".to_string(),
            "ppt" => "application/vnd.ms-powerpoint".to_string(),
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                .to_string(),
            "doc" => "application/msword".to_string(),
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .to_string(),
            "xls" => "application/vnd.ms-excel".to_string(),
            "xlsx" => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()
            }
            "png" => "image/png".to_string(),
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "gif" => "image/gif".to_string(),
            "webp" => "image/webp".to_string(),
            "bmp" => "image/bmp".to_string(),
            "txt" => "text/plain".to_string(),
            "md" => "text/markdown".to_string(),
            "html" | "htm" => "text/html".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_result_new() {
        let result = ParseProbeResult::new("text/plain".to_string(), "txt".to_string());
        assert_eq!(result.mime_type, "text/plain");
        assert_eq!(result.extension, "txt");
        assert_eq!(result.extracted_text_chars, 0);
        assert!(!result.likely_scanned);
        assert!(result.pdf_page_probes.is_empty());
    }

    #[test]
    fn test_guess_mime_type() {
        assert_eq!(ParseProbe::guess_mime_type("pdf"), "application/pdf");
        assert_eq!(
            ParseProbe::guess_mime_type("pptx"),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        );
        assert_eq!(ParseProbe::guess_mime_type("png"), "image/png");
        assert_eq!(ParseProbe::guess_mime_type("txt"), "text/plain");
    }
}
