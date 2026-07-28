//! T3 (2026-07-28, table-aware ingestion §4.2): xlsx/xls grid parsing via
//! calamine — replaces the office service's XML tag stripper for Excel files
//! (which flattened every sheet into one text block and discarded the grid).
//!
//! Per non-empty sheet one `BlockType::SheetTable` block is emitted:
//! markdown `text` (embedding/lexical surface) + structured `TableIr` in
//! `block.metadata["table_ir"]` (T1 shape), real `sheet_name` / `table_index`
//! / `row_range` in the source locator. Mapping decisions:
//! - headers = first non-empty row of the sheet (after trimming empty
//!   leading rows); remaining non-empty rows are data rows;
//! - fully-empty sheets are skipped; a workbook with no non-empty sheet
//!   parses to an empty-block DocumentIr (honest nothing, not an error);
//! - merged cells are not reconstructed (calamine's auto reader does not
//!   expose ranges) — cell values are taken as displayed; no merge notes.

use std::collections::BTreeMap;
use std::io::Cursor;

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use uuid::Uuid;

use crate::ir::{
    BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr, ParseBackend,
    SourceLocator, TableConfidence, TableIr,
};

/// Parse an xls/xlsx workbook into a DocumentIr of SheetTable blocks.
pub fn parse_excel_document_ir(
    document_id: Uuid,
    filename: &str,
    bytes: &[u8],
) -> anyhow::Result<DocumentIr> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|e| anyhow::anyhow!("calamine failed to open {filename}: {e}"))?;

    let mut document = DocumentIr::new(
        document_id.to_string(),
        filename,
        DocumentType::Xlsx,
        ParseBackend::CalamineExcel,
    );
    document.pages.push(PageIr {
        page_number: 1,
        width: None,
        height: None,
        backend: ParseBackend::CalamineExcel,
        text_char_count: bytes.len(),
        image_count: 0,
        metadata: BTreeMap::new(),
    });

    for (sheet_index, (sheet_name, range)) in workbook.worksheets().into_iter().enumerate() {
        let Some(table) = sheet_to_table(&sheet_name, &range) else {
            continue; // fully-empty sheet → skipped
        };
        let row_count = table.rows.len() as u32;
        let mut metadata = BTreeMap::new();
        metadata.insert(
            TableIr::METADATA_KEY.to_string(),
            serde_json::to_string(&table).unwrap_or_else(|_| "{}".to_string()),
        );
        metadata.insert("table_parser".to_string(), "calamine-v1".to_string());
        metadata.insert("sheet_name".to_string(), sheet_name.clone());
        document.blocks.push(BlockIr {
            block_id: format!("{document_id}-sheet{sheet_index}"),
            page: Some(1),
            block_type: BlockType::SheetTable,
            modality: BlockModality::TextOnly,
            text: table.to_markdown(),
            alt_text: None,
            asset_refs: Vec::new(),
            caption: table.caption.clone(),
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(1),
                table_index: Some(sheet_index),
                sheet_name: Some(sheet_name),
                row_range: Some((1, row_count.max(1))),
                ..SourceLocator::default()
            },
            parser_backend: ParseBackend::CalamineExcel,
            metadata,
        });
    }

    Ok(document)
}

