//! 校验套件（对齐 `pipeline.checks_for` / `pipeline.table_report`；8+1 项确定性校验）。

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

/// 校验套件（8+1 项）：header_suspicious / header_numeric_banner / column_count /
/// empty_rows / empty_columns / dual_column_suspect / section_header_rows / sequence / total_reconcile。
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

    // dual_column_suspect：疑似双栏/兄弟面板行混入（已知提取限制；只探测上报
    // needs_diagnosis，不自动拆表）。数据行 <6 或列数 <2 不适用（不触发）。
    // 信号1 列组分离：排除全空列（empty_columns 口径）后，数据行非空列集合的列共现图
    //   恰为两个连通分量、各 ≥3 行且各 ≥2 列（≥2 列排除布局碎表「标签列 vs 其余」假分离）。
    // 信号2 面板表头行混入：数据行与表头在 ≥2 个表头非空格上同值（整行重复表头已被
    //   merge_continuations 剔除；此处捕获兄弟面板的近重复表头，如万科 t114 的
    //   「负债及股东权益/资产」行）≥2 行。
    let mut parts: Vec<String> = Vec::new();
    if data.len() >= 6 && n_cols >= 2 {
        // 排除全空列（empty_columns 口径）
        let keep: Vec<usize> = (0..n_cols)
            .filter(|&i| (0..data.len()).any(|j| !cell(g, j, i).is_empty()))
            .collect();
        // 每行非空列集合
        let sets: Vec<Vec<usize>> = (0..data.len())
            .map(|j| {
                keep.iter()
                    .copied()
                    .filter(|&i| !cell(g, j, i).is_empty())
                    .collect()
            })
            .collect();
        // 列共现连通分量（union-find，行内各列并到首列的根）
        let mut parent: Vec<usize> = (0..n_cols).collect();
        fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
            while parent[x] != x {
                x = parent[x];
            }
            x
        }
        for s in &sets {
            if s.len() > 1 {
                let r0 = find(&mut parent, s[0]);
                for &c in &s[1..] {
                    let rc = find(&mut parent, c);
                    if rc != r0 {
                        parent[rc] = r0;
                    }
                }
            }
        }
        let mut comp_cols: std::collections::HashMap<usize, Vec<usize>> = Default::default();
        let mut comp_lines: std::collections::HashMap<usize, Vec<usize>> = Default::default();
        for (j, s) in sets.iter().enumerate() {
            if s.is_empty() {
                continue; // 全空行归 empty_rows，不参与列组聚类
            }
            let root = find(&mut parent, s[0]);
            let cols = comp_cols.entry(root).or_default();
            for &c in s {
                if !cols.contains(&c) {
                    cols.push(c);
                }
            }
            comp_lines.entry(root).or_default().push(data[j].line);
        }
        if comp_cols.len() == 2 {
            let mut groups: Vec<(Vec<usize>, Vec<usize>)> = comp_cols
                .iter()
                .map(|(root, cols)| {
                    let mut cols = cols.clone();
                    cols.sort_unstable();
                    (cols, comp_lines[root].clone())
                })
                .collect();
            groups.sort_by_key(|(cols, _)| cols[0]);
            if groups
                .iter()
                .all(|(cols, lines)| lines.len() >= 3 && cols.len() >= 2)
            {
                for (tag, (cols, lines)) in ['A', 'B'].iter().zip(groups.iter()) {
                    let names: Vec<String> = cols
                        .iter()
                        .map(|&i| {
                            if hdr[i].is_empty() {
                                format!("col_{i}")
                            } else {
                                hdr[i].clone()
                            }
                        })
                        .collect();
                    parts.push(format!(
                        "列组{tag}{names:?} {}行, 代表源行 {:?}",
                        lines.len(),
                        &lines[..lines.len().min(5)]
                    ));
                }
            }
        }
        let panel: Vec<&crate::grid::Row> = data
            .iter()
            .filter(|r| {
                (0..n_cols)
                    .filter(|&i| {
                        !hdr[i].is_empty()
                            && r.cells.get(i).map(String::as_str).unwrap_or("") == hdr[i]
                    })
                    .count()
                    >= 2
            })
            .collect();
        if panel.len() >= 2 {
            let lines: Vec<usize> = panel.iter().map(|r| r.line).take(5).collect();
            let firsts: Vec<&str> = panel
                .iter()
                .take(5)
                .map(|r| r.cells.first().map(String::as_str).unwrap_or(""))
                .collect();
            parts.push(format!(
                "面板表头行 {} 行混入(疑似双栏另一面板), 源行 {:?}, 首格 {:?}",
                panel.len(),
                lines,
                firsts
            ));
        }
    }
    checks.push(Check {
        name: "dual_column_suspect".into(),
        passed: parts.is_empty(),
        detail: parts.join("; "),
    });

    // section_header_rows：孤立段标题行计数（提示信号，passed 恒 true 仅作 detail 记录，
    // 裁决交 supervision）。口径：非首数据行、首格非空、其余列全空、首格不含 合计/总计/小计。
    let sec_hits: Vec<&crate::grid::Row> = data
        .iter()
        .enumerate()
        .filter(|(idx, r)| {
            *idx > 0
                && !r.cells.first().map(String::as_str).unwrap_or("").is_empty()
                && !re(TOTAL_LABEL_RE).is_match(&r.cells[0])
                && r.cells[1..].iter().all(|c| c.is_empty())
        })
        .map(|(_, r)| r)
        .collect();
    checks.push(Check {
        name: "section_header_rows".into(),
        passed: true,
        detail: if sec_hits.is_empty() {
            String::new()
        } else {
            let lines: Vec<usize> = sec_hits.iter().map(|r| r.line).take(5).collect();
            let firsts: Vec<&str> = sec_hits
                .iter()
                .take(5)
                .map(|r| r.cells[0].as_str())
                .collect();
            format!(
                "{} 孤立段标题行(首格非空余全空), 源行 {:?}, 首格 {:?}",
                sec_hits.len(),
                lines,
                firsts
            )
        },
    });

    // sequence：第 0 列全 ASCII 数字且连续无重复
    // 对齐 pipeline.py:214-221：Python str.isdigit() 仅接受 ASCII 数字 0-9，
    // 拒绝负号与 Unicode 数字；i128 + checked_sub 防极端值 debug panic。
    if !data.is_empty() {
        let col0: Vec<&str> = data.iter().filter_map(|r| r.cells.first().map(String::as_str)).collect();
        let ints: Vec<i128> = col0
            .iter()
            .filter(|c| c.bytes().all(|b| b.is_ascii_digit()) && !c.is_empty())
            .filter_map(|c| c.parse().ok())
            .collect();
        if ints.len() == col0.len() && !ints.is_empty() {
            let lo = *ints.iter().min().unwrap();
            let hi = *ints.iter().max().unwrap();
            let uniq: std::collections::HashSet<i128> = ints.iter().copied().collect();
            let span = hi.checked_sub(lo).and_then(|d| d.checked_add(1));
            let ok = span == Some(ints.len() as i128) && uniq.len() == ints.len();
            checks.push(Check {
                name: "sequence".into(),
                passed: ok,
                detail: format!("序号 {lo}..{hi}, count={}", ints.len()) + if ok { "" } else { " 断号/重复" },
            });
        }
    }

    // total_reconcile：合计行 vs 明细行（对齐 pipeline.py:223-235 叶子行口径；
    // 排除所有首格命中 TOTAL_LABEL_RE 的合计/总计/小计行，仅对叶子行求和）。
    if !data.is_empty() {
        let total_rows: Vec<usize> = data
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.cells
                    .first()
                    .map(|c| re(TOTAL_LABEL_RE).is_match(c))
                    .unwrap_or(false)
            })
            .map(|(j, _)| j)
            .collect();
        // 仅对首个合计行做对账（对齐 Python `for _, tr in total_rows[:1]`）
        if let Some(&ti) = total_rows.first() {
            let tr = &data[ti];
            // 叶子行：所有首格不命中 TOTAL_LABEL_RE 的数据行
            let detail_idxs: std::collections::HashSet<usize> = (0..data.len())
                .filter(|j| {
                    !data[*j]
                        .cells
                        .first()
                        .map(|c| re(TOTAL_LABEL_RE).is_match(c))
                        .unwrap_or(false)
                })
                .collect();
            let mut bad = Vec::new();
            for i in 1..n_cols {
                let tv = tr.cells.get(i).map(String::as_str).and_then(to_num);
                if tv.is_none() {
                    continue;
                }
                let s: f64 = detail_idxs
                    .iter()
                    .filter_map(|&j| data[j].cells.get(i).map(String::as_str).and_then(to_num))
                    .sum();
                let tv = tv.unwrap();
                // 容差对齐 pipeline.py:232 取大语义
                if (s - tv).abs() > (tv.abs() * 0.001).max(1e-6) {
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

    #[test]
    fn dual_column_suspect_fires_on_disjoint_column_groups() {
        // 真·左右双栏：左栏行只用 {0,1,2}，右栏行只用 {3,4,5}，各 ≥3 行
        let grid = g(&[
            ("1", &["资产", "附注", "期末", "负债", "附注", "期末"]),
            ("2", &["货币资金", "1", "88", "", "", ""]),
            ("3", &["应收票据", "2", "17", "", "", ""]),
            ("4", &["存货", "3", "51", "", "", ""]),
            ("5", &["", "", "", "短期借款", "4", "15"]),
            ("6", &["", "", "", "应付账款", "5", "16"]),
            ("7", &["", "", "", "合同负债", "6", "19"]),
        ]);
        let rep = table_report(0, &grid);
        let dc = rep
            .checks_full
            .iter()
            .find(|c| c.name == "dual_column_suspect")
            .unwrap();
        assert!(!dc.passed, "{:?}", dc.detail);
        assert!(dc.detail.contains("列组A"), "{:?}", dc.detail);
        assert!(dc.detail.contains("列组B"), "{:?}", dc.detail);
        assert!(dc.detail.contains("资产"), "{:?}", dc.detail);
        assert!(dc.detail.contains("负债"), "{:?}", dc.detail);
        assert_eq!(rep.status, "needs_diagnosis");
    }

    #[test]
    fn dual_column_suspect_fires_on_panel_header_rows() {
        // 万科 t114 形态：两面板共用列布局竖向拼接，表体混入兄弟面板表头行
        // （与表头在日期列等非空格上同值，但非整行重复——整行重复已被 merge 剔除）
        let grid = g(&[
            ("6039", &["资产", "附注五", "2024年12月31日", "", "2023年12月31日", ""]),
            ("6042", &["流动资产：", "", "", "", "", ""]),
            ("6084", &["负债及股东权益", "附注五", "2024年12月31日", "", "2023年12月31日", ""]),
            ("6085", &["流动负债：", "", "", "", "", ""]),
            ("6086", &["短期借款", "23", "15,973,061,991.55", "", "1,063,561,883.10", ""]),
            ("6089", &["应付账款", "25", "160,033,042,049.19", "", "221,688,101,235.72", ""]),
            ("6135", &["资产", "附注十五", "2024年12月31日", "", "2023年12月31日", ""]),
            ("6137", &["货币资金", "1", "911,239,043.23", "", "18,397,363,742.88", ""]),
        ]);
        let rep = table_report(0, &grid);
        let dc = rep
            .checks_full
            .iter()
            .find(|c| c.name == "dual_column_suspect")
            .unwrap();
        assert!(!dc.passed, "{:?}", dc.detail);
        assert!(dc.detail.contains("面板表头行 2 行"), "{:?}", dc.detail);
        assert!(dc.detail.contains("6084"), "{:?}", dc.detail);
        assert!(dc.detail.contains("6135"), "{:?}", dc.detail);
        assert_eq!(rep.status, "needs_diagnosis");
    }

    #[test]
    fn dual_column_suspect_clean_on_normal_table() {
        // ipd 风格正常表：全部行使用同一组列，无面板表头行 → 不触发
        let grid = g(&[
            ("1", &["编号", "阶段", "活动"]),
            ("2", &["1", "概念", "x"]),
            ("3", &["2", "概念", "y"]),
            ("4", &["3", "计划", "z"]),
            ("5", &["4", "开发", "u"]),
            ("6", &["5", "验证", "v"]),
            ("7", &["6", "发布", "w"]),
        ]);
        let rep = table_report(0, &grid);
        let dc = rep
            .checks_full
            .iter()
            .find(|c| c.name == "dual_column_suspect")
            .unwrap();
        assert!(dc.passed, "{:?}", dc.detail);
        assert!(dc.detail.is_empty(), "{:?}", dc.detail);
        assert!(rep.all_passed(), "{:?}", rep.failed_checks);
        assert_eq!(rep.status, "high_candidate");
    }

    #[test]
    fn dual_column_suspect_skips_small_tables() {
        // <6 数据行的小表不触发（即使含面板表头行形态）
        let grid = g(&[
            ("1", &["资产", "附注", "期末"]),
            ("2", &["负债", "附注", "期末"]),
            ("3", &["货币资金", "1", "88"]),
            ("4", &["负债", "附注", "期末"]),
            ("5", &["存货", "2", "51"]),
        ]);
        let rep = table_report(0, &grid);
        let dc = rep
            .checks_full
            .iter()
            .find(|c| c.name == "dual_column_suspect")
            .unwrap();
        assert!(dc.passed, "{:?}", dc.detail);
    }

    #[test]
    fn section_header_rows_is_hint_only() {
        // 孤立段标题行：passed 恒 true（提示信号），detail 记录行号；首数据行与合计行不计
        let grid = g(&[
            ("1", &["项目", "金额"]),
            ("2", &["流动资产：", ""]),
            ("3", &["货币资金", "88"]),
            ("4", &["非流动资产：", ""]),
            ("5", &["存货", "51"]),
            ("6", &["合计", "139"]),
        ]);
        let rep = table_report(0, &grid);
        let sh = rep
            .checks_full
            .iter()
            .find(|c| c.name == "section_header_rows")
            .unwrap();
        assert!(sh.passed, "{:?}", sh.detail);
        assert!(sh.detail.contains("1 孤立段标题行"), "{:?}", sh.detail);
        assert!(sh.detail.contains('4'), "{:?}", sh.detail);
        assert!(!sh.detail.contains('2'), "首数据行不计: {:?}", sh.detail);
        assert!(rep.all_passed(), "提示信号不影响 status: {:?}", rep.failed_checks);
    }

    #[test]
    fn total_reconcile_excludes_subtotals() {
        // 「总计在前小计在后」版式：总计行（t1）在前，后面还有小计行（其中/西部）；
        // 旧口径只排除第一个合计行，小计仍计入求和导致误报。
        // 新口径排除所有首格命中 TOTAL_LABEL_RE 的行，仅叶子行参与求和。
        let grid = g(&[
            ("1", &["项目", "金额"]),
            ("2", &["总计", "350"]),   // 第一个合计行 = 对账目标
            ("3", &["东部", "100"]),
            ("4", &["西部", "200"]),
            ("5", &["小计", "50"]),    // 小计 — 应排除（命中 TOTAL_LABEL_RE）
            ("6", &["西部-a", "120"]),
            ("7", &["西部-b", "80"]),
        ]);
        let rep = table_report(0, &grid);
        let tr = rep
            .checks_full
            .iter()
            .find(|c| c.name == "total_reconcile")
            .unwrap();
        // 叶子行: 东部100 + 西部200 + 西部-a120 + 西部-b80 = 500 ≠ 总计350 → 应报错
        assert!(!tr.passed, "总计350 ≠ 叶子行之和500: {:?}", tr.detail);
        assert!(tr.detail.contains("sum=500"), "{:?}", tr.detail);
        assert!(tr.detail.contains("合计=350"), "{:?}", tr.detail);
    }

    #[test]
    fn total_reconcile_leaf_only_passes() {
        // 正常版式：合计在末尾，前面全是叶子行，应对账一致
        let grid = g(&[
            ("1", &["项目", "金额"]),
            ("2", &["东部", "100"]),
            ("3", &["西部", "200"]),
            ("4", &["北部", "50"]),
            ("5", &["合计", "350"]),
        ]);
        let rep = table_report(0, &grid);
        let tr = rep
            .checks_full
            .iter()
            .find(|c| c.name == "total_reconcile")
            .unwrap();
        assert!(tr.passed, "{:?}", tr.detail);
    }
}
