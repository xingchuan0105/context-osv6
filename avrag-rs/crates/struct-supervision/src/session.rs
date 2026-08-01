//! Session：健康报告消费 + 6 工具语义（对齐 `supervise.Session` 与各 t_* 方法；
//! LLM loop 在 S2 的 runner 中，本模块不含 LLM 调用）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::checks::TableReport;
use crate::grid::{Grid, clip};
use crate::store::rebuild_db;

/// 观察文本截断常量（与 `supervise.py` 一致）。
pub const CELL_BR: usize = 60;
pub const CELL_SLICE: usize = 200;
pub const MAX_SLICE_ROWS: usize = 40;
pub const MAX_CHECK_ROWS: usize = 50;

/// SQL 禁词（词边界，大小写不敏感）：文件读写/外部访问/DDL/DML/配置。
/// `read_*` 表函数族由下方的 FORBIDDEN_FAMILY_PATTERNS 覆盖，此处不重复列出。
/// 对齐 `struct_query.rs` 禁词全集。
const FORBIDDEN_SQL: &[&str] = &[
    "attach",
    "detach",
    "copy",
    "install",
    "load",
    "pragma",
    "set",
    "create",
    "insert",
    "update",
    "delete",
    "drop",
    "alter",
    "export",
    "import",
    "prepare",
    "execute",
    "macro",
    "glob",
    "sqlite_scan",
    "postgres_scan",
    "mysql_scan",
    "parquet_scan",
    "csv_auto",
];

/// SQL 族模式禁词（额外正则，大小写不敏感）：覆盖 read_* 表函数族
/// （read_csv / read_csv_auto / read_parquet / read_json / read_text /
/// read_blob / read_ndjson / read_npy 等），弥补词边界禁词表无法覆盖
/// `read_csv_auto` 等后续字符仍为词字符的变体。
const FORBIDDEN_FAMILY_PATTERNS: &[&str] = &[
    r"\bread_[a-z0-9_]+\b",
];

/// DuckDB 单元格 → String（与 `struct_query.rs::cell_to_string` 同款）。
fn cell_to_string(v: duckdb::types::ValueRef) -> String {
    match v {
        duckdb::types::ValueRef::Null => String::new(),
        duckdb::types::ValueRef::Text(b) => String::from_utf8_lossy(b).into_owned(),
        duckdb::types::ValueRef::Boolean(x) => x.to_string(),
        duckdb::types::ValueRef::TinyInt(x) => x.to_string(),
        duckdb::types::ValueRef::SmallInt(x) => x.to_string(),
        duckdb::types::ValueRef::Int(x) => x.to_string(),
        duckdb::types::ValueRef::BigInt(x) => x.to_string(),
        duckdb::types::ValueRef::HugeInt(x) => x.to_string(),
        duckdb::types::ValueRef::UTinyInt(x) => x.to_string(),
        duckdb::types::ValueRef::USmallInt(x) => x.to_string(),
        duckdb::types::ValueRef::UInt(x) => x.to_string(),
        duckdb::types::ValueRef::UBigInt(x) => x.to_string(),
        duckdb::types::ValueRef::Float(x) => x.to_string(),
        duckdb::types::ValueRef::Double(x) => x.to_string(),
        other => format!("{other:?}"),
    }
}

/// 单表终态（annotate / quarantine / exclude 产出；`supervise.final` 的值）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinalState {
    pub table_id: String,
    pub caption: Option<String>,
    pub unit: Option<String>,
    pub column_semantics: Option<serde_json::Value>,
    pub table_kind: Option<String>,
    pub confidence: Option<String>,
    pub excluded: bool,
    pub reason: Option<String>,
    pub notes_add: Option<Vec<String>>,
}

/// 监督会话：grids + 原文 + 健康报告 + 终态 + 内存 DuckDB。
pub struct Session {
    pub grids: Vec<Grid>,
    pub lines: Vec<String>,
    pub reports: BTreeMap<String, TableReport>,
    pub finals: BTreeMap<String, FinalState>,
    pub log: Vec<(String, serde_json::Value, String)>,
    pub con: duckdb::Connection,
}

impl Session {
    pub fn new(input: &crate::SuperviseInput) -> anyhow::Result<Self> {
        let reports: BTreeMap<String, TableReport> = input
            .grids
            .iter()
            .enumerate()
            .map(|(i, g)| (format!("t{i}"), crate::checks::table_report(i, g)))
            .collect();
        let con = duckdb::Connection::open_in_memory()?;
        rebuild_db(&con, &input.grids)?;
        Ok(Self {
            grids: input.grids.clone(),
            lines: input.source_text.lines().map(str::to_string).collect(),
            reports,
            finals: BTreeMap::new(),
            log: Vec::new(),
            con,
        })
    }

