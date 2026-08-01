//! markdown 管道表提取（`pipeline.py` `extract_grids` / `merge_continuations` /
//! `auto_rotate` 的 Rust 移植——S4 表格提取阶段 Rust 化，ingestion 直接库调用）。
//!
//! 语义对齐 markdown-it-py（gfm-like）实测（2026-07-31 探针）：
//! - 表头行须含 `|`；分隔行单元格 `:?-+:?`（trim 后，≥1 个 `-`），且列数与表头一致，否则非表。
//! - 表体吸收一切非空行（含无 `|` 的行）；ragged 行：多出的截断、不足的补空串到表头列数。
//! - 单元格 trim，`\|` 反转义为 `|`；行首尾空段（前导/结尾 `|`）丢弃。
//! - 围栏代码块（``` / ~~~）与缩进代码（≥4 空格）内的管道行不成表。
//! - 行号为 0-based 源行（对齐 markdown-it `token.map[0]`）。

use crate::grid::Grid;
use crate::grid::Row;

/// 行内未转义 `|` 切分（markdown-it escapedSplit 同语义）。
fn split_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            cur.push('|');
            chars.next();
            continue;
        }
        if ch == '|' {
            cells.push(cur);
            cur = String::new();
        } else {
            cur.push(ch);
        }
    }
    cells.push(cur);
    // 前导/结尾空段（`|` 开头/结尾的行）丢弃；随后各格 trim。
    if cells.first().is_some_and(|c| c.trim().is_empty()) && cells.len() > 1 {
        cells.remove(0);
    }
    if cells.last().is_some_and(|c| c.trim().is_empty()) && cells.len() > 1 {
        cells.pop();
    }
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

/// 分隔行单元格（`:?-+:?`）。
fn is_delim_cell(c: &str) -> bool {
    let c = c.trim();
    let core = c.strip_prefix(':').unwrap_or(c);
    let core = core.strip_suffix(':').unwrap_or(core);
    !core.is_empty() && core.chars().all(|ch| ch == '-')
}

/// 围栏代码标记（``` 或 ~~~，≥3 字符，≤3 前导空格）。
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None; // ≥4 空格缩进 = 缩进代码，非围栏
    }
    let ch = t.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let n = t.chars().take_while(|c| *c == ch).count();
    if n >= 3 { Some((ch, n)) } else { None }
}

/// 无序/有序列表项行（markdown-it list 规则触发条件同款；单独的 `-`/`*`/`+`
/// 是空列表项——万科 13324 实证：孤 `-` 行终止表体并开启列表上下文）。
fn is_list_item(line: &str) -> bool {
    let t = line.trim_start();
    let mut it = t.chars();
    if let Some(c0) = it.next() {
        if matches!(c0, '-' | '*' | '+') && (it.next() == Some(' ') || t.chars().count() == 1) {
            return true;
        }
        if c0.is_ascii_digit() {
            let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            let rest = &t[digits.len()..];
            if rest.starts_with(". ") || rest.starts_with(") ") {
                return true;
            }
        }
    }
    false
}

/// 新块起始行（markdown-it 表体终止规则同款）：ATX 标题 / 块引用 / 列表项
/// （`-`*+` 或 `N.`/`N)`）/ 水平线（*** --- ___）。纯文本行不在此列
/// （markdown-it 实测：段落行不能中断表体，会被吸收）。
fn starts_new_block(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with('>') {
        return true;
    }
    // hr：去掉空白后 ≥3 个同种 - * _
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() >= 3
        && (compact.chars().all(|c| c == '-')
            || compact.chars().all(|c| c == '*')
            || compact.chars().all(|c| c == '_'))
    {
        return true;
    }
    is_list_item(line)
}

