//! 内存 DuckDB 重建 + per-doc DuckDB 落库（对齐 `pipeline._rebuild_db` / `write_duckdb`；
//! `_meta` 12 列 + evidence sidecar JSON 形状与 Python 产物一致）。

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::checks::{Check, TableReport};
use crate::grid::{Grid, quote_ident, render_table_md, sanitize_headers};

/// 表级证据 chunk 记录（sidecar `json` 形状与 `pipeline.write_duckdb` 一致）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceChunk {
    pub chunk_id: String,
    pub table: String,
    pub start_line: usize,
    pub n_rows: usize,
    pub md: String,
}

/// `_meta` 一行（12 列，与 Python `CREATE TABLE _meta` 列序一致）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMeta {
    pub table_name: String,
    pub caption: Option<String>,
    pub unit: Option<String>,
    pub table_kind: Option<String>,
    pub confidence: Option<String>,
    pub start_line: usize,
    pub n_rows: usize,
    pub n_cols: usize,
    pub status: String,
    pub checks: Vec<Check>,
    pub notes: Vec<String>,
    pub evidence_chunk_id: Option<String>,
}

/// 在内存连接上重建表结构（`pipeline._rebuild_db`；每表 t{i}(row_ord, cols…)）。
/// 同一连接重复调用时先 DROP 既有表（对齐 Python 每次重建全新内存库的语义）。
pub fn rebuild_db(con: &duckdb::Connection, grids: &[Grid]) -> duckdb::Result<()> {
    for (i, g) in grids.iter().enumerate() {
        con.execute_batch(&format!("DROP TABLE IF EXISTS t{i}"))?;
        let hdr = sanitize_headers(g.header());
        let cols: Vec<String> = hdr
            .iter()
            .map(|h| format!("{} VARCHAR", quote_ident(h)))
            .collect();
        con.execute_batch(&format!(
            "CREATE TABLE t{i} (row_ord INTEGER, __src_line INTEGER, {})",
            cols.join(", ")
        ))?;
        let placeholders = vec!["?"; hdr.len() + 2].join(", ");
        let mut stmt = con.prepare(&format!("INSERT INTO t{i} VALUES ({placeholders})"))?;
        for (j, r) in g.data().iter().enumerate() {
            let mut params: Vec<duckdb::types::Value> = vec![
                duckdb::types::Value::BigInt(i64::from(j as i32)),
                duckdb::types::Value::BigInt(i64::from(r.line as i32)),
            ];
            for k in 0..hdr.len() {
                params.push(duckdb::types::Value::Text(
                    r.cells.get(k).cloned().unwrap_or_default(),
                ));
            }
            stmt.execute(duckdb::params_from_iter(params))?;
        }
    }
    Ok(())
}

