use anyhow::{Context, Result};
use lopdf::Document;

use super::{NormalizedDocument, ParsedUnit};

pub(crate) fn is_low_value_pdf_upload_page(bytes: &[u8]) -> Result<bool> {
    let document =
        Document::load_mem(bytes).context("Failed to load PDF page for OCR skip check")?;
    let Some(page_id) = document.get_pages().values().next().copied() else {
        return Ok(true);
    };
    let content = document.get_page_content(page_id).unwrap_or_default();
    Ok(!pdf_content_has_renderable_operations(&content))
}

fn pdf_content_has_renderable_operations(content: &[u8]) -> bool {
    let content = String::from_utf8_lossy(content);
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.contains(" Tj")
        || compact.contains(" TJ")
        || compact.contains(" '")
        || compact.contains(" \"")
        || compact.contains(" Do")
        || compact.contains(" BI")
}
pub(crate) fn skipped_ocr_page_unit(page_number: u32) -> ParsedUnit {
    let mut unit = ParsedUnit::new_text(page_number, String::new(), "mineru_pdf_ocr".to_string());
    unit.metadata
        .insert("ocr_skipped".to_string(), "low_value".to_string());
    unit
}

pub(crate) fn is_low_value_ocr_document(document: &NormalizedDocument) -> bool {
    let signal_chars = document
        .units
        .iter()
        .map(|unit| meaningful_ocr_signal_chars(&unit.text))
        .sum::<usize>();
    signal_chars < 8
}

fn meaningful_ocr_signal_chars(text: &str) -> usize {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("[Image:"))
        .flat_map(str::chars)
        .filter(|ch| ch.is_alphanumeric())
        .count()
}
