//! T1 (2026-07-28, table-aware ingestion §4.2): whitelist table extraction
//! from flat txt/md text.
//!
//! Two formats, both self-validating with honest degradation — anything that
//! fails validation stays plain prose (never garbage structure):
//!
//! 1. **Numbered-row text tables** (IPD style): row anchor = line-start
//!    number + TAB + a constrained phase-ish cell (`…阶段` / `…周期`, CJK).
//!    Anchor never relies on space counts. Cells wrap across lines: a
//!    continuation line's first TAB-segment joins the pending cell, further
//!    segments open new cells. Codes like `PAC- 170` are rejoined
//!    (`PAC-170`) per cell cleanup.
//! 2. **Markdown pipe tables** (`| a | b |` with a `|---|` separator).
//!
//! Validation (numbered-row): anchor coverage ≥ 90% of digit-leading lines
//! in the region, every row ≥ 3 cells, row numbers strictly increasing
//! (gaps → a `notes` entry marking them as source gaps, not parse
//! fragments). Pipe tables: ≥ 2 columns, ≥ 1 data row, consistent width.

use crate::ir::{TableConfidence, TableIr};

/// One ordered piece of a segmented text unit: prose (original text) or a
/// table (text = markdown serialization, `table` = structured form).
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub text: String,
    pub table: Option<TableIr>,
}

/// Segment flat text into prose / table regions (order + content preserved).
pub fn segment_text(text: &str) -> Vec<TextSegment> {
    let lines: Vec<&str> = text.lines().collect();
    let mut regions: Vec<(usize, usize, TableIr)> = Vec::new(); // [start, end) line indexes
    regions.extend(detect_numbered_tables(&lines));
    regions.extend(detect_pipe_tables(&lines));
    regions.sort_by_key(|(start, _, _)| *start);
    // Drop regions nested inside an earlier one (pipe lines inside a
    // numbered table's continuation are cell text, not a pipe table).
    let mut merged: Vec<(usize, usize, TableIr)> = Vec::new();
    for region in regions {
        if let Some(last) = merged.last()
            && region.0 < last.1
        {
            continue;
        }
        merged.push(region);
    }

    let mut segments = Vec::new();
    let mut cursor = 0usize;
    for (start, end, table) in merged {
        if start > cursor {
            push_prose(&mut segments, &lines[cursor..start]);
        }
        segments.push(TextSegment {
            text: table.to_markdown(),
            table: Some(table),
        });
        cursor = end;
    }
    push_prose(&mut segments, &lines[cursor..]);
    if segments.is_empty() {
        segments.push(TextSegment {
            text: text.to_string(),
            table: None,
        });
    }
    segments
}

/// Whole-block parse: succeeds when the text is exactly one validated table.
pub fn try_parse_block(text: &str) -> Option<TableIr> {
    let segments = segment_text(text);
    match segments.as_slice() {
        [TextSegment {
            table: Some(table), ..
        }] => Some(table.clone()),
        _ => None,
    }
}

fn push_prose(segments: &mut Vec<TextSegment>, lines: &[&str]) {
    let text = lines.join("\n");
    if text.trim().is_empty() {
        return;
    }
    segments.push(TextSegment { text, table: None });
}

// ---------------------------------------------------------------------------
// Numbered-row tables
// ---------------------------------------------------------------------------

/// Minimum fraction of digit-leading lines in a region that must match the
/// full row anchor (number + TAB + phase-ish cell).
const MIN_ANCHOR_COVERAGE: f64 = 0.9;
/// Minimum cells per data row (before padding to the header width).
const MIN_ROW_CELLS: usize = 3;

/// Row anchor: line-start digits + TAB + phase-ish second cell
/// (`…阶段` / `…周期`, 2–8 CJK chars). Returns the row number.
fn row_anchor(line: &str) -> Option<u64> {
    let trimmed = line.trim_start();
    let tab = trimmed.find('\t')?;
    let (digits, rest) = trimmed.split_at(tab);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number = digits.parse::<u64>().ok()?;
    let second_cell = rest[1..].split('\t').next().unwrap_or("").trim();
    is_phase_cell(second_cell).then_some(number)
}