/// 落库（与 `pipeline.write_duckdb` 对齐）：grids + metas → `<out_path>.duckdb`；
/// 返回 evidence chunk 列表（调用方写 sidecar）。
/// 原子写（审核建议 J1）：先写同目录临时文件再 rename——写半路失败旧库原样保留
/// （旧实现先 remove_file 再写，半路失败时旧库已失、新库残缺且 _line_map 不重建）。
pub fn write_duckdb(
    grids: &[Grid],
    metas: &[TableMeta],
    out_path: &Path,
) -> anyhow::Result<Vec<EvidenceChunk>> {
    let tmp_path = out_path.with_extension(format!("duckdb.tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp_path);
    // inner 返回即连接闭合（checkpoint），随后原子改名。
    match write_duckdb_inner(grids, metas, &tmp_path) {
        Ok(evidence) => {
            let rename_result = std::fs::rename(&tmp_path, out_path);
            let _ = std::fs::remove_file(tmp_path.with_extension("wal"));
            match rename_result {
                Ok(()) => Ok(evidence),
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp_path);
                    Err(e.into())
                }
            }
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

fn write_duckdb_inner(
    grids: &[Grid],
    metas: &[TableMeta],
    out_path: &Path,
) -> anyhow::Result<Vec<EvidenceChunk>> {
    let con = duckdb::Connection::open(out_path)?;
    con.execute_batch(
        "CREATE TABLE _meta (table_name VARCHAR, caption VARCHAR, unit VARCHAR, table_kind VARCHAR, \
         confidence VARCHAR, start_line INTEGER, n_rows INTEGER, n_cols INTEGER, status VARCHAR, \
         checks JSON, notes JSON, evidence_chunk_id VARCHAR)",
    )?;

    let mut evidence = Vec::new();
    for (idx, (g, m)) in grids.iter().zip(metas.iter()).enumerate() {
        if m.status == "quarantine" {
            continue; // quarantine/excluded 表不写入（查询侧 catalog 不可见）
        }
        let name = format!("t{idx}");
        let hdr = sanitize_headers(g.header());
        let cols: Vec<String> = hdr
            .iter()
            .map(|h| format!("{} VARCHAR", quote_ident(h)))
            .collect();
        con.execute_batch(&format!(
            "CREATE TABLE {name} (row_ord INTEGER, __src_line INTEGER, {})",
            cols.join(", ")
        ))?;
        let rows: Vec<Vec<String>> = g
            .data()
            .iter()
            .map(|r| {
                (0..hdr.len())
                    .map(|k| r.cells.get(k).cloned().unwrap_or_default())
                    .collect()
            })
            .collect();
        if !rows.is_empty() {
            let placeholders = vec!["?"; hdr.len() + 2].join(", ");
            let mut stmt = con.prepare(&format!("INSERT INTO {name} VALUES ({placeholders})"))?;
            for (j, (r, row)) in g.data().iter().zip(rows.iter()).enumerate() {
                let mut params: Vec<duckdb::types::Value> = vec![
                    duckdb::types::Value::BigInt(i64::from(j as i32)),
                    duckdb::types::Value::BigInt(i64::from(r.line as i32)),
                ];
                for c in row {
                    params.push(duckdb::types::Value::Text(c.clone()));
                }
                stmt.execute(duckdb::params_from_iter(params))?;
            }
        }
        let chunk_id = uuid::Uuid::new_v4().to_string();
        evidence.push(EvidenceChunk {
            chunk_id: chunk_id.clone(),
            table: name.clone(),
            start_line: g.start_line,
            n_rows: g.n_rows(),
            md: render_table_md(&hdr, &rows),
        });
        // FTS 索引（fts 表内值发现）：bundled duckdb 内建 fts 扩展，PRAGMA 即可。
        // 查询侧（struct_query 只读连接）用 fts_main_<table>.match_bm25(row_ord, 'x')
        // 谓词检索；中文整串不分词（与 grep 配合：grep 管子串、fts 管空格分隔 token）。
        if let Err(e) = con.execute_batch(&format!(
            "PRAGMA create_fts_index('{name}', 'row_ord', {})",
            hdr.iter()
                .map(|h| format!("'{}'", h.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ")
        )) {
            eprintln!("struct-supervision: create_fts_index({name}) failed: {e}");
        }
        con.execute(
            "INSERT INTO _meta VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            duckdb::params![
                name,
                m.caption.clone(),
                m.unit.clone(),
                m.table_kind.clone(),
                m.confidence.clone(),
                g.start_line as i64,
                g.n_rows() as i64,
                hdr.len() as i64,
                m.status.clone(),
                serde_json::to_string(&m.checks).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&m.notes).unwrap_or_else(|_| "[]".into()),
                chunk_id.clone(),
            ],
        )?;
    }
    Ok(evidence)
}

/// `_meta` 构造（supervise 兜底终态后的入库元数据；与 `supervise.supervise` 尾部对齐）。
pub fn build_metas(
    grids: &[Grid],
    reports: &BTreeMap<String, TableReport>,
    finals: &BTreeMap<String, crate::session::FinalState>,
) -> Vec<TableMeta> {
    let mut metas = Vec::new();
    for (i, g) in grids.iter().enumerate() {
        let tid = format!("t{i}");
        let r = reports.get(&tid);
        let f = finals.get(&tid);
        let mut notes = g.notes.clone();
        if let Some(add) = f.and_then(|f| f.notes_add.as_ref()) {
            notes.extend(add.iter().cloned());
        }
        metas.push(TableMeta {
            table_name: tid.clone(),
            caption: f.and_then(|f| f.caption.clone()),
            unit: f.and_then(|f| f.unit.clone()),
            table_kind: f.and_then(|f| f.table_kind.clone()),
            confidence: f.map(|f| f.confidence.clone().unwrap_or_else(|| "low".into())),
            start_line: g.start_line,
            n_rows: g.n_rows(),
            n_cols: g.header().len(),
            status: if f.map(|f| f.excluded).unwrap_or(false) {
                "quarantine".into()
            } else {
                r.map(|r| r.status.clone()).unwrap_or_else(|| "quarantine".into())
            },
            checks: r.map(|r| r.checks_full.clone()).unwrap_or_default(),
            notes,
            evidence_chunk_id: None,
        });
    }
    metas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Row;

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
    fn rebuild_db_roundtrips_rows() {
        let g = grid(&[
            ("1", &["编号", "名称"]),
            ("2", &["1", "a"]),
            ("3", &["2", "b"]),
        ]);
        let con = duckdb::Connection::open_in_memory().unwrap();
        rebuild_db(&con, &[g]).unwrap();
        let n: i64 = con
            .query_row("SELECT COUNT(*) FROM t0", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let first: String = con
            .query_row("SELECT 名称 FROM t0 WHERE row_ord = 0", [], |r| r.get(0))
            .unwrap();
        assert_eq!(first, "a");
    }

    #[test]
    fn write_duckdb_produces_meta_and_evidence() {
        let dir = std::env::temp_dir();
        let out = dir.join(format!("sup_test_{}.duckdb", uuid::Uuid::new_v4()));
        let g = grid(&[("1", &["编号", "名称"]), ("2", &["1", "a concept"])]);
        let rep = crate::checks::table_report(0, &g);
        let mut reports = BTreeMap::new();
        reports.insert("t0".to_string(), rep);
        let finals = BTreeMap::new();
        let metas = build_metas(&[g.clone()], &reports, &finals);
        let evidence = write_duckdb(&[g], &metas, &out).unwrap();
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].table, "t0");
        assert!(evidence[0].md.contains("| 编号 | 名称 |"));

        let con = duckdb::Connection::open(&out).unwrap();
        let (status, n_rows, chunk_id): (String, i64, Option<String>) = con
            .query_row("SELECT status, n_rows, evidence_chunk_id FROM _meta", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(status, "high_candidate");
        assert_eq!(n_rows, 1);
        // _meta.evidence_chunk_id 必须与 sidecar evidence 的 chunk_id 一致（证据水合依赖）
        assert_eq!(chunk_id.as_deref(), Some(evidence[0].chunk_id.as_str()));
        // FTS 索引已建：match_bm25 谓词可检索（fts 表内值发现）
        let hits: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM t0 WHERE fts_main_t0.match_bm25(row_ord, 'concept') IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
        let _ = std::fs::remove_file(&out);
    }

    /// A5 回归（监督干预成功案例）：xlsx 经 markitdown 后 sheet 标题行被吃成假表头
    /// （`华为IPD流程各阶段活动` + `Unnamed: 1..5`），真表头降为数据第 1 行。
    /// rotate_header(header_row=1, drop_columns_matching="^Unnamed") 修复后复验全过。
    #[test]
    fn a5_eaten_header_rotate_header_sql_recheck() {
        // rows[0] 假表头（sheet 标题 + Unnamed）；rows[1] 真表头；rows[2..] 数据行。
        // 末列数据区全空 → 守卫允许丢；其余 Unnamed 列数据区非空 → 守卫保留。
        let g = grid(&[
            ("3", &["华为IPD流程各阶段活动", "Unnamed: 1", "Unnamed: 2", "Unnamed: 3", "Unnamed: 4", "Unnamed: 5"]),
            ("4", &["编号", "阶段", "活动", "参与角色", "输出物", ""]),
            ("5", &["1", "概念", "市场调研与需求分析", "市场部", "市场调研报告", ""]),
            ("6", &["2", "计划", "产品开发计划制定", "PDT", "产品开发计划", ""]),
            ("7", &["3", "开发", "详细设计与实现", "研发部", "产品包", ""]),
        ]);

        // 干预前：header_suspicious 不过，status=needs_diagnosis。
        let before = crate::checks::table_report(0, &g);
        assert!(!before
            .checks_full
            .iter()
            .find(|c| c.name == "header_suspicious")
            .unwrap()
            .passed);
        assert_eq!(before.status, "needs_diagnosis");

        // 监督干预：apply_directive（内部 directives::apply + 确定性重跑 + rebuild_db）。
        let input = crate::SuperviseInput {
            doc_id: Some("a5".into()),
            source_text: String::new(),
            grids: vec![g],
        };
        let mut s = crate::session::Session::new(&input).unwrap();
        let r = s.t_apply_directive(&serde_json::json!({
            "table_id": "t0",
            "directive": {
                "action": "rotate_header",
                "header_row": 1,
                "drop_columns_matching": "^Unnamed",
            },
        }));
        assert!(r.contains("已通过"), "{r}");

        // 修复后：假表头行消失、真表头提升；全空 Unnamed 列被丢，非空列被守卫保留（6→5 列）。
        assert_eq!(
            s.grids[0].header(),
            &["编号".to_string(), "阶段".to_string(), "活动".to_string(), "参与角色".to_string(), "输出物".to_string()]
        );
        assert_eq!(s.grids[0].n_rows(), 3);

        // 复验：全部 checks 通过、status=high_candidate。
        let after = &s.reports["t0"];
        assert!(after.all_passed(), "{:?}", after.failed_checks);
        assert_eq!(after.status, "high_candidate");

        // 内存库复验：COUNT(*) 与序号自校验（row_ord 0 起 ↔ 编号 1..=3）。
        let con = duckdb::Connection::open_in_memory().unwrap();
        rebuild_db(&con, &s.grids).unwrap();
        let n: i64 = con
            .query_row("SELECT COUNT(*) FROM t0", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3);
        let seq_ok: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM t0 WHERE \"编号\" = CAST(row_ord + 1 AS VARCHAR)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(seq_ok, 3);
    }

    #[test]
    fn write_duckdb_skips_quarantine() {
        let dir = std::env::temp_dir();
        let out = dir.join(format!("sup_test_{}.duckdb", uuid::Uuid::new_v4()));
        let g = grid(&[("1", &["h"]), ("2", &["x"])]);
        let rep = crate::checks::table_report(0, &g);
        let mut reports = BTreeMap::new();
        reports.insert("t0".to_string(), rep);
        let mut finals = BTreeMap::new();
        finals.insert(
            "t0".to_string(),
            crate::session::FinalState {
                table_id: "t0".into(),
                caption: None,
                unit: None,
                column_semantics: None,
                table_kind: None,
                confidence: None,
                excluded: true,
                reason: Some("quarantine test".into()),
                notes_add: None,
            },
        );
        let metas = build_metas(&[g.clone()], &reports, &finals);
        let evidence = write_duckdb(&[g], &metas, &out).unwrap();
        assert!(evidence.is_empty());
        let con = duckdb::Connection::open(&out).unwrap();
        let n: i64 = con
            .query_row("SELECT COUNT(*) FROM _meta", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        let _ = std::fs::remove_file(&out);
    }
}
