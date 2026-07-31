//! 校验套件（对齐 `pipeline.checks_for` / `pipeline.table_report`；6+1 项确定性校验）。

use serde::{Deserialize, Serialize};

use crate::grid::{Grid, sanitize_headers};

pub const NUM_RE: &str = r"^[+-]?\d+(\.\d+)?$";
pub const TOTAL_LABEL_RE: &str = "合计|总计|小计";
pub const JUNK_CELL_RE: &str = r"^[-:\s]+$";
pub const PURE_NUM_RE: &str = r"^\d+$";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableReport {
    pub table_id: String,
    pub start_line: usize,
    pub headers: Vec<String>,
    pub n_rows: usize,
    pub status: String,
    /// 未通过的校验（空 == all_passed）。
    pub failed_checks: Vec<Check>,
    pub checks_full: Vec<Check>,
}

impl TableReport {
    pub fn all_passed(&self) -> bool {
        self.failed_checks.is_empty()
    }
}

fn re(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).unwrap()
}

/// 数值解析（去逗号/全角逗号后匹配 NUM_RE；与 `pipeline.to_num` 一致）。
pub fn to_num(s: &str) -> Option<f64> {
    let v = s.replace(',', "").replace('，', "").trim().to_string();
    if re(NUM_RE).is_match(&v) {
        v.parse().ok()
    } else {
        None
    }
}

fn cell(g: &Grid, i: usize, k: usize) -> &str {
    g.data()
        .get(i)
        .and_then(|r| r.cells.get(k))
        .map(String::as_str)
        .unwrap_or("")
}

/// 校验套件（6+1 项）：header_suspicious / header_numeric_banner / column_count /
/// empty_rows / empty_columns / sequence / total_reconcile。
pub fn checks_for(g: &Grid) -> Vec<Check> {
    let mut checks = Vec::new();
    let hdr = g.header();
    let data = g.data();
    let n_cols = hdr.len();

    // header_suspicious：列名 ^Unnamed 或空
    let unnamed: Vec<String> = hdr
        .iter()
        .filter(|h| re(r"^Unnamed").is_match(h) || h.is_empty())
        .cloned()
        .collect();
    checks.push(Check {
        name: "header_suspicious".into(),
        passed: unnamed.is_empty(),
        detail: if unnamed.is_empty() {
            String::new()
        } else {
            format!("列名可疑: {unnamed:?}")
        },
    });

    // header_numeric_banner：表头含纯数字列名（白药 638 案例）
    let num_hdrs: Vec<String> = hdr.iter().filter(|h| re(PURE_NUM_RE).is_match(h)).cloned().collect();
    checks.push(Check {
        name: "header_numeric_banner".into(),
        passed: num_hdrs.is_empty(),
        detail: if num_hdrs.is_empty() {
            String::new()
        } else {
            format!("表头含纯数字列名: {num_hdrs:?}(疑似 banner/数据行混入表头)")
        },
    });

    // column_count：数据行列数不符
    let ragged: Vec<usize> = data
        .iter()
        .filter(|r| r.cells.len() != n_cols)
        .map(|r| r.line)
        .collect();
    checks.push(Check {
        name: "column_count".into(),
        passed: ragged.is_empty(),
        detail: if ragged.is_empty() {
            String::new()
        } else {
            format!("{} 行列数不符, 源行 {:?}", ragged.len(), &ragged[..ragged.len().min(5)])
        },
    });

    // empty_rows：全空行
    let empty_rows: Vec<usize> = data
        .iter()
        .filter(|r| r.cells.iter().all(|c| c.is_empty()))
        .map(|r| r.line)
        .collect();
    checks.push(Check {
        name: "empty_rows".into(),
        passed: empty_rows.is_empty(),
        detail: if empty_rows.is_empty() {
            String::new()
        } else {
            format!("{} 全空行, 源行 {:?}", empty_rows.len(), &empty_rows[..empty_rows.len().min(5)])
        },
    });

    // empty_columns：数据区全空列
    let empty_cols: Vec<String> = (0..n_cols)
        .filter(|&i| !data.is_empty() && (0..data.len()).all(|j| cell(g, j, i).is_empty()))
        .map(|i| {
            if hdr[i].is_empty() {
                format!("col_{i}")
            } else {
                hdr[i].clone()
            }
        })
        .collect();
    checks.push(Check {
        name: "empty_columns".into(),
        passed: empty_cols.is_empty(),
        detail: if empty_cols.is_empty() {
            String::new()
        } else {
            format!("全空列: {empty_cols:?}")
        },
    });

    // sequence：第 0 列全数字且连续无重复
    if !data.is_empty() {
        let col0: Vec<&str> = data.iter().filter_map(|r| r.cells.first().map(String::as_str)).collect();
        let ints: Vec<i64> = col0.iter().filter_map(|c| c.parse().ok()).collect();
        if ints.len() == col0.len() && !ints.is_empty() {
            let lo = *ints.iter().min().unwrap();
            let hi = *ints.iter().max().unwrap();
            let uniq: std::collections::HashSet<i64> = ints.iter().copied().collect();
            let ok = hi - lo + 1 == ints.len() as i64 && uniq.len() == ints.len();
            checks.push(Check {
                name: "sequence".into(),
                passed: ok,
                detail: format!("序号 {lo}..{hi}, count={}", ints.len()) + if ok { "" } else { " 断号/重复" },
            });
        }
    }

    // total_reconcile：合计行 vs 明细行（仅首个合计行；数值列逐一求和比对）
    if !data.is_empty() {
        let total_idx = data.iter().position(|r| {
            r.cells
                .first()
                .map(|c| re(TOTAL_LABEL_RE).is_match(c))
                .unwrap_or(false)
        });
        if let Some(ti) = total_idx {
            let tr = &data[ti];
            let mut bad = Vec::new();
            for i in 1..n_cols {
                let tv = tr.cells.get(i).map(String::as_str).and_then(to_num);
                if tv.is_none() {
                    continue;
                }
                let s: f64 = data
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != ti)
                    .filter_map(|(_, r)| r.cells.get(i).map(String::as_str).and_then(to_num))
                    .sum();
                let tv = tv.unwrap();
                if (s - tv).abs() > tv.abs() * 0.001 + 1e-6 {
                    let col = if hdr[i].is_empty() {
                        format!("col_{i}")
                    } else {
                        hdr[i].clone()
                    };
                    bad.push(format!("{col}: sum={s} != 合计={tv}"));
                }
            }
            checks.push(Check {
                name: "total_reconcile".into(),
                passed: bad.is_empty(),
                detail: if bad.is_empty() {
                    "合计对账一致".into()
                } else {
                    bad.join("; ")
                },
            });
        }
    }
    checks
}

