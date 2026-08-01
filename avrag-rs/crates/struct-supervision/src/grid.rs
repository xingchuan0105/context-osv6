//! Grid/Row 类型与纯函数（对齐 `pipeline.py` 的 `Grid` dataclass 与工具函数）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub line: usize,
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    pub start_line: usize,
    pub rows: Vec<Row>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Grid {
    pub fn header(&self) -> &[String] {
        self.rows.first().map(|r| r.cells.as_slice()).unwrap_or(&[])
    }

    pub fn data(&self) -> &[Row] {
        if self.rows.len() > 1 {
            &self.rows[1..]
        } else {
            &[]
        }
    }

    pub fn n_rows(&self) -> usize {
        self.data().len()
    }
}

/// 表头签名（续表合并判定用；与 `pipeline.header_sig` 一致——trim 后的 tuple）。
pub fn header_sig(cells: &[String]) -> Vec<String> {
    cells.iter().map(|c| c.trim().to_string()).collect()
}

/// 列名净化：空名 → `col_{i}`，重名 → `name_2`/`name_3`…（与 `pipeline.sanitize_headers` 一致）。
pub fn sanitize_headers(hdr: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(hdr.len());
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, h) in hdr.iter().enumerate() {
        let base = if h.trim().is_empty() {
            format!("col_{i}")
        } else {
            h.trim().to_string()
        };
        let name = match seen.get(&base) {
            Some(n) => {
                let next = n + 1;
                seen.insert(base.clone(), next);
                format!("{base}_{next}")
            }
            None => {
                seen.insert(base.clone(), 1);
                base.clone()
            }
        };
        out.push(name);
    }
    out
}

/// DuckDB 标识符引用（与 `pipeline.quote_ident` 一致）。
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// 表格 → pipe md（证据 chunk 文本；单元格 `|` 转义、换行压平；与 `pipeline.render_table_md` 一致）。
pub fn render_table_md(headers: &[String], rows: &[Vec<String>]) -> String {
    fn esc(c: &str) -> String {
        c.replace('|', "\\|").replace('\n', " ").trim().to_string()
    }
    let mut lines = vec![
        format!("| {} |", headers.iter().map(|h| esc(h)).collect::<Vec<_>>().join(" | ")),
        format!("| {} |", headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")),
    ];
    for r in rows {
        lines.push(format!(
            "| {} |",
            r.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | ")
        ));
    }
    lines.join("\n")
}

/// 文本截断（与 `supervise.clip` 一致；按 char 截断避免切坏 CJK，截断后补「…」）。
pub fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: &[(&str, &[&str])]) -> Grid {
        Grid {
            start_line: 1,
            rows: rows
                .iter()
                .map(|(line, cells)| Row {
                    line: line.parse().unwrap(),
                    cells: cells.iter().map(|c| c.to_string()).collect(),
                })
                .collect(),
            notes: vec![],
        }
    }

    #[test]
    fn sanitize_headers_dedups_and_fills() {
        let hdr = vec!["a".to_string(), "a".to_string(), "".to_string(), "a".to_string()];
        assert_eq!(sanitize_headers(&hdr), vec!["a", "a_2", "col_2", "a_3"]);
    }

    #[test]
    fn quote_ident_escapes_double_quote() {
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_ident("编号"), "\"编号\"");
    }

    #[test]
    fn render_table_md_escapes_pipe_and_newline() {
        let headers = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec!["x|y".to_string(), "l1\nl2".to_string()]];
        let md = render_table_md(&headers, &rows);
        assert!(md.contains("| x\\|y | l1 l2 |"), "{md}");
    }

    #[test]
    fn header_sig_trims() {
        let h = vec![" 阶段 ".to_string(), "活动".to_string()];
        assert_eq!(header_sig(&h), vec!["阶段".to_string(), "活动".to_string()]);
    }

    #[test]
    fn grid_data_excludes_header() {
        let g = grid(&[("1", &["h1", "h2"]), ("2", &["a", "b"]), ("3", &["c", "d"])]);
        assert_eq!(g.header(), &["h1".to_string(), "h2".to_string()]);
        assert_eq!(g.n_rows(), 2);
        assert_eq!(g.data()[0].cells, vec!["a".to_string(), "b".to_string()]);
    }
}
