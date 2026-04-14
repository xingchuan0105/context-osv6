use std::collections::BTreeMap;

use async_trait::async_trait;

use super::{DocumentParser, Page, ParsedDocument};

pub struct TextParser;

#[async_trait]
impl DocumentParser for TextParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
        let content = String::from_utf8_lossy(bytes).to_string();

        let pages = vec![Page {
            number: 1,
            content,
            cursor: "chunk-0".to_string(),
        }];

        let mut metadata = BTreeMap::new();
        metadata.insert("source_file".to_string(), filename.to_string());
        metadata.insert("parser".to_string(), "text".to_string());

        Ok(ParsedDocument {
            title: filename.to_string(),
            pages,
            metadata,
        })
    }
}