/// markdown 全文 → grids（未合并、未 rotate）。
pub fn extract_grids(md_text: &str) -> Vec<Grid> {
    let lines: Vec<&str> = md_text.lines().collect();
    let n = lines.len();
    let mut grids = Vec::new();
    let mut in_fence: Option<(char, usize)> = None;
    // 列表上下文：列表项开始后，后续非空行是列表内容（lazy continuation），
    // markdown-it 不会在列表块内触发 table 规则（万科 7949-7958 实证）。
    let mut in_list = false;
    let mut i = 0;
    while i < n {
        let line = lines[i];
        // 围栏状态机（围栏内一律跳过）
        if let Some((ch, cnt)) = fence_marker(line) {
            match in_fence {
                Some((c0, n0)) if c0 == ch && cnt >= n0 => in_fence = None,
                None => in_fence = Some((ch, cnt)),
                _ => {}
            }
            i += 1;
            continue;
        }
        if in_fence.is_some() {
            i += 1;
            continue;
        }
        if line.trim().is_empty() {
            in_list = false;
            i += 1;
            continue;
        }
        if starts_new_block(line) {
            // 列表/标题/块引用/hr 行自身不成表；**非空**列表项开启列表上下文
            // （空标记项无段落可延续，列表随即结束——万科 13324 孤 `-` 实证）。
            if is_list_item(line) && line.trim().chars().count() > 1 {
                in_list = true;
            }
            i += 1;
            continue;
        }
        if in_list {
            i += 1;
            continue;
        }
        // 缩进代码（≥4 空格）不成表
        let indent = line.len() - line.trim_start_matches(' ').len();
        // 表检测：当前行含 `|` 且下一行是列数一致的分隔行
        if indent < 4 && line.contains('|') && i + 1 < n {
            let hdr_cells = split_cells(line);
            let delim_indent = lines[i + 1].len() - lines[i + 1].trim_start_matches(' ').len();
            let delim_cells = if delim_indent < 4 {
                split_cells(lines[i + 1])
            } else {
                Vec::new()
            };
            let is_table = !hdr_cells.is_empty()
                && hdr_cells.len() == delim_cells.len()
                && delim_cells.iter().all(|c| is_delim_cell(c));
            if is_table {
                let n_cols = hdr_cells.len();
                let start_line = i;
                let mut rows = vec![Row {
                    line: i,
                    cells: hdr_cells,
                }];
                // 表体：吸收一切非空行（markdown-it gfm 同语义；ragged 截断/补空）；
                // 新块起始行（列表/标题/块引用/hr）终止表体（markdown-it terminatorRules）。
                let mut j = i + 2;
                while j < n {
                    let l = lines[j];
                    if l.trim().is_empty() || fence_marker(l).is_some() || starts_new_block(l) {
                        break;
                    }
                    let ind = l.len() - l.trim_start_matches(' ').len();
                    if ind >= 4 {
                        break;
                    }
                    let mut cells = split_cells(l);
                    cells.truncate(n_cols);
                    while cells.len() < n_cols {
                        cells.push(String::new());
                    }
                    rows.push(Row { line: j, cells });
                    j += 1;
                }
                grids.push(Grid {
                    start_line,
                    rows,
                    notes: Vec::new(),
                });
                i = j;
                continue;
            }
        }
        i += 1;
    }
    grids
}

fn is_junk_cell(c: &str) -> bool {
    // JUNK_CELL_RE = ^[-:\s]+$
    !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
}

/// 同表头签名的后续 grid 并入首见 grid（跨页续表）；数据行与表头相同者剔除
/// （页重复表头）；分隔行残迹（全 junk 格行）剔除。
pub fn merge_continuations(grids: Vec<Grid>) -> Vec<Grid> {
    use std::collections::HashMap;
    let mut merged: Vec<Grid> = Vec::new();
    let mut by_sig: HashMap<Vec<String>, usize> = HashMap::new();
    for mut g in grids {
        if g.rows.is_empty() {
            continue;
        }
        let sig = crate::grid::header_sig(g.header());
        if let Some(&tgt) = by_sig.get(&sig) {
            let extra: Vec<Row> = g.rows.drain(1..).collect();
            let start = g.start_line;
            merged[tgt].rows.extend(extra);
            merged[tgt].notes.push(format!("merged_continuation@{start}"));
        } else {
            by_sig.insert(sig, merged.len());
            merged.push(g);
        }
    }
    for g in &mut merged {
        let hdr_sig = crate::grid::header_sig(g.header());
        let first_line = g.rows[0].line;
        let before = g.rows.len();
        // 首行恒保留；数据行签名同表头者剔除（页重复表头）
        g.rows
            .retain(|r| r.line == first_line || crate::grid::header_sig(&r.cells) != hdr_sig);
        if g.rows.len() != before {
            g.notes
                .push(format!("dropped_repeated_header_x{}", before - g.rows.len()));
        }
        let before = g.rows.len();
        g.rows.retain(|r| {
            r.line == first_line
                || !r
                    .cells
                    .iter()
                    .all(|c| is_junk_cell(if c.is_empty() { "-" } else { c }))
        });
        if g.rows.len() != before {
            g.notes
                .push(format!("dropped_delimiter_artifact_x{}", before - g.rows.len()));
        }
    }
    merged
}