    /// 未终态的表（reports 中不在 finals 的）。
    pub fn unfinished(&self) -> Vec<String> {
        self.reports
            .keys()
            .filter(|t| !self.finals.contains_key(*t))
            .cloned()
            .collect()
    }

    /// 简报（对齐 `supervise.Session.briefing`；LLM 的首条 user 消息）。
    pub fn briefing(&self, doc_name: &str) -> String {
        let mut parts = vec![format!(
            "文档「{doc_name}」的表格提取与校验已完成。共 {} 张表。校验由 SQL 确定性执行,其数值即事实。\n",
            self.grids.len()
        )];
        for (tid, r) in &self.reports {
            let idx: usize = tid[1..].parse().unwrap_or(0);
            let g = &self.grids[idx];
            parts.push(format!(
                "---\n表 {tid} | {} 列 × {} 行 | 状态:{}",
                r.headers.len(),
                r.n_rows,
                r.status
            ));
            parts.push(format!("表头:{:?}", r.headers));
            let mut samples: Vec<&crate::grid::Row> = g.data().iter().take(2).collect();
            if r.n_rows > 2 {
                samples.push(g.data().last().unwrap());
            }
            for s in samples {
                let row: Vec<String> = s.cells.iter().map(|c| clip(c, CELL_BR)).collect();
                parts.push(format!("  采样: {}", row.join(" | ")));
            }
            if r.all_passed() {
                parts.push("校验:全部通过".to_string());
            } else {
                for c in &r.failed_checks {
                    parts.push(format!("校验失败:{} — {}", c.name, c.detail));
                }
            }
            if !g.notes.is_empty() {
                parts.push(format!("管线备注:{:?}", g.notes));
            }
        }
        parts.push(
            "---\n状态为「待诊断」的表存在至少一项失败校验。全部表给出终态(high/low/quarantine)\
             并完成语义标注后调用 done。"
                .to_string(),
        );
        parts.join("\n")
    }

    /// annotate：批量语义标注并给出终态置信度（守卫：隔离终态不被覆盖；
    /// 校验失败表 confidence=high 不生效）。
    pub fn t_annotate(&mut self, tables: &[serde_json::Value]) -> String {
        let mut out = Vec::new();
        for t in tables {
            let tid = t.get("table_id").and_then(|v| v.as_str()).unwrap_or("");
            if !self.reports.contains_key(tid) {
                out.push(format!("{tid}: 不存在,标注未记录"));
                continue;
            }
            if self.finals.get(tid).map(|f| f.excluded).unwrap_or(false) {
                out.push(format!("{tid}: 已处于隔离/排除终态,标注未生效(终态不被后续标注覆盖)"));
                continue;
            }
            let failing = !self.reports[tid].all_passed();
            if t.get("confidence").and_then(|v| v.as_str()) == Some("high") && failing {
                out.push(format!(
                    "{tid}: 校验未全部通过,confidence=high 未生效(守卫);请以 low 终态或先修复"
                ));
                continue;
            }
            self.finals.insert(
                tid.to_string(),
                FinalState {
                    table_id: tid.to_string(),
                    caption: t.get("caption").and_then(|v| v.as_str()).map(str::to_string),
                    unit: t.get("unit").and_then(|v| v.as_str()).map(str::to_string),
                    column_semantics: t.get("column_semantics").cloned(),
                    table_kind: t.get("table_kind").and_then(|v| v.as_str()).map(str::to_string),
                    confidence: t.get("confidence").and_then(|v| v.as_str()).map(str::to_string),
                    excluded: false,
                    reason: None,
                    notes_add: None,
                },
            );
            out.push(format!(
                "{tid}: 已标注 table_kind={}, confidence={}",
                t.get("table_kind").and_then(|v| v.as_str()).unwrap_or(""),
                t.get("confidence").and_then(|v| v.as_str()).unwrap_or("")
            ));
        }
        if out.is_empty() {
            "未提供 tables".to_string()
        } else {
            out.join("\n")
        }
    }

