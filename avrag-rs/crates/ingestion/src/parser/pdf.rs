use std::collections::BTreeMap;

use async_trait::async_trait;
use lopdf::Document;

use super::{DocumentParser, Page, ParsedDocument};

pub struct PdfParser;

#[async_trait]
impl DocumentParser for PdfParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
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

        let page_count = doc.get_pages().len() as u32;

        let mut pages = Vec::new();
        for (page_num, (page_id, _)) in doc.get_pages().iter().enumerate() {
            let page_num = page_num as u32 + 1;

            if let Ok(content) = doc.extract_text(&[*page_id]) {
                if !content.trim().is_empty() {
                    pages.push(Page {
                        number: page_num,
                        content,
                        cursor: format!("page-{}", page_num),
                    });
                }
            }
        }

        if pages.is_empty() {
            pages.push(Page {
                number: 1,
                content: String::new(),
                cursor: "page-1".to_string(),
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