/// 假表头信号（列名 ^Unnamed 或全空）→ rotate_header(header_row=1)，带守卫：
/// 仅当数据第 1 行非空单元格过半才提升；Unnamed 列仅当数据区全空才丢。
pub fn auto_rotate(g: &mut Grid) {
    let hdr: Vec<String> = g.header().to_vec();
    if hdr.is_empty()
        || !hdr
            .iter()
            .any(|h| h.starts_with("Unnamed") || h.is_empty())
    {
        return;
    }
    if g.data().is_empty() {
        return;
    }
    let first = &g.data()[0].cells;
    let nonempty = first.iter().filter(|c| !c.is_empty()).count();
    if nonempty as f64 <= hdr.len() as f64 / 2.0 {
        return; // 守卫: 数据第 1 行不像真表头
    }
    let keep: Vec<usize> = (0..hdr.len())
        .filter(|&i| {
            !(hdr[i].starts_with("Unnamed")
                && g.data()
                    .iter()
                    .all(|r| r.cells.get(i).map(String::as_str).unwrap_or("").is_empty()))
        })
        .collect();
    let mut new_rows = Vec::with_capacity(g.rows.len().saturating_sub(1));
    if g.rows.len() > 1 {
        // 新表头行也按 keep 过滤（对齐 directives.rs:65-71 rotate_header 做法；
        // 否则 Unnamed 列被丢后表头比数据行宽 → column_count 全灭）。
        new_rows.push(Row {
            line: g.rows[1].line,
            cells: keep
                .iter()
                .map(|&k| g.rows[1].cells.get(k).cloned().unwrap_or_default())
                .collect(),
        });
        for r in g.rows.iter().skip(2) {
            new_rows.push(Row {
                line: r.line,
                cells: keep
                    .iter()
                    .map(|&i| r.cells.get(i).cloned().unwrap_or_default())
                    .collect(),
            });
        }
    }
    g.rows = new_rows;
    g.notes.push("auto:rotate_header(header_row=1)".into());
}