fn is_phase_cell(cell: &str) -> bool {
    let chars: Vec<char> = cell.chars().collect();
    if !(2..=8).contains(&chars.len()) {
        return false;
    }
    (cell.ends_with("阶段") || cell.ends_with("周期"))
        && chars.iter().all(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
}

fn detect_numbered_tables(lines: &[&str]) -> Vec<(usize, usize, TableIr)> {
    // 1. Collect anchors; split regions where numbering restarts.
    let mut regions: Vec<(usize, Vec<(usize, u64)>)> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let Some(number) = row_anchor(line) else {
            continue;
        };
        match regions.last_mut() {
            Some((_, anchors)) if anchors.last().is_some_and(|(_, prev)| number > *prev) => {
                anchors.push((idx, number));
            }
            _ => regions.push((idx, vec![(idx, number)])),
        }
    }

    let mut out = Vec::new();
    for (first_anchor_line, anchors) in regions {
        let Some((end, table)) = parse_numbered_region(lines, first_anchor_line, &anchors) else {
            continue;
        };
        out.push((region_prose_start(lines, first_anchor_line), end, table));
    }
    out
}

/// The region's display start: include the header line and caption line
/// immediately above the first anchor when they look like table furniture.
fn region_prose_start(lines: &[&str], first_anchor_line: usize) -> usize {
    let mut start = first_anchor_line;
    if start > 0 && is_header_line(lines[start - 1]) {
        start -= 1;
        if start > 0 && is_caption_line(lines[start - 1]) {
            start -= 1;
        }
    }
    start
}

/// Tab-separated, ≥3 cells, first cell not a number → table header line.
fn is_header_line(line: &str) -> bool {
    let cells: Vec<&str> = line.split('\t').collect();
    cells.len() >= 3 && !cells[0].trim().chars().all(|c| c.is_ascii_digit())
}

/// Short single-cell line (title) — a caption candidate.
fn is_caption_line(line: &str) -> bool {
    let trimmed = line.trim_end_matches(['\t', ' ']);
    !trimmed.is_empty() && trimmed.chars().count() <= 60
}

