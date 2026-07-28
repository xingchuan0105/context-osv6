use std::collections::BTreeMap;

use async_trait::async_trait;

use super::{DocumentParser, Page, ParsedDocument};

pub struct TextParser;

#[async_trait]
impl DocumentParser for TextParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
        let content = String::from_utf8(bytes.to_vec())
            .map_err(|error| anyhow::anyhow!("Text parser requires valid UTF-8: {error}"))?;

        let mut metadata = BTreeMap::new();
        metadata.insert("source_file".to_string(), filename.to_string());
        metadata.insert("parser".to_string(), "text".to_string());

        // T3: .csv/.tsv try the csv-crate grid parse first; the TableIr rides
        // in metadata and `from_normalized_document` emits ONE Table block
        // (malformed CSV → None → stays the plain-text path).
        let extension = filename
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let delimiter = match extension.as_str() {
            "csv" => Some(b','),
            "tsv" => Some(b'\t'),
            _ => None,
        };
        if let Some(delimiter) = delimiter
            && let Some(table) = super::csv_table::try_parse_csv(&content, delimiter)
        {
            metadata.insert(
                "csv_table_ir".to_string(),
                serde_json::to_string(&table).unwrap_or_else(|_| "{}".to_string()),
            );
            metadata.insert("table_parser".to_string(), "csv-v1".to_string());
        }

        let pages = vec![Page {
            number: 1,
            content,
            cursor: "chunk-0".to_string(),
        }];

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

    /// T3: a .csv file pre-parses into csv_table_ir metadata, and the IR
    /// projection emits ONE Table block with the markdown surface.
    #[tokio::test]
    async fn csv_file_produces_single_table_block() {
        let parsed = TextParser
            .parse("编号,名称,数量\n1,速冻机,10\n2,冷却塔,3\n".as_bytes(), "库存.csv")
            .await
            .unwrap();
        assert!(parsed.metadata.contains_key("csv_table_ir"));
        assert_eq!(
            parsed.metadata.get("table_parser").map(String::as_str),
            Some("csv-v1")
        );

        let normalized = crate::parser::normalize_parsed_document(&parsed, "text_local");
        let document = crate::ir::DocumentIr::from_normalized_document(
            "doc-csv",
            crate::ir::DocumentType::Text,
            crate::ir::ParseBackend::TextLocal,
            &normalized,
        );
        assert_eq!(document.blocks.len(), 1);
        let block = &document.blocks[0];
        assert_eq!(block.block_type, crate::ir::BlockType::Table);
        let table = crate::ir::TableIr::from_block(block).expect("table_ir");
        assert_eq!(table.headers, vec!["编号", "名称", "数量"]);
        assert_eq!(table.rows.len(), 2);
        assert!(block.text.contains("|编号|名称|数量|"));
        assert_eq!(block.source_locator.row_range, Some((1, 2)));
    }

    /// T3: malformed CSV degrades to the plain-text path (no csv_table_ir).
    #[tokio::test]
    async fn malformed_csv_stays_plain_text() {
        let parsed = TextParser
            .parse("a,b\n1,2,3\n".as_bytes(), "broken.csv")
            .await
            .unwrap();
        assert!(!parsed.metadata.contains_key("csv_table_ir"));
        let normalized = crate::parser::normalize_parsed_document(&parsed, "text_local");
        let document = crate::ir::DocumentIr::from_normalized_document(
            "doc-bad",
            crate::ir::DocumentType::Text,
            crate::ir::ParseBackend::TextLocal,
            &normalized,
        );
        assert!(
            document
                .blocks
                .iter()
                .all(|b| b.block_type == crate::ir::BlockType::Paragraph),
            "degraded CSV stays prose: {:?}",
            document.blocks.iter().map(|b| &b.block_type).collect::<Vec<_>>()
        );
    }

    /// T3: .tsv files take the TAB delimiter path.
    #[tokio::test]
    async fn tsv_file_uses_tab_delimiter() {
        let parsed = TextParser
            .parse("编号\t阶段\n1\t验证阶段\n".as_bytes(), "data.tsv")
            .await
            .unwrap();
        assert!(parsed.metadata.contains_key("csv_table_ir"));
    }
}