    /// fetch_slice：取表的有界切片（源行或数据行区间）。
    pub fn t_fetch_slice(&self, args: &serde_json::Value) -> String {
        let tid = args.get("table_id").and_then(|v| v.as_str()).unwrap_or("");
        if !self.reports.contains_key(tid) {
            return format!("{tid}: 不存在");
        }
        let idx: usize = tid[1..].parse().unwrap_or(0);
        let g = &self.grids[idx];
        if let Some(lines) = args.get("source_lines").and_then(|v| v.as_array()) {
            let a = lines.first().and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize;
            let b = lines
                .get(1)
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(a + MAX_SLICE_ROWS);
            let end = b.min(a + MAX_SLICE_ROWS - 1).min(self.lines.len());
            let rows: Vec<String> = (a..=end)
                .map(|i| format!("L{i}: {}", clip(&self.lines[i - 1], CELL_SLICE)))
                .collect();
            return format!(
                "源行 {a}–{end}(共 {} 行)原文切片;未覆盖部分仍处于未观察状态:\n{}",
                self.lines.len(),
                rows.join("\n")
            );
        }
        let a = args
            .get("row_range")
            .and_then(|v| v.as_array())
            .and_then(|v| v.first())
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let data = g.data();
        let end = (a + MAX_SLICE_ROWS).min(data.len());
        let rows: Vec<String> = data[a..end]
            .iter()
            .enumerate()
            .map(|(j, r)| {
                let cells: Vec<String> = r.cells.iter().map(|c| clip(c, CELL_SLICE)).collect();
                format!("row {}: {}", a + j, cells.join(" | "))
            })
            .collect();
        format!(
            "表 {tid} 第 {a}–{end} 行(共 {} 行)切片;未覆盖行仍处于未观察状态:\n{}",
            data.len(),
            rows.join("\n")
        )
    }