fn parse_numbered_region(
    lines: &[&str],
    first_anchor_line: usize,
    anchors: &[(usize, u64)],
) -> Option<(usize, TableIr)> {
    // Header / caption furniture above the first anchor.
    let (headers, caption, header_width_hint) = header_furniture(lines, first_anchor_line);

    let col_count = header_width_hint
        .unwrap_or_else(|| split_cells(lines[first_anchor_line]).len().max(MIN_ROW_CELLS));
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row_numbers: Vec<u64> = Vec::new();
    let mut end = first_anchor_line;

    for (anchor_pos, (line_idx, number)) in anchors.iter().enumerate() {
        let is_last = anchor_pos + 1 == anchors.len();
        let next_anchor = anchors
            .get(anchor_pos + 1)
            .map(|(idx, _)| *idx)
            .unwrap_or(lines.len());
        let mut cells = split_cells(lines[*line_idx]);
        // Open-row model: every line up to the next anchor belongs to this
        // row. Once the row has all columns, only inter-anchor lines merge
        // into the last cell — after the LAST row completes, remaining lines
        // stay prose (a completed table must not swallow following text).
        let mut cursor = *line_idx + 1;
        while cursor < next_anchor && (!is_last || cells.len() < col_count) {
            let mut segs = split_cells(lines[cursor]);
            if !segs.is_empty() {
                let first = segs.remove(0);
                if let Some(last) = cells.last_mut() {
                    join_cell(last, &first);
                }
                cells.extend(segs);
            }
            cursor += 1;
        }
        if cells.len() < MIN_ROW_CELLS {
            return None; // validation: every row ≥ 3 cells
        }
        // Extra segments merge into the last cell.
        if cells.len() > col_count {
            let tail = cells.split_off(col_count);
            let merged = tail.join(" ");
            if let Some(last) = cells.last_mut() {
                join_cell(last, &merged);
            }
        }
        cells.resize(col_count, String::new());
        for cell in &mut cells {
            cleanup_cell(cell);
        }
        rows.push(cells);
        row_numbers.push(*number);
        end = cursor;
    }

    // Validation: anchor coverage over digit-leading lines in the region.
    let digit_lines = lines[first_anchor_line..end]
        .iter()
        .filter(|l| {
            l.trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        })
        .count();
    if digit_lines == 0 || (anchors.len() as f64 / digit_lines as f64) < MIN_ANCHOR_COVERAGE {
        return None;
    }

    let headers = match headers {
        Some(h) => h,
        None => (1..=rows[0].len()).map(|i| format!("col_{i}")).collect(),
    };
    let mut notes = Vec::new();
    if row_numbers.len() > 1 {
        let missing: Vec<u64> = (row_numbers[0]..=row_numbers[row_numbers.len() - 1])
            .filter(|n| !row_numbers.contains(n))
            .collect();
        if !missing.is_empty() {
            notes.push(format!(
                "源文档行号缺口（非解析碎片）: {}",
                missing
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    Some((
        end,
        TableIr {
            caption,
            headers,
            rows,
            parse_confidence: TableConfidence::High,
            notes,
        },
    ))
}

/// Header line + caption line above the first anchor (when present).
/// Returns (headers, caption, width hint).
fn header_furniture(
    lines: &[&str],
    first_anchor_line: usize,
) -> (Option<Vec<String>>, Option<String>, Option<usize>) {
    if first_anchor_line == 0 || !is_header_line(lines[first_anchor_line - 1]) {
        return (None, None, None);
    }
    let headers: Vec<String> = lines[first_anchor_line - 1]
        .split('\t')
        .map(|c| c.trim().to_string())
        .collect();
    let width = headers.len();
    let caption = if first_anchor_line >= 2 && is_caption_line(lines[first_anchor_line - 2]) {
        Some(
            lines[first_anchor_line - 2]
                .trim_end_matches(['\t', ' '])
                .to_string(),
        )
    } else {
        None
    };
    (Some(headers), caption, Some(width))
}

fn split_cells(line: &str) -> Vec<String> {
    line.split('\t').map(|c| c.trim().to_string()).collect()
}

/// Append a wrapped segment to a cell: no separator at a CJK boundary
/// (Chinese wraps mid-word), a single space otherwise.
fn join_cell(cell: &mut String, segment: &str) {
    let cjk_boundary = cell.chars().last().is_some_and(is_cjk_char)
        || segment.chars().next().is_some_and(is_cjk_char);
    if !cjk_boundary && !cell.is_empty() && !segment.is_empty() {
        cell.push(' ');
    }
    cell.push_str(segment);
}

fn is_cjk_char(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// Cell cleanup: rejoin wrapped codes (`PAC- 170` → `PAC-170`).
fn cleanup_cell(cell: &mut String) {
    let trimmed = cell.trim();
    if let Some(dash) = trimmed.find('-') {
        let (prefix, rest) = trimmed.split_at(dash);
        let digits = &rest[1..];
        if !prefix.is_empty()
            && prefix.chars().all(|c| c.is_ascii_uppercase())
            && !digits.is_empty()
            && digits.chars().all(|c| c.is_ascii_digit() || c.is_whitespace())
        {
            *cell = format!("{}-{}", prefix, digits.chars().filter(|c| c.is_ascii_digit()).collect::<String>());
        }
    }
}

// ---------------------------------------------------------------------------
// Markdown pipe tables
// ---------------------------------------------------------------------------

fn detect_pipe_tables(lines: &[&str]) -> Vec<(usize, usize, TableIr)> {
    let mut out = Vec::new();
    let mut idx = 0usize;
    while idx + 1 < lines.len() {
        if is_pipe_line(lines[idx]) && is_pipe_separator(lines[idx + 1]) {
            let start = idx;
            let headers = pipe_cells(lines[idx]);
            let mut rows: Vec<Vec<String>> = Vec::new();
            idx += 2;
            while idx < lines.len() && is_pipe_line(lines[idx]) && !is_pipe_separator(lines[idx])
            {
                let mut cells = pipe_cells(lines[idx]);
                cells.resize(headers.len(), String::new());
                cells.truncate(headers.len());
                rows.push(cells);
                idx += 1;
            }
            // Validation: ≥2 columns, ≥1 data row.
            if headers.len() >= 2 && !rows.is_empty() {
                let caption = if start > 0 && is_caption_line(lines[start - 1]) {
                    Some(lines[start - 1].trim().to_string())
                } else {
                    None
                };
                let region_start = if caption.is_some() { start - 1 } else { start };
                out.push((
                    region_start,
                    idx,
                    TableIr {
                        caption,
                        headers,
                        rows,
                        parse_confidence: TableConfidence::High,
                        notes: Vec::new(),
                    },
                ));
            }
        } else {
            idx += 1;
        }
    }
    out
}

fn is_pipe_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.matches('|').count() >= 2
}

fn is_pipe_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|')
        && trimmed
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
        && trimmed.contains('-')
}

fn pipe_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim().trim_matches('|');
    trimmed.split('|').map(|c| c.trim().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipd_fixture() -> String {
        std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../app/tests/product_e2e/fixtures/huawei_ipd_370_activities.txt"
        ))
        .expect("IPD fixture readable")
    }

    #[test]
    fn ipd_fixture_parses_370_rows_with_phase_counts() {
        let table = try_parse_block(&ipd_fixture()).expect("IPD table parses");
        assert_eq!(table.rows.len(), 370, "total six-phase rows");
        assert_eq!(
            table.headers,
            vec!["编号", "阶段", "活动", "活动号", "活动描述", "角色"]
        );
        assert_eq!(
            table.caption.as_deref(),
            Some("华为IPD流程各阶段活动详解")
        );

        let phase_count = |phase: &str| {
            table
                .rows
                .iter()
                .filter(|r| r[1] == phase)
                .count()
        };
        assert_eq!(phase_count("概念阶段"), 81);
        assert_eq!(phase_count("计划阶段"), 86);
        assert_eq!(phase_count("开发阶段"), 92);
        assert_eq!(phase_count("验证阶段"), 59);
        assert_eq!(phase_count("发布阶段"), 30);
        assert_eq!(phase_count("生命周期"), 22);

        // Row 309 exists with its full grid intact.
        let row309 = table
            .rows
            .iter()
            .find(|r| r[0] == "309")
            .expect("row 309 exists");
        assert_eq!(row309[1], "验证阶段");
        assert_eq!(row309[2], "准备可获得性决策评审材料");

        // Wrapped codes are rejoined.
        assert!(
            table.rows.iter().any(|r| r[3] == "PAC-170"),
            "PAC-170 rejoined (no internal space)"
        );
        assert!(
            !table.rows.iter().any(|r| r[3].contains("PAC- 170")),
            "no spaced code survives"
        );

        // Row numbers strictly increasing, no gaps → no gap note.
        let numbers: Vec<u64> = table
            .rows
            .iter()
            .map(|r| r[0].parse::<u64>().unwrap())
            .collect();
        assert!(numbers.windows(2).all(|w| w[1] > w[0]));
        assert!(table.notes.is_empty(), "{:?}", table.notes);
    }

    #[test]
    fn numbered_table_segmentation_keeps_surrounding_prose() {
        let text = "引言段落，不含表格。\n\n华为IPD流程各阶段活动详解\t\t\t\n编号\t阶段\t活动\t活动号\t活动描述\t角色\n1\t概念阶段\t概念启动\tPAC-05\t根据规划分析可行性。\tPAC\n2\t概念阶段\t组建PDT\tPAC-10\t选择落实成员。\tPAC\n\n结尾段落。";
        let segments = segment_text(text);
        assert_eq!(segments.len(), 3, "{segments:?}");
        assert!(segments[0].table.is_none());
        assert!(segments[0].text.contains("引言段落"));
        let table = segments[1].table.as_ref().expect("middle is table");
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[1][3], "PAC-10");
        assert!(segments[2].table.is_none());
        assert!(segments[2].text.contains("结尾段落"));
    }

    #[test]
    fn numbered_table_gap_gets_source_note() {
        let text = "编号\t阶段\t活动\t活动号\t活动描述\t角色\n1\t概念阶段\ta\tX-1\td1\tR\n3\t概念阶段\tb\tX-2\td2\tR\n4\t概念阶段\tc\tX-3\td3\tR";
        let table = try_parse_block(text).expect("parses with gap note");
        assert_eq!(table.rows.len(), 3);
        assert!(
            table.notes.iter().any(|n| n.contains("源文档行号缺口") && n.contains('2')),
            "{:?}",
            table.notes
        );
    }

    #[test]
    fn irregular_prose_with_numbers_degrades() {
        // Prose mentioning numbers: no TAB structure → not a table.
        assert!(try_parse_block("公司2023年销售额增长5%，员工约200人。").is_none());
        // Numbered list without tabs → not a table.
        assert!(try_parse_block("1. 第一条\n2. 第二条\n3. 第三条").is_none());
        // Tab-separated but no phase vocabulary → not a table.
        assert!(try_parse_block("1\t苹果\t3\n2\t香蕉\t4").is_none());
        // Too few cells per row → validation fails → None.
        assert!(try_parse_block("1\t概念阶段\n2\t计划阶段\n3\t开发阶段").is_none());
    }

    #[test]
    fn markdown_pipe_table_parses() {
        let text = "表 1 库存清单\n| 名称 | 数量 |\n|---|---|\n| 速冻机 | 10 |\n| 冷却塔 | 3 |\n\n后续段落。";
        let segments = segment_text(text);
        assert_eq!(segments.len(), 2, "{segments:?}");
        let table = segments[0].table.as_ref().expect("pipe table");
        assert_eq!(table.headers, vec!["名称", "数量"]);
        assert_eq!(table.rows, vec![vec!["速冻机", "10"], vec!["冷却塔", "3"]]);
        // The line directly above a pipe table is lifted as its caption.
        assert_eq!(table.caption.as_deref(), Some("表 1 库存清单"));
        assert!(segments[1].text.contains("后续段落"));
    }

    #[test]
    fn pipe_table_requires_separator_and_rows() {
        assert!(try_parse_block("| a | b |\n| 1 | 2 |").is_none());
        assert!(try_parse_block("| a | b |\n|---|---|").is_none());
    }
}
