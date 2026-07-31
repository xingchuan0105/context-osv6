//! T3 (2026-07-28, table-aware ingestion §4.2): CSV/TSV grid parsing via the
//! `csv` crate — previously flattened into plain text by TextParser.
//!
//! Mapping decisions:
//! - headers = first record when every cell is non-empty AND at least one
//!   cell is non-numeric; otherwise positional `col_1..n` with a note;
//! - the reader is RIGID (consistent record width required) — a width
//!   mismatch or any other parse error, a single-column grid, or fewer than
//!   two records → NOT a table: degrade to the plain-text path (never
//!   garbage structure).

use crate::ir::{TableConfidence, TableIr};

/// Try to parse CSV/TSV text into a TableIr. `delimiter` is b',' or b'\t'.
pub fn try_parse_csv(text: &str, delimiter: u8) -> Option<TableIr> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut grid: Vec<Vec<String>> = Vec::new();
    for record in reader.records() {
        // Any parse error → degrade to plain text.
        let record = record.ok()?;
        grid.push(record.iter().map(|c| c.trim().to_string()).collect());
    }
    while grid
        .last()
        .is_some_and(|row| row.iter().all(|c| c.is_empty()))
    {
        grid.pop();
    }
    if grid.len() < 2 {
        return None;
    }
    let width = grid.iter().map(Vec::len).max().unwrap_or(0);
    if width < 2 {
        return None;
    }
    for row in &mut grid {
        row.resize(width, String::new());
    }

    let first = &grid[0];
    let looks_like_headers =
        first.iter().all(|c| !c.is_empty()) && first.iter().any(|c| !is_numeric(c));
    let mut notes = Vec::new();
    let (headers, rows) = if looks_like_headers {
        let headers = grid.remove(0);
        (headers, grid)
    } else {
        notes.push("无表头，使用位置列名".to_string());
        ((1..=width).map(|i| format!("col_{i}")).collect(), grid)
    };
    if rows.is_empty() {
        return None;
    }

    Some(TableIr {
        caption: None,
        headers,
        rows,
        parse_confidence: TableConfidence::High,
        notes,
    })
}

fn is_numeric(cell: &str) -> bool {
    !cell.is_empty()
        && cell
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | ',' | '%'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_with_headers_parses() {
        let table =
            try_parse_csv("编号,名称,数量\n1,速冻机,10\n2,冷却塔,3\n", b',').expect("csv parses");
        assert_eq!(table.headers, vec!["编号", "名称", "数量"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["1", "速冻机", "10"]);
        assert!(table.notes.is_empty());
    }

    #[test]
    fn tsv_variant_parses() {
        let table = try_parse_csv("编号\t阶段\n1\t验证阶段\n", b'\t').expect("tsv parses");
        assert_eq!(table.headers, vec!["编号", "阶段"]);
        assert_eq!(table.rows, vec![vec!["1", "验证阶段"]]);
    }

    #[test]
    fn headerless_grid_gets_positional_columns() {
        let table = try_parse_csv("1,2,3\n4,5,6\n", b',').expect("grid parses");
        assert_eq!(table.headers, vec!["col_1", "col_2", "col_3"]);
        assert!(table.notes.iter().any(|n| n.contains("位置列名")));
    }

    #[test]
    fn quoted_commas_parse() {
        let table = try_parse_csv("名称,说明\n\"南通,四方\",\"丰富的产品线,产品组合\"\n", b',')
            .expect("quoted csv parses");
        assert_eq!(table.rows[0][0], "南通,四方");
        assert_eq!(table.rows[0][1], "丰富的产品线,产品组合");
    }

    #[test]
    fn malformed_csv_degrades_to_none() {
        // Width-mismatched records → rigid reader error → degrade.
        assert!(try_parse_csv("a,b\n1,2,3\n", b',').is_none());
        // Single column / single record → not a table.
        assert!(try_parse_csv("只有一列\n又一列\n", b',').is_none());
        assert!(try_parse_csv("a,b\n", b',').is_none());
        assert!(try_parse_csv("", b',').is_none());
    }
}