    /// run_check：在表存储上跑只读校验 SQL（守卫：仅 SELECT、禁文件/DDL/DML）。
    pub fn t_run_check(&self, args: &serde_json::Value) -> String {
        let sql = args
            .get("sql")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .trim_end_matches(';');
        if !sql.to_lowercase().starts_with("select") {
            return format!("校验 SQL 未通过只读守卫,未执行:{}", clip(sql, 120));
        }
        // 精确禁词（词边界，大小写不敏感）
        for kw in FORBIDDEN_SQL {
            let pat = format!(r"\b{}\b", regex::escape(kw));
            if regex::RegexBuilder::new(&pat)
                .case_insensitive(true)
                .build()
                .expect("forbidden regex")
                .is_match(sql)
            {
                return format!(
                    "校验 SQL 未通过只读守卫,未执行:{}",
                    clip(sql, 120)
                );
            }
        }
        // 族模式禁词（如 read_csv_auto 等变体逃逸词边界）
        for pat in FORBIDDEN_FAMILY_PATTERNS {
            if regex::RegexBuilder::new(pat)
                .case_insensitive(true)
                .build()
                .expect("forbidden family regex")
                .is_match(sql)
            {
                return format!(
                    "校验 SQL 未通过只读守卫,未执行:{}",
                    clip(sql, 120)
                );
            }
        }
        let mut stmt = match self.con.prepare(sql) {
            Ok(stmt) => stmt,
            Err(e) => return format!("SQL 执行失败:{e}"),
        };
        let rows = match stmt.query_map([], |row| {
            let mut cells = Vec::new();
            for i in 0..row.as_ref().column_count() {
                cells.push(cell_to_string(row.get_ref(i)?));
            }
            Ok(cells)
        }) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect::<Vec<Vec<String>>>(),
            Err(e) => return format!("SQL 执行失败:{e}"),
        };
        let trunc = rows.len() > MAX_CHECK_ROWS;
        let body: Vec<String> = rows
            .iter()
            .take(MAX_CHECK_ROWS)
            .map(|r| clip(&format!("{r:?}"), 300))
            .collect();
        format!(
            "run_check 完成,返回 {}{}:\n{}",
            rows.len().min(MAX_CHECK_ROWS),
            if trunc { "(已截断)" } else { "" },
            if body.is_empty() { "(空结果)".to_string() } else { body.join("\n") }
        )
    }

    /// apply_directive：应用修复指令并重跑复验（守卫与语义见 `directives` 模块）。
    pub fn t_apply_directive(&mut self, args: &serde_json::Value) -> String {
        let tid = args.get("table_id").and_then(|v| v.as_str()).unwrap_or("");
        let d = args.get("directive").cloned().unwrap_or(serde_json::json!({}));
        if !self.reports.contains_key(tid) {
            return format!("指令未通过校验,未被应用。表 {tid} 不存在。");
        }
        match crate::directives::apply(self, tid, &d) {
            Ok(()) => {
                let idx: usize = tid[1..].parse().unwrap_or(0);
                let g = &self.grids[idx];
                self.reports
                    .insert(tid.to_string(), crate::checks::table_report(idx, g));
                if let Err(e) = rebuild_db(&self.con, &self.grids) {
                    return format!("指令已应用,但内存库重建失败:{e}");
                }
                let r = &self.reports[tid];
                let checks = if r.all_passed() {
                    "全部通过".to_string()
                } else {
                    r.failed_checks
                        .iter()
                        .map(|c| format!("{}: {}", c.name, c.detail))
                        .collect::<Vec<_>>()
                        .join("; ")
                };
                format!(
                    "指令 {} 已通过 schema 校验与确定性守卫,应用于表 {tid};确定性重跑已完成。\n\
                     新健康报告:{} 列 × {} 行 | 状态:{}\n表头:{:?}\n校验:{checks}",
                    d.get("action").and_then(|v| v.as_str()).unwrap_or(""),
                    r.headers.len(),
                    r.n_rows,
                    r.status,
                    r.headers
                )
            }
            Err(reason) => format!(
                "指令 {} 未通过校验,未被应用。表 {tid} 状态未变。\n拒绝原因:{reason}",
                d.get("action").and_then(|v| v.as_str()).unwrap_or("")
            ),
        }
    }

    /// quarantine：隔离表（不入查询侧）。
    pub fn t_quarantine(&mut self, args: &serde_json::Value) -> String {
        let tid = args.get("table_id").and_then(|v| v.as_str()).unwrap_or("");
        if !self.reports.contains_key(tid) {
            return format!("{tid}: 不存在");
        }
        self.finals.insert(
            tid.to_string(),
            FinalState {
                table_id: tid.to_string(),
                excluded: true,
                reason: args.get("reason").and_then(|v| v.as_str()).map(str::to_string),
                ..Default::default()
            },
        );
        format!(
            "{tid}: 已隔离,原因:{}. 该表不出现在查询侧 catalog。",
            args.get("reason").and_then(|v| v.as_str()).unwrap_or("")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Grid, Row, SuperviseInput};

    fn fixture_session() -> Session {
        let text = ["| h |", "| --- |", "| a |"].join("\n");
        let grids = vec![Grid {
            start_line: 1,
            rows: vec![
                Row { line: 1, cells: vec!["h".into()] },
                Row { line: 3, cells: vec!["a".into()] },
            ],
            notes: vec![],
        }];
        let input = SuperviseInput {
            doc_id: Some("test".into()),
            source_text: text,
            grids,
        };
        Session::new(&input).expect("fixture session")
    }

    #[test]
    fn run_check_rejects_non_select() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "DROP TABLE t0"}));
        assert!(r.contains("未通过只读守卫"), "non-SELECT should be rejected: {r}");
    }

    #[test]
    fn run_check_rejects_forbidden_keyword() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM t0; ATTACH 'evil.db'"}));
        assert!(r.contains("未通过只读守卫"), "ATTACH should be rejected: {r}");
    }

    #[test]
    fn run_check_rejects_read_csv_lowercase() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM read_csv('/etc/passwd')"}));
        assert!(r.contains("未通过只读守卫"), "read_csv should be rejected: {r}");
    }

    #[test]
    fn run_check_rejects_read_csv_mixed_case_bypass() {
        // H2 回归：大小写绕过 `Read_Csv('/etc/passwd')` 必须被拦截。
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM Read_Csv('/etc/passwd')"}));
        assert!(
            r.contains("未通过只读守卫"),
            "Read_Csv (mixed case) must be rejected by case-insensitive guard: {r}"
        );
    }

    #[test]
    fn run_check_rejects_read_parquet_mixed_case() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM Read_ParquET('/tmp/x.parquet')"}));
        assert!(r.contains("未通过只读守卫"), "Read_ParquET should be rejected by family pattern: {r}");
    }

    #[test]
    fn run_check_rejects_read_text() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM read_text('/etc/hosts')"}));
        assert!(r.contains("未通过只读守卫"), "read_text should be rejected by family pattern: {r}");
    }

    #[test]
    fn run_check_rejects_export() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM t0; EXPORT DATABASE '/tmp'"}));
        assert!(r.contains("未通过只读守卫"), "EXPORT should be rejected: {r}");
    }

    #[test]
    fn run_check_rejects_glob_function() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM glob('*.duckdb')"}));
        assert!(r.contains("未通过只读守卫"), "glob() should be rejected: {r}");
    }

    #[test]
    fn run_check_allows_safe_select() {
        let s = fixture_session();
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT COUNT(*) FROM t0"}));
        assert!(!r.contains("未通过只读守卫"), "safe SELECT should pass: {r}");
    }

    #[test]
    fn run_check_allows_select_with_fts() {
        let s = fixture_session();
        // fts_main_t0 不是禁词；match_bm25 谓词应被放行
        let r = s.t_run_check(&serde_json::json!({"sql": "SELECT * FROM t0 WHERE fts_main_t0.match_bm25(row_ord, 'x') IS NOT NULL"}));
        assert!(!r.contains("未通过只读守卫"), "FTS predicate should pass: {r}");
    }
}
