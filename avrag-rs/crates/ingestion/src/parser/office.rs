use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use async_trait::async_trait;
use calamine::{Reader, Sheets, open_workbook};

use super::{DocumentParser, Page, ParsedDocument};

pub struct OfficeParser;

#[async_trait]
impl DocumentParser for OfficeParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
        let is_xlsx =
            filename.to_lowercase().ends_with("xlsx") || filename.to_lowercase().ends_with("xls");

        let mut temp_file = tempfile::Builder::new()
            .prefix("avrag_ingest_")
            .suffix(if is_xlsx { ".xlsx" } else { ".docx" })
            .tempfile()
            .map_err(|e| anyhow::anyhow!("Failed to create temp file: {}", e))?;

        temp_file
            .write_all(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to write to temp file: {}", e))?;

        let path = temp_file.path();

        let content = if is_xlsx {
            parse_spreadsheet(path)?
        } else {
            parse_document(path)?
        };

        drop(temp_file);

        let pages = vec![Page {
            number: 1,
            content,
            cursor: "chunk-0".to_string(),
        }];

        let mut metadata = BTreeMap::new();
        metadata.insert("source_file".to_string(), filename.to_string());
        metadata.insert("parser".to_string(), "office".to_string());

        Ok(ParsedDocument {
            title: filename.to_string(),
            pages,
            metadata,
        })
    }
}

fn parse_spreadsheet(path: &Path) -> anyhow::Result<String> {
    let mut workbook: Sheets<std::io::BufReader<std::fs::File>> =
        open_workbook(path).map_err(|e| anyhow::anyhow!("Failed to open spreadsheet: {}", e))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut all_content = Vec::new();

    for name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&name) {
            for row in range.rows() {
                let row_text: Vec<String> = row
                    .iter()
                    .map(|cell| {
                        use calamine::Data;
                        match cell {
                            Data::Int(i) => i.to_string(),
                            Data::Float(f) => f.to_string(),
                            Data::String(s) => s.clone(),
                            Data::Bool(b) => b.to_string(),
                            Data::DateTime(dt) => dt.to_string(),
                            Data::DateTimeIso(s) => s.clone(),
                            Data::DurationIso(s) => s.clone(),
                            Data::Error(e) => format!("{:?}", e),
                            Data::Empty => String::new(),
                        }
                    })
                    .collect();
                let row_line = row_text.join("\t");
                if !row_line.trim().is_empty() {
                    all_content.push(row_line);
                }
            }
        }
    }

    Ok(all_content.join("\n"))
}

fn parse_document(path: &Path) -> anyhow::Result<String> {
    let mut workbook: Sheets<std::io::BufReader<std::fs::File>> =
        open_workbook(path).map_err(|e| anyhow::anyhow!("Failed to open document: {}", e))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut all_content = Vec::new();

    for name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&name) {
            for row in range.rows() {
                let row_text: Vec<String> = row
                    .iter()
                    .map(|cell| {
                        use calamine::Data;
                        match cell {
                            Data::String(s) => s.clone(),
                            Data::Int(i) => i.to_string(),
                            Data::Float(f) => f.to_string(),
                            Data::Bool(b) => b.to_string(),
                            Data::DateTime(dt) => dt.to_string(),
                            Data::DateTimeIso(s) => s.clone(),
                            Data::DurationIso(s) => s.clone(),
                            Data::Error(e) => format!("{:?}", e),
                            Data::Empty => String::new(),
                        }
                    })
                    .collect();
                let row_line = row_text.join(" ");
                if !row_line.trim().is_empty() {
                    all_content.push(row_line);
                }
            }
        }
    }

    Ok(all_content.join("\n"))
}
