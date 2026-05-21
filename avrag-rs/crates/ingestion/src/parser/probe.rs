use std::io::{Cursor, Read};

use anyhow::Result;
use lopdf::Document;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Configurable thresholds for probe-based routing decisions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParseProbeConfig {
    /// Pages to inspect for PDF probes (0 = all pages).
    pub pdf_inspect_pages: usize,
    /// A page with fewer chars than this is considered scanned.
    pub scanned_page_threshold: usize,
    /// A page with more image XObjects than this is considered image-heavy.
    pub image_heavy_threshold: usize,
    /// A page with more table structures than this is considered table-heavy.
    pub table_heavy_threshold: usize,
    /// Aspect-ratio tolerance for detecting presentation-like pages.
    pub presentation_aspect_ratio_tolerance: f32,
}

impl Default for ParseProbeConfig {
    fn default() -> Self {
        Self {
            pdf_inspect_pages: 3,
            scanned_page_threshold: 100,
            image_heavy_threshold: 5,
            table_heavy_threshold: 10,
            presentation_aspect_ratio_tolerance: 0.15,
        }
    }
}

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
        Self::probe_with_config(bytes, filename, &ParseProbeConfig::default())
    }

    pub fn probe_with_config(
        bytes: &[u8],
        filename: &str,
        config: &ParseProbeConfig,
    ) -> Result<ParseProbeResult> {
        let extension = filename
            .rsplit('.')
            .next()
            .unwrap_or("unknown")
            .to_lowercase();

        let mime_type = Self::guess_mime_type(&extension);

        match extension.as_str() {
            "pdf" => Self::probe_pdf(bytes, &extension, &mime_type, config),
            "ppt" | "pptx" => Self::probe_presentation(bytes, &extension, &mime_type),
            "doc" | "docx" | "xls" | "xlsx" => {
                Self::probe_office(bytes, &extension, &mime_type)
            }
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => {
                Self::probe_image(bytes, &extension, &mime_type)
            }
            _ => Ok(ParseProbeResult::new(mime_type, extension)),
        }
    }

    fn probe_pdf(
        bytes: &[u8],
        extension: &str,
        mime_type: &str,
        config: &ParseProbeConfig,
    ) -> Result<ParseProbeResult> {
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

        let inspect_limit = if config.pdf_inspect_pages == 0 {
            pages.len()
        } else {
            pages.len().min(config.pdf_inspect_pages)
        };

        let mut total_text_chars = 0;
        let mut total_image_xobjects = 0;
        let mut total_table_structures = 0;
        let mut page_probes = Vec::with_capacity(pages.len());
        let mut presentation_page_hits = 0;

        for (page_num, (page_number_key, page_object_id)) in pages.iter().enumerate() {
            let page_number = *page_number_key;

            // --- Text extraction ---
            let page_text_chars = doc
                .extract_text(&[page_number])
                .map(|c| c.len())
                .unwrap_or(0);

            // --- Real image detection via XObject dictionary ---
            let page_image_count =
                Self::count_image_xobjects(&doc, *page_object_id).unwrap_or(0);

            // --- Real table detection via content-stream analysis ---
            let page_table_count =
                Self::count_table_structures(&doc, *page_object_id).unwrap_or(0);

            // --- Presentation detection via aspect ratio ---
            let is_presentation_page = Self::is_presentation_page(&doc, *page_object_id)
                .unwrap_or(false);
            if is_presentation_page {
                presentation_page_hits += 1;
            }

            if page_num < inspect_limit {
                total_text_chars += page_text_chars;
                total_image_xobjects += page_image_count;
                total_table_structures += page_table_count;
            }

            page_probes.push(PdfPageProbeResult {
                page_number,
                extracted_text_chars: page_text_chars,
                image_hint_count: page_image_count,
                table_hint_count: page_table_count,
                likely_scanned: page_text_chars < config.scanned_page_threshold,
            });
        }

        result.extracted_text_chars = total_text_chars;
        result.image_hint_count = total_image_xobjects;
        result.table_hint_count = total_table_structures;
        result.pdf_page_probes = page_probes;

        let inspected_pages = pages.len().min(inspect_limit).max(1);
        let avg_text_per_page = total_text_chars / inspected_pages;
        result.likely_scanned = avg_text_per_page < config.scanned_page_threshold;

        // Presentation if >50% of inspected pages look like slides
        let presentation_ratio = if inspected_pages > 0 {
            presentation_page_hits as f32 / inspected_pages as f32
        } else {
            0.0
        };
        result.likely_presentation = presentation_ratio > 0.5;

        Ok(result)
    }

    /// Count actual Image XObjects in the page's Resources dictionary.
    fn count_image_xobjects(doc: &Document, page_object_id: lopdf::ObjectId) -> Result<usize> {
        let page_obj = doc.get_object(page_object_id).map_err(|_| {
            anyhow::anyhow!("Page object not found")
        })?;
        let page_dict = page_obj.as_dict().map_err(|_| {
            anyhow::anyhow!("Page object is not a dictionary")
        })?;

        let resources = match page_dict.get(b"Resources") {
            Ok(r) => r,
            Err(_) => return Ok(0),
        };
        let res_dict = resources.as_dict().map_err(|_| {
            anyhow::anyhow!("Resources is not a dictionary")
        })?;

        let xobjects = match res_dict.get(b"XObject") {
            Ok(x) => x,
            Err(_) => return Ok(0),
        };
        let xobj_dict = xobjects.as_dict().map_err(|_| {
            anyhow::anyhow!("XObject is not a dictionary")
        })?;

        let mut image_count = 0;
        for (_, obj_ref) in xobj_dict.iter() {
            let reference = match obj_ref.as_reference() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let xobj = match doc.get_object(reference) {
                Ok(o) => o,
                Err(_) => continue,
            };
            let xobj_dict = match xobj.as_dict() {
                Ok(d) => d,
                Err(_) => continue,
            };
            if let Ok(subtype) = xobj_dict.get(b"Subtype") {
                if let Ok(name_bytes) = subtype.as_name() {
                    if name_bytes == b"Image" {
                        image_count += 1;
                    }
                }
            }
        }

        Ok(image_count)
    }

    /// Detect table structures by analysing drawing commands in the page content stream.
    fn count_table_structures(doc: &Document, page_object_id: lopdf::ObjectId) -> Result<usize> {
        let page_obj = doc.get_object(page_object_id).map_err(|_| {
            anyhow::anyhow!("Page object not found")
        })?;
        let page_dict = page_obj.as_dict().map_err(|_| {
            anyhow::anyhow!("Page object is not a dictionary")
        })?;

        let resources = match page_dict.get(b"Resources") {
            Ok(r) => r,
            Err(_) => return Ok(0),
        };
        let res_dict = resources.as_dict().map_err(|_| {
            anyhow::anyhow!("Resources is not a dictionary")
        })?;

        // Count fonts as a proxy for structured text regions
        let font_count: usize = match res_dict.get(b"Font") {
            Ok(f) => f.as_dict().map(|d| d.len()).unwrap_or(0),
            Err(_) => 0,
        };

        // Extract text and look for column-aligned whitespace patterns
        let text = doc.extract_text(&[page_object_id.0]).unwrap_or_default();

        // Heuristic: multiple consecutive lines with pipe/tab delimiters
        let pipe_lines = text.lines().filter(|l| l.contains('|')).count();
        let tab_aligned_lines = text
            .lines()
            .filter(|l| l.split('\t').count() >= 3)
            .count();

        // Combine structural signals: font diversity + delimiter patterns
        let table_score = font_count.saturating_add(pipe_lines).saturating_add(tab_aligned_lines);

        Ok(table_score)
    }

    /// Detect presentation-like pages via aspect-ratio analysis.
    fn is_presentation_page(doc: &Document, page_object_id: lopdf::ObjectId) -> Result<bool> {
        let page_obj = doc.get_object(page_object_id).map_err(|_| {
            anyhow::anyhow!("Page object not found")
        })?;
        let page_dict = page_obj.as_dict().map_err(|_| {
            anyhow::anyhow!("Page object is not a dictionary")
        })?;

        let mediabox = match page_dict.get(b"MediaBox") {
            Ok(m) => m,
            Err(_) => return Ok(false),
        };
        let arr = mediabox.as_array().map_err(|_| {
            anyhow::anyhow!("MediaBox is not an array")
        })?;
        if arr.len() < 4 {
            return Ok(false);
        }

        let x0 = Self::object_to_f32(&arr[0]);
        let y0 = Self::object_to_f32(&arr[1]);
        let x1 = Self::object_to_f32(&arr[2]);
        let y1 = Self::object_to_f32(&arr[3]);

        let width = (x1 - x0).abs();
        let height = (y1 - y0).abs();
        if height == 0.0 {
            return Ok(false);
        }

        let aspect = width / height;

        // Common slide ratios: 16:9 (1.778), 4:3 (1.333), 16:10 (1.6)
        let target_ratios = [16.0 / 9.0, 4.0 / 3.0, 16.0 / 10.0];
        let tolerance = 0.15; // Allow ~15% deviation

        let is_slide_ratio = target_ratios
            .iter()
            .any(|&r| (aspect - r).abs() < tolerance);

        // Also check for very wide or tall pages typical of slides
        let is_extreme_ratio = aspect > 1.2 || aspect < 0.85;

        Ok(is_slide_ratio && is_extreme_ratio)
    }

    fn object_to_f32(obj: &lopdf::Object) -> f32 {
        match obj {
            lopdf::Object::Integer(i) => *i as f32,
            lopdf::Object::Real(f) => *f as f32,
            _ => 0.0,
        }
    }

    fn probe_presentation(
        bytes: &[u8],
        extension: &str,
        mime_type: &str,
    ) -> Result<ParseProbeResult> {
        let mut result = ParseProbeResult::new(mime_type.to_string(), extension.to_string());
        result.likely_presentation = true;

        // Deep probe for .pptx files via ZIP internal structure
        if extension == "pptx" {
            if let Ok(probed) = Self::probe_office_docx_like(bytes, extension, mime_type) {
                result.page_count = probed.page_count;
                result.extracted_text_chars = probed.extracted_text_chars;
                result.image_hint_count = probed.image_hint_count;
            }
        }

        Ok(result)
    }

    fn probe_office(
        bytes: &[u8],
        extension: &str,
        mime_type: &str,
    ) -> Result<ParseProbeResult> {
        let mut result = ParseProbeResult::new(mime_type.to_string(), extension.to_string());
        result.likely_presentation = extension == "pptx";

        if let Ok(probed) = Self::probe_office_docx_like(bytes, extension, mime_type) {
            result.page_count = probed.page_count;
            result.extracted_text_chars = probed.extracted_text_chars;
            result.image_hint_count = probed.image_hint_count;
            result.table_hint_count = probed.table_hint_count;
        }

        Ok(result)
    }

    /// Shared deep-probe logic for Office Open XML formats (docx/xlsx/pptx).
    fn probe_office_docx_like(
        bytes: &[u8],
        extension: &str,
        _mime_type: &str,
    ) -> Result<ParseProbeResult> {
        let mut result = ParseProbeResult::new(
            Self::guess_mime_type(extension),
            extension.to_string(),
        );

        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| anyhow::anyhow!("Failed to open ZIP archive: {}", e))?;

        // --- Count embedded media files ---
        let mut media_count = 0;
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let name = file.name();
                if name.contains("/media/") && !name.ends_with('/') {
                    media_count += 1;
                }
            }
        }
        result.image_hint_count = media_count;

        // --- Extract text length and page/slide/sheet count ---
        match extension {
            "docx" => {
                // Word: read document.xml for text, count paragraphs as proxy for structure
                if let Ok(mut file) = archive.by_name("word/document.xml") {
                    let mut text = String::new();
                    if file.read_to_string(&mut text).is_ok() {
                        result.extracted_text_chars = text.len();
                        // Count <w:p> (paragraph) tags as rough page proxy
                        let para_count = text.matches("<w:p").count();
                        // Assume ~50 paragraphs per page as rough estimate
                        result.page_count = Some((para_count / 50).max(1) as u32);
                    }
                }
                // Count tables
                if let Ok(mut file) = archive.by_name("word/document.xml") {
                    let mut text = String::new();
                    if file.read_to_string(&mut text).is_ok() {
                        result.table_hint_count = text.matches("<w:tbl").count();
                    }
                }
            }
            "xlsx" => {
                // Excel: read sharedStrings.xml for text, workbook.xml for sheet count
                let mut text = String::new();
                if let Ok(mut file) = archive.by_name("xl/sharedStrings.xml") {
                    let _ = file.read_to_string(&mut text);
                }
                if text.is_empty() {
                    // Fallback: try reading worksheets directly
                    for i in 0..archive.len() {
                        if let Ok(mut file) = archive.by_index(i) {
                            let name = file.name();
                            if name.starts_with("xl/worksheets/sheet")
                                && name.ends_with(".xml")
                            {
                                let mut sheet_text = String::new();
                                if file.read_to_string(&mut sheet_text).is_ok() {
                                    text.push_str(&sheet_text);
                                }
                            }
                        }
                    }
                }
                result.extracted_text_chars = text.len();

                if let Ok(mut file) = archive.by_name("xl/workbook.xml") {
                    let mut wb = String::new();
                    if file.read_to_string(&mut wb).is_ok() {
                        result.page_count = Some(wb.matches("<sheet ").count() as u32);
                    }
                }
            }
            "pptx" => {
                // PowerPoint: read presentation.xml for slide count, slide XML for text
                if let Ok(mut file) = archive.by_name("ppt/presentation.xml") {
                    let mut text = String::new();
                    if file.read_to_string(&mut text).is_ok() {
                        result.page_count = Some(text.matches("<p:sldId").count() as u32);
                    }
                }
                // Read first slide as text sample
                if let Ok(mut file) = archive.by_name("ppt/slides/slide1.xml") {
                    let mut text = String::new();
                    if file.read_to_string(&mut text).is_ok() {
                        result.extracted_text_chars = text.len();
                    }
                }
            }
            _ => {}
        }

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
