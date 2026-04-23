use std::collections::BTreeMap;

use async_trait::async_trait;

use super::{DocumentParser, Page, ParsedDocument};

pub struct TextParser;

#[async_trait]
impl DocumentParser for TextParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
        let content = String::from_utf8(bytes.to_vec())
            .map_err(|error| anyhow::anyhow!("Text parser requires valid UTF-8: {error}"))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn text_parser_rejects_invalid_utf8() {
        let error = TextParser
            .parse(&[0xff, 0xfe], "notes.txt")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("valid UTF-8"));
    }
}