/// 单表健康报告（与 `pipeline.table_report` 对齐）。
pub fn table_report(idx: usize, g: &Grid) -> TableReport {
    let checks = checks_for(g);
    let all = checks.iter().all(|c| c.passed);
    TableReport {
        table_id: format!("t{idx}"),
        start_line: g.start_line,
        headers: sanitize_headers(g.header()),
        n_rows: g.n_rows(),
        status: if all { "high_candidate".into() } else { "needs_diagnosis".into() },
        failed_checks: checks.iter().filter(|c| !c.passed).cloned().collect(),
        checks_full: checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Row;

    fn g(rows: &[(&str, &[&str])]) -> Grid {
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
    fn to_num_handles_commas_and_junk() {
        assert_eq!(to_num("1,234.5"), Some(1234.5));
        assert_eq!(to_num("abc"), None);
        assert_eq!(to_num(""), None);
    }

    #[test]
    fn all_passed_table_is_high_candidate() {
        let grid = g(&[
            ("1", &["编号", "名称"]),
            ("2", &["1", "a"]),
            ("3", &["2", "b"]),
        ]);
        let rep = table_report(0, &grid);
        assert_eq!(rep.status, "high_candidate");
        assert!(rep.all_passed());
        assert_eq!(rep.headers, vec!["编号".to_string(), "名称".to_string()]);
    }

    #[test]
    fn sequence_check_flags_break() {
        let grid = g(&[
            ("1", &["序号", "名称"]),
            ("2", &["1", "a"]),
            ("3", &["3", "b"]), // 缺 2 → 断号
        ]);
        let rep = table_report(0, &grid);
        let seq = rep.checks_full.iter().find(|c| c.name == "sequence").unwrap();
        assert!(!seq.passed, "{:?}", seq.detail);
        assert_eq!(rep.status, "needs_diagnosis");
    }

    #[test]
    fn total_reconcile_catches_mismatch() {
        let grid = g(&[
            ("1", &["项目", "金额"]),
            ("2", &["a", "100"]),
            ("3", &["b", "200"]),
            ("4", &["合计", "301"]), // 应为 300
        ]);
        let rep = table_report(0, &grid);
        let tr = rep.checks_full.iter().find(|c| c.name == "total_reconcile").unwrap();
        assert!(!tr.passed, "{:?}", tr.detail);
        assert!(tr.detail.contains("sum=300"));
    }

    #[test]
    fn total_reconcile_passes_when_consistent() {
        let grid = g(&[
            ("1", &["项目", "金额"]),
            ("2", &["a", "100"]),
            ("3", &["b", "200"]),
            ("4", &["合计", "300"]),
        ]);
        let rep = table_report(0, &grid);
        let tr = rep.checks_full.iter().find(|c| c.name == "total_reconcile").unwrap();
        assert!(tr.passed, "{:?}", tr.detail);
    }

    #[test]
    fn header_suspicious_and_banner_flags() {
        let grid = g(&[
            ("1", &["Unnamed: 0", "638", "名称"]),
            ("2", &["1", "x", "a"]),
        ]);
        let rep = table_report(0, &grid);
        assert!(!rep
            .checks_full
            .iter()
            .find(|c| c.name == "header_suspicious")
            .unwrap()
            .passed);
        assert!(!rep
            .checks_full
            .iter()
            .find(|c| c.name == "header_numeric_banner")
            .unwrap()
            .passed);
    }

    #[test]
    fn empty_column_detected() {
        let grid = g(&[
            ("1", &["a", "b", "c"]),
            ("2", &["1", "", "x"]),
            ("3", &["2", "", "y"]),
        ]);
        let rep = table_report(0, &grid);
        let ec = rep.checks_full.iter().find(|c| c.name == "empty_columns").unwrap();
        assert!(!ec.passed, "{:?}", ec.detail);
        assert!(ec.detail.contains("b"), "{:?}", ec.detail);
    }
}