/// One sheet → TableIr, or None when the sheet has no non-empty row.
fn sheet_to_table(sheet_name: &str, range: &calamine::Range<Data>) -> Option<TableIr> {
    let mut grid: Vec<Vec<String>> = range
        .rows()
        .map(|row| row.iter().map(cell_text).collect::<Vec<_>>())
        .collect();

    // Trim fully-empty leading/trailing rows and trailing columns.
    let non_empty = |row: &Vec<String>| row.iter().any(|c| !c.trim().is_empty());
    let first = grid.iter().position(non_empty)?;
    let last = grid.iter().rposition(non_empty)?;
    grid = grid[first..=last].to_vec();
    let width = grid
        .iter()
        .map(|row| {
            row.iter()
                .rposition(|c| !c.trim().is_empty())
                .map(|i| i + 1)
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    if width == 0 {
        return None;
    }
    for row in &mut grid {
        row.truncate(width);
        while row.len() < width {
            row.push(String::new());
        }
    }

    let headers = grid.remove(0);
    Some(TableIr {
        caption: Some(sheet_name.to_string()),
        headers,
        rows: grid,
        parse_confidence: TableConfidence::High,
        notes: Vec::new(),
    })
}

/// Cell value as displayed text (calamine `Data`; dates kept in Excel serial
/// form via their Debug-free Display where available).
fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => format_number(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => format_number(dt.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.trim().to_string(),
        Data::Error(e) => format!("{e:?}"),
    }
}

/// Compact number rendering (no trailing `.0` for integral floats).
fn format_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Minimal in-test xlsx writer (calamine cannot write): two sheets, the
    /// second with an empty leading row. Uses the crate's existing `zip` dep.
    fn build_test_xlsx() -> Vec<u8> {
        fn sheet_xml(rows: &[&[&str]]) -> String {
            let mut xml = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
            );
            for (r, row) in rows.iter().enumerate() {
                xml.push_str(&format!("<row r=\"{}\">", r + 1));
                for (c, cell) in row.iter().enumerate() {
                    let col = (b'A' + c as u8) as char;
                    if cell.is_empty() {
                        continue;
                    }
                    xml.push_str(&format!(
                        "<c r=\"{col}{}\" t=\"inlineStr\"><is><t>{cell}</t></is></c>",
                        r + 1
                    ));
                }
                xml.push_str("</row>");
            }
            xml.push_str("</sheetData></worksheet>");
            xml
        }

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            for (name, body) in [
                (
                    "[Content_Types].xml",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#
                        .to_string(),
                ),
                (
                    "_rels/.rels",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
                        .to_string(),
                ),
                (
                    "xl/workbook.xml",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="活动清单" sheetId="1" r:id="rId1"/><sheet name="空表" sheetId="2" r:id="rId2"/></sheets></workbook>"#
                        .to_string(),
                ),
                (
                    "xl/_rels/workbook.xml.rels",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#
                        .to_string(),
                ),
                (
                    "xl/worksheets/sheet1.xml",
                    sheet_xml(&[
                        &["编号", "阶段", "活动"],
                        &["1", "概念阶段", "概念启动"],
                        &["2", "验证阶段", "系统验证"],
                        &["3", "发布阶段", "发布准备"],
                    ]),
                ),
                (
                    "xl/worksheets/sheet2.xml",
                    sheet_xml(&[]),
                ),
            ] {
                zip.start_file(name, options).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn xlsx_parses_sheets_into_sheet_table_blocks() {
        let bytes = build_test_xlsx();
        let document =
            parse_excel_document_ir(Uuid::nil(), "ipd.xlsx", &bytes).expect("xlsx parses");

        // The empty sheet is skipped; exactly one SheetTable block survives.
        assert_eq!(document.blocks.len(), 1);
        let block = &document.blocks[0];
        assert_eq!(block.block_type, BlockType::SheetTable);
        assert_eq!(block.parser_backend, ParseBackend::CalamineExcel);
        assert_eq!(
            block.source_locator.sheet_name.as_deref(),
            Some("活动清单")
        );
        assert_eq!(block.source_locator.table_index, Some(0));
        assert_eq!(block.source_locator.row_range, Some((1, 3)));

        let table = TableIr::from_block(block).expect("table_ir payload");
        assert_eq!(table.caption.as_deref(), Some("活动清单"));
        assert_eq!(table.headers, vec!["编号", "阶段", "活动"]);
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[1], vec!["2", "验证阶段", "系统验证"]);
        assert_eq!(table.parse_confidence, TableConfidence::High);
        assert_eq!(
            block.metadata.get("table_parser").map(String::as_str),
            Some("calamine-v1")
        );

        // Markdown text surface (T2 chunker arm consumes this shape).
        assert!(block.text.contains("|编号|阶段|活动|"));
        assert!(block.text.contains("|3|发布阶段|发布准备|"));
    }

    #[test]
    fn xlsx_empty_workbook_yields_no_blocks() {
        let bytes = build_test_xlsx();
        // Reuse the same archive but drop sheet1 usage: parse sheet2-only
        // workbook by parsing and filtering — here we simply verify the
        // empty-sheet skip leaves zero blocks when only 空表 exists.
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default();
            for (name, body) in [
                (
                    "[Content_Types].xml",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#
                        .to_string(),
                ),
                (
                    "_rels/.rels",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
                        .to_string(),
                ),
                (
                    "xl/workbook.xml",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="空表" sheetId="1" r:id="rId1"/></sheets></workbook>"#
                        .to_string(),
                ),
                (
                    "xl/_rels/workbook.xml.rels",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#
                        .to_string(),
                ),
                (
                    "xl/worksheets/sheet1.xml",
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData></sheetData></worksheet>"#
                        .to_string(),
                ),
            ] {
                zip.start_file(name, options).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let _ = bytes;
        let document =
            parse_excel_document_ir(Uuid::nil(), "empty.xlsx", &cursor.into_inner())
                .expect("empty workbook parses");
        assert!(document.blocks.is_empty(), "empty sheets degrade to nothing");
    }
}