/// 提取 + 合并 + auto_rotate（`pipeline.prepare` 同语义）。
pub fn prepare(md_text: &str) -> Vec<Grid> {
    let mut grids = merge_continuations(extract_grids(md_text));
    for g in &mut grids {
        auto_rotate(g);
    }
    grids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(grid: &Grid) -> Vec<&[String]> {
        grid.rows.iter().map(|r| r.cells.as_slice()).collect()
    }

    #[test]
    fn basic_table_with_line_numbers() {
        let md = "intro\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let g = extract_grids(md);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].start_line, 2);
        assert_eq!(cells(&g[0]), vec![&["a", "b"][..], &["1", "2"][..]]);
        assert_eq!(g[0].rows[1].line, 4);
    }

    #[test]
    fn ragged_rows_truncated_and_padded() {
        // markdown-it 实测：多出截断、不足补空
        let md = "| a | b | c |\n| --- | --- | --- |\n| 1 |\n| 2 | 3 |\n| 4 | 5 | 6 | 7 |\n";
        let g = extract_grids(md);
        assert_eq!(g.len(), 1);
        assert_eq!(
            cells(&g[0]),
            vec![
                &["a", "b", "c"][..],
                &["1", "", ""][..],
                &["2", "3", ""][..],
                &["4", "5", "6"][..],
            ]
        );
    }

    #[test]
    fn body_absorbs_lines_without_pipe_until_blank() {
        // markdown-it 实测：无 `|` 的非空行也被吸收为单行格
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |\nsome text no pipe\n| 3 | 4 |\n\ntail\n";
        let g = extract_grids(md);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].rows.len(), 4);
        assert_eq!(&g[0].rows[2].cells, &["some text no pipe".to_string(), String::new()]);
    }

    #[test]
    fn escaped_pipe_and_markers_preserved() {
        let md = "| a | b |\n| --- | --- |\n| x \\| y | **bold** |\n";
        let g = extract_grids(md);
        assert_eq!(&g[0].rows[1].cells, &["x | y".to_string(), "**bold**".to_string()]);
    }

    #[test]
    fn header_delim_count_mismatch_is_not_table() {
        let md = "| a | b |\n| --- | --- | --- |\n| 1 | 2 |\n";
        assert!(extract_grids(md).is_empty());
    }

    #[test]
    fn fenced_and_indented_code_ignored() {
        let md = "```\n| not | table |\n| --- | --- |\n```\n\n    | code | block |\n    | --- | --- |\n\n| a | b |\n| - | - |\n| 1 | 2 |\n";
        let g = extract_grids(md);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].rows[0].cells, ["a", "b"]);
    }

    #[test]
    fn no_leading_pipe_table() {
        let md = "a | b\n--- | ---\n1 | 2\n";
        let g = extract_grids(md);
        assert_eq!(g.len(), 1);
        assert_eq!(cells(&g[0]), vec![&["a", "b"][..], &["1", "2"][..]]);
    }

    #[test]
    fn body_terminates_at_new_block_start() {
        // markdown-it terminatorRules：列表项中断表体；其后的纯文本行不入表（万科 t140 实证）
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |\n- 转回第二阶段\n本年计提 46,248\n";
        let g = extract_grids(md);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].rows.len(), 2); // 表头 + 1 数据行
        // 标题/块引用/hr 同样中断；无 `|` 纯文本行被吸收
        let md2 = "| a | b |\n| --- | --- |\nplain text\n## heading\n| 9 | 9 |\n";
        let g2 = extract_grids(md2);
        assert_eq!(g2[0].rows.len(), 2);
        assert_eq!(g2[0].rows[1].cells[0], "plain text");
    }

    #[test]
    fn merge_continuation_and_artifacts() {
        let md = "| h1 | h2 |\n| --- | --- |\n| 1 | 2 |\n\n| h1 | h2 |\n| --- | --- |\n| h1 | h2 |\n| --- | --- |\n| 3 | 4 |\n";
        let g = merge_continuations(extract_grids(md));
        assert_eq!(g.len(), 1);
        // 续表合并 + 页重复表头剔除 + 分隔行残迹剔除
        assert_eq!(
            cells(&g[0]),
            vec![&["h1", "h2"][..], &["1", "2"][..], &["3", "4"][..]]
        );
        assert!(g[0].notes.iter().any(|n| n.starts_with("merged_continuation@")));
        assert!(g[0].notes.iter().any(|n| n.starts_with("dropped_repeated_header")));
    }

    #[test]
    fn merge_continuation_multi_page_preserves_order_and_lines() {
        // 跨 3 页同表头签名续表：数据行按源序拼接、源行号保留、页重复表头剔除
        let md = "| h1 | h2 |\n| --- | --- |\n| 1 | a |\n\n| h1 | h2 |\n| --- | --- |\n| 2 | b |\n\n| h1 | h2 |\n| --- | --- |\n| h1 | h2 |\n| 3 | c |\n";
        let g = merge_continuations(extract_grids(md));
        assert_eq!(g.len(), 1);
        assert_eq!(
            cells(&g[0]),
            vec![
                &["h1", "h2"][..],
                &["1", "a"][..],
                &["2", "b"][..],
                &["3", "c"][..]
            ]
        );
        let lines: Vec<usize> = g[0].rows.iter().map(|r| r.line).collect();
        assert_eq!(lines, vec![0, 2, 6, 11]);
        assert_eq!(
            g[0]
                .notes
                .iter()
                .filter(|n| n.starts_with("merged_continuation@"))
                .count(),
            2
        );
        assert!(g[0]
            .notes
            .iter()
            .any(|n| n.starts_with("dropped_repeated_header")));
    }

    #[test]
    fn merge_continuation_no_false_merge_and_keeps_near_header_rows() {
        // 表头签名不同 → 不合并（防错并）；数据行仅一格与表头不同 → 保留
        // （仅剔除与表头完全同签名的行）
        let md = "| h1 | h2 |\n| --- | --- |\n| h1 | h2x |\n\n| h1 | h3 |\n| --- | --- |\n| 9 | z |\n";
        let g = merge_continuations(extract_grids(md));
        assert_eq!(g.len(), 2, "签名不同不得合并");
        assert_eq!(
            cells(&g[0]),
            vec![&["h1", "h2"][..], &["h1", "h2x"][..]],
            "近重复表头行(一格不同)应保留"
        );
        assert!(g[0]
            .notes
            .iter()
            .all(|n| !n.starts_with("merged_continuation")));
        assert_eq!(cells(&g[1]), vec![&["h1", "h3"][..], &["9", "z"][..]]);
    }

    #[test]
    fn auto_rotate_fake_header_with_guard() {
        // IPD 方言：sheet 标题行成假表头（Unnamed 列），真表头降为数据第 1 行
        let mut g = Grid {
            start_line: 0,
            rows: vec![
                Row { line: 0, cells: vec!["华为IPD流程各阶段活动".into(), "Unnamed: 1".into(), "Unnamed: 2".into()] },
                Row { line: 1, cells: vec!["编号".into(), "阶段".into(), "活动".into()] },
                Row { line: 2, cells: vec!["1".into(), "概念".into(), "x".into()] },
            ],
            notes: vec![],
        };
        auto_rotate(&mut g);
        assert_eq!(g.header(), &["编号", "阶段", "活动"]);
        assert_eq!(g.rows.len(), 2);
        assert!(g.notes.iter().any(|n| n.contains("rotate_header")));

        // 守卫：数据第 1 行非空不过半 → 不提升
        let mut g2 = Grid {
            start_line: 0,
            rows: vec![
                Row { line: 0, cells: vec!["Unnamed: 0".into(), "Unnamed: 1".into()] },
                Row { line: 1, cells: vec!["".into(), "".into()] },
                Row { line: 2, cells: vec!["1".into(), "2".into()] },
            ],
            notes: vec![],
        };
        auto_rotate(&mut g2);
        assert_eq!(g2.header(), &["Unnamed: 0", "Unnamed: 1"]);
    }

    #[test]
    fn auto_rotate_keep_filters_new_header_too() {
        // Unnamed: 1 在数据区全空 → 应被丢弃；新表头行也须按 keep 过滤，
        // 否则表头宽于数据行 → column_count 全灭。
        let mut g = Grid {
            start_line: 0,
            rows: vec![
                Row { line: 0, cells: vec!["sheet title".into(), "Unnamed: 1".into(), "Unnamed: 2".into()] },
                // 数据第 1 行 = 真表头，"Unnamed: 1" 在数据区全空，"Unnamed: 2" 非全空
                Row { line: 1, cells: vec!["编号".into(), "".into(), "金额".into()] },
                Row { line: 2, cells: vec!["1".into(), "".into(), "100".into()] },
                Row { line: 3, cells: vec!["2".into(), "".into(), "200".into()] },
            ],
            notes: vec![],
        };
        auto_rotate(&mut g);
        // Unnamed: 1 全空 → 丢弃；保留 {0,2} → 表头 ["编号", "金额"]
        assert_eq!(g.header(), &["编号".to_string(), "金额".to_string()]);
        assert_eq!(g.rows.len(), 3); // 表头 + 2 数据行
        // 每行都应是 2 列
        for r in &g.rows {
            assert_eq!(r.cells.len(), 2, "行 L{} 列数应为 2: {:?}", r.line, r.cells);
            assert!(!r.cells.iter().any(|c| c == "Unnamed"), "不应残留 Unnamed: {:?}", r.cells);
        }
    }

    #[test]
    fn auto_rotate_keep_retains_nonempty_unnamed() {
        // Unnamed 列在数据区非全空 → 保留
        let mut g = Grid {
            start_line: 0,
            rows: vec![
                Row { line: 0, cells: vec!["Unnamed: 0".into(), "Unnamed: 1".into()] },
                Row { line: 1, cells: vec!["a".into(), "b".into()] },
                Row { line: 2, cells: vec!["1".into(), "2".into()] },
            ],
            notes: vec![],
        };
        auto_rotate(&mut g);
        // 2 列都保留，表头 = 原数据第 1 行
        assert_eq!(g.header(), &["a".to_string(), "b".to_string()]);
        assert_eq!(g.rows.len(), 2);
        for r in &g.rows {
            assert_eq!(r.cells.len(), 2);
        }
    }
}
