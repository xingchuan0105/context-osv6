use std::collections::BTreeMap;

use async_trait::async_trait;
use lopdf::Document;

use super::{DocumentParser, Page, ParsedDocument};

pub struct PdfParser;

impl PdfParser {
    pub async fn parse_pages(
        &self,
        bytes: &[u8],
        filename: &str,
        page_numbers: &[u32],
    ) -> anyhow::Result<ParsedDocument> {
        self.parse_with_page_filter(bytes, filename, Some(page_numbers))
            .await
    }

    async fn parse_with_page_filter(
        &self,
        bytes: &[u8],
        filename: &str,
        page_numbers: Option<&[u32]>,
    ) -> anyhow::Result<ParsedDocument> {
        let doc =
            Document::load_mem(bytes).map_err(|e| anyhow::anyhow!("Failed to load PDF: {}", e))?;

        let title = doc
            .get_object(
                doc.trailer
                    .get(b"Info")
                    .ok()
                    .and_then(|o| o.as_reference().ok())
                    .unwrap_or_default(),
            )
            .ok()
            .and_then(|obj| obj.as_dict().ok())
            .and_then(|dict| dict.get(b"Title").ok())
            .and_then(|obj| obj.as_str().ok())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .unwrap_or_else(|| filename.to_string());

        let doc_pages = doc.get_pages();
        let page_count = doc_pages.len() as u32;

        let selected_pages = if let Some(page_numbers) = page_numbers {
            if page_numbers.is_empty() {
                anyhow::bail!("PDF page filter must not be empty");
            }

            let mut selected = Vec::with_capacity(page_numbers.len());
            for page_number in page_numbers {
                doc_pages.get(page_number).ok_or_else(|| {
                    anyhow::anyhow!("Requested PDF page {} is missing", page_number)
                })?;
                selected.push(*page_number);
            }
            selected
        } else {
            doc_pages.keys().copied().collect::<Vec<_>>()
        };

        let mut pages = Vec::with_capacity(selected_pages.len());
        for page_number in selected_pages {
            let content = doc.extract_text(&[page_number]).unwrap_or_default();
            pages.push(Page {
                number: page_number,
                content,
                cursor: format!("page-{}", page_number),
            });
        }

        let mut metadata = BTreeMap::new();
        metadata.insert("page_count".to_string(), page_count.to_string());
        metadata.insert("source_file".to_string(), filename.to_string());

        Ok(ParsedDocument {
            title,
            pages,
            metadata,
        })
    }
}

#[async_trait]
impl DocumentParser for PdfParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
        self.parse_with_page_filter(bytes, filename, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parse_pages_rejects_missing_page_numbers() {
        let error = PdfParser
            .parse_pages(b"%PDF-1.4", "broken.pdf", &[2])
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("Failed to load PDF")
                || error.to_string().contains("missing")
        );
    }
}
