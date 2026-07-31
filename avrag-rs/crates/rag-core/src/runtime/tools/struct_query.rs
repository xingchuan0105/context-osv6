//! struct_catalog / struct_query（2026-07-31，计划：docs/plans/2026-07-31-struct-query-virtual-tables.md）
//!
//! per-doc DuckDB 表格存储的只读查询面：
//! - `struct_catalog`：列出 doc_scope 内各 doc 表格存储中的 relation（表名/表头/行数/
//!   样例行/caption/unit/confidence）。**无存储或无表 → relations 为空，ok**（「无表格」）。
//! - `struct_query`：受限 SQL（单条 SELECT、禁文件/DDL/DML 函数、标识符 ∈ catalog）在
//!   加固只读连接上执行（READ_ONLY + enable_external_access=false + lock_configuration=true，
//!   配方：Simon Willison duckdb-security；详见计划 §6.2）。
//!
//! 存储约定：`<STRUCT_STORE_DIR 或 storage/struct_store>/<doc_id>.duckdb`，
//! 由灌入 pipeline（scripts/struct_query_poc）产出；证据列 `row_ord` / `__src_line`
//! （chunk_id 映射待 2b，当前回传 src_line）。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use contracts::auth_runtime::AuthContext;
use contracts::{StructCatalogArgs, StructQueryArgs, ToolResult, ToolStatus, ToolTrace};
use serde_json::json;
use uuid::Uuid;

use crate::RagRuntime;

const MAX_SAMPLE_ROWS: usize = 3;
const MAX_RESULT_ROWS: usize = 200;
const MAX_CELL_CHARS: usize = 300;

/// SQL 禁词（词边界，大小写不敏感）：文件读写/外部访问/DDL/DML/配置。
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
    "read_csv",
    "read_json",
    "read_parquet",
    "read_text",
    "read_blob",
    "glob",
    "sqlite_scan",
    "postgres_scan",
    "mysql_scan",
    "parquet_scan",
    "csv_auto",
];

fn struct_store_dir() -> PathBuf {
    std::env::var("STRUCT_STORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("storage/struct_store"))
}

fn doc_file(dir: &Path, doc_id: Uuid) -> PathBuf {
    dir.join(format!("{doc_id}.duckdb"))
}

/// 加固只读打开（配置顺序：先关外部访问，再锁定配置防 SET 撤销）。
fn open_readonly(path: &Path) -> Result<duckdb::Connection, String> {
    let config = duckdb::Config::default()
        .access_mode(duckdb::AccessMode::ReadOnly)
        .map_err(|e| format!("config: {e}"))?;
    let con = duckdb::Connection::open_with_flags(path, config)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    con.execute_batch("SET enable_external_access=false; SET lock_configuration=true;")
        .map_err(|e| format!("harden: {e}"))?;
    Ok(con)
}

fn clip(text: String) -> String {
    if text.chars().count() <= MAX_CELL_CHARS {
        text
    } else {
        format!("{}…", text.chars().take(MAX_CELL_CHARS).collect::<String>())
    }
}

fn cell_to_string(v: duckdb::types::ValueRef) -> String {
    match v {
        duckdb::types::ValueRef::Null => String::new(),
        duckdb::types::ValueRef::Text(b) => clip(String::from_utf8_lossy(b).into_owned()),
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
        other => clip(format!("{other:?}")),
    }
}

/// 单文件的 catalog：information_schema 列表 + DESCRIBE + 样例行 + _meta。
fn catalog_for_file(
    con: &duckdb::Connection,
    doc_id: Uuid,
) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = con
        .prepare("SELECT table_name FROM information_schema.tables WHERE table_name != '_meta' ORDER BY table_name")
        .map_err(|e| e.to_string())?;
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    // _meta 可选：缺省时 caption/confidence 等为空。
    let meta: HashMap<String, serde_json::Value> = con
        .prepare("SELECT table_name, caption, unit, table_kind, confidence, notes FROM _meta")
        .and_then(|mut s| {
            s.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    json!({
                        "caption": r.get::<_, Option<String>>(1)?,
                        "unit": r.get::<_, Option<String>>(2)?,
                        "table_kind": r.get::<_, Option<String>>(3)?,
                        "confidence": r.get::<_, Option<String>>(4)?,
                        "notes": r.get::<_, Option<String>>(5)?,
                    }),
                ))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for table in tables {
        let headers: Vec<String> = con
            .prepare(&format!("DESCRIBE {}", quote_ident(&table)))
            .and_then(|mut s| {
                s.query_map([], |r| r.get::<_, String>(0))
                    .map(|q| q.collect())
            })
            .map(|v: Result<Vec<String>, _>| v.unwrap_or_default())
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c != "row_ord" && c != "__src_line")
            .collect();
        let n_rows: i64 = con
            .query_row(
                &format!("SELECT COUNT(*) FROM {}", quote_ident(&table)),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let sample_sql = format!(
            "SELECT * FROM {} LIMIT {MAX_SAMPLE_ROWS}",
            quote_ident(&table)
        );
        let sample_rows = query_rows(con, &sample_sql, MAX_SAMPLE_ROWS)
            .map(|(_, rows)| rows)
            .unwrap_or_default();
        let m = meta.get(&table).cloned().unwrap_or_else(|| json!({}));
        out.push(json!({
            "name": table,
            "doc_id": doc_id.to_string(),
            "caption": m.get("caption"),
            "unit": m.get("unit"),
            "table_kind": m.get("table_kind"),
            "confidence": m.get("confidence"),
            "headers": headers,
            "n_rows": n_rows,
            "sample_rows": sample_rows,
            "notes": m.get("notes").and_then(|v| v.as_str()).map(|s| {
                serde_json::from_str::<serde_json::Value>(s).unwrap_or_else(|_| json!(s))
            }),
        }));
    }
    Ok(out)
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// 执行查询 → (columns, rows as string arrays)。
/// 注意：duckdb-rs 的 column_count/column_name 必须在语句**执行后**调用
/// （raw_statement: “The statement was not executed yet”），故先收集行、
/// 释放游标后再读 schema（空结果集同样可得列名）。
fn query_rows(
    con: &duckdb::Connection,
    sql: &str,
    cap: usize,
) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut stmt = con.prepare(sql).map_err(|e| e.to_string())?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    {
        let mut q = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = q.next().map_err(|e| e.to_string())? {
            if rows.len() >= cap {
                break;
            }
            let n = row.as_ref().column_count();
            let mut cells = Vec::with_capacity(n);
            for i in 0..n {
                cells.push(row.get_ref(i).map(cell_to_string).unwrap_or_default());
            }
            rows.push(cells);
        }
    }
    let n_cols = stmt.column_count();
    let columns: Vec<String> = (0..n_cols)
        .map(|i| {
            stmt.column_name(i)
                .map(|s| s.to_string())
                .unwrap_or_default()
        })
        .collect();
    Ok((columns, rows))
}

/// SQL 白名单校验 + FROM/JOIN 标识符收集（词边界正则，非完整 parser；
/// 加固由连接层 READ_ONLY + enable_external_access=false 兜底）。
fn validate_sql(sql: &str) -> Result<(String, Vec<String>), (String, String)> {
    let trimmed = sql.trim().trim_end_matches(';').trim().to_string();
    if trimmed.is_empty() {
        return Err(("parse".into(), "empty sql".into()));
    }
    if trimmed.contains(';') {
        return Err((
            "forbidden".into(),
            "multi-statement (';') not allowed".into(),
        ));
    }
    // v1：子查询/派生表会绕过 FROM/JOIN 关系解析，先禁掉。
    let subquery_re = regex::Regex::new(r#"(?i)\b(from|join)\s*\("#).expect("subquery regex");
    if subquery_re.is_match(&trimmed) {
        return Err((
            "forbidden".into(),
            "subquery/derived table in FROM/JOIN not allowed in v1".into(),
        ));
    }
    let lower = trimmed.to_lowercase();
    if !(lower.starts_with("select") || lower.starts_with("with")) {
        return Err(("forbidden".into(), "only single SELECT allowed".into()));
    }
    for kw in FORBIDDEN_SQL {
        let pat = format!(r"\b{}\b", regex::escape(kw));
        if regex::RegexBuilder::new(&pat)
            .case_insensitive(true)
            .build()
            .expect("forbidden regex")
            .is_match(&trimmed)
        {
            return Err((
                "forbidden".into(),
                format!("keyword/function not allowed: {kw}"),
            ));
        }
    }
    let from_re =
        regex::Regex::new(r#"(?i)\b(?:from|join)\s+(?:"([^"]+)"|([a-zA-Z_][a-zA-Z0-9_]*))"#)
            .expect("from regex");
    let mut idents = Vec::new();
    for cap in from_re.captures_iter(&trimmed) {
        let name = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        // information_schema 等系统表同样拒绝（catalog 以 _meta 外的用户表为准）。
        idents.push(name);
    }
    Ok((trimmed, idents))
}

fn resolve_doc_uuids(raw: &[String], tool: &str) -> Result<Vec<Uuid>, ToolResult> {
    let uuids: Vec<Uuid> = raw
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();
    if uuids.is_empty() {
        return Err(super::error_result(
            tool,
            "no valid doc_ids provided".to_string(),
        ));
    }
    Ok(uuids)
}

fn ok_result(tool: &str, data: serde_json::Value, elapsed: std::time::Instant) -> ToolResult {
    ToolResult {
        tool: tool.to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Ok,
        data: Some(data),
        trace: Some(ToolTrace {
            elapsed_ms: Some(elapsed.elapsed().as_millis() as u64),
            raw_hit_count: None,
            hydrated_hit_count: None,
            degrade_reason: None,
        }),
    }
}

async fn run_blocking<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("join: {e}"))?
}

/// run_catalog 的存储层主体（纯函数，便于单测）：逐 doc 收集 relations，
/// 无存储文件的 doc 跳过（非错误）。
fn catalog_store(dir: &Path, doc_uuids: &[Uuid]) -> Result<Vec<serde_json::Value>, String> {
    let mut relations = Vec::new();
    for doc_id in doc_uuids {
        let path = doc_file(dir, *doc_id);
        if !path.exists() {
            continue; // 无表格存储：该 doc 对 catalog 无贡献（非错误）
        }
        let con = open_readonly(&path)?;
        relations.extend(catalog_for_file(&con, *doc_id)?);
    }
    Ok(relations)
}

/// run_query 的存储层主体（纯函数，便于单测）：可见性收集 → 标识符校验 →
/// 单 doc 归属 → 加固连接上执行。可修复错误以 `ok:false + error.code` 走 Ok
/// 分支（模型可读结构化 error 自纠）；系统级错误（开库失败等）走 Err。
fn query_store(dir: &Path, doc_uuids: &[Uuid], sql_arg: &str) -> Result<serde_json::Value, String> {
    let (sql, idents) = match validate_sql(sql_arg) {
        Ok(v) => v,
        Err((code, message)) => {
            return Ok(json!({ "ok": false, "error": { "code": code, "message": message } }));
        }
    };
    // 打开 scope 内有存储的文件，收集可见 relation → 所在 doc。
    let files: Vec<(Uuid, PathBuf)> = doc_uuids
        .iter()
        .map(|id| (*id, doc_file(dir, *id)))
        .filter(|(_, p)| p.exists())
        .collect();
    if files.is_empty() {
        return Ok(json!({
            "ok": false,
            "error": { "code": "no_relations", "message": "doc scope 内无表格存储（relations 为空）" }
        }));
    }
    let mut visible: HashMap<String, Uuid> = HashMap::new();
    for (doc_id, path) in &files {
        let con = open_readonly(path)?;
        let mut stmt = con
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_name != '_meta'")
            .map_err(|e| e.to_string())?;
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        for n in names {
            visible.entry(n.to_lowercase()).or_insert(*doc_id);
        }
    }
    let visible_names: HashSet<String> = visible.keys().cloned().collect();
    for ident in &idents {
        if !visible_names.contains(&ident.to_lowercase()) {
            return Ok(json!({
                "ok": false,
                "error": {
                    "code": "unknown_relation",
                    "message": format!("relation `{ident}` 不在 catalog;可见:{:?}", {
                        let mut v: Vec<_> = visible_names.iter().collect();
                        v.sort();
                        v
                    })
                }
            }));
        }
    }
    // v1：查询涉及的所有 relation 必须落在同一个 doc 文件内。
    let target_docs: HashSet<Uuid> = idents.iter().map(|i| visible[&i.to_lowercase()]).collect();
    let target_doc = if target_docs.len() == 1 {
        *target_docs.iter().next().unwrap()
    } else if idents.is_empty() {
        files[0].0 // 无 FROM（如 SELECT 1）：任一文件
    } else {
        return Ok(json!({
            "ok": false,
            "error": { "code": "cross_doc", "message": "v1 不支持跨 doc 的 relation 联合查询" }
        }));
    };
    let path = files
        .iter()
        .find(|(id, _)| *id == target_doc)
        .map(|(_, p)| p.clone())
        .ok_or_else(|| "target doc file missing".to_string())?;
    let con = open_readonly(&path)?;
    let (columns, rows) = match query_rows(&con, &sql, MAX_RESULT_ROWS + 1) {
        Ok(v) => v,
        Err(e) => {
            return Ok(json!({
                "ok": false,
                "error": { "code": "execute", "message": e }
            }));
        }
    };
    let truncated = rows.len() > MAX_RESULT_ROWS;
    let rows: Vec<_> = rows.into_iter().take(MAX_RESULT_ROWS).collect();
    let row_count = rows.len();
    let col_idx: HashMap<&str, usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();
    let evidence: Vec<serde_json::Value> = if col_idx.contains_key("row_ord") {
        rows.iter()
            .map(|r| {
                json!({
                    "doc_id": target_doc.to_string(),
                    "row_ord": col_idx.get("row_ord").and_then(|i| r.get(*i)),
                    "__src_line": col_idx.get("__src_line").and_then(|i| r.get(*i)),
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(json!({
        "ok": true,
        "columns": columns,
        "rows": rows,
        "row_count": row_count,
        "truncated": truncated,
        "engine": "duckdb",
        "doc_id": target_doc.to_string(),
        "evidence": evidence,
        "evidence_note": "__src_line 为源 markdown 行号;chunk_id 映射待灌入切块联动(2b)",
    }))
}

pub async fn run_catalog(
    _runtime: &RagRuntime,
    _auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let started = std::time::Instant::now();
    let mut normalized = args.clone();
    contracts::normalize_doc_id_alias(&mut normalized);
    let args: StructCatalogArgs = match serde_json::from_value(normalized) {
        Ok(a) => a,
        Err(e) => return super::error_result("struct_catalog", format!("invalid args: {e}")),
    };
    let doc_uuids = match resolve_doc_uuids(&args.doc_ids, "struct_catalog") {
        Ok(u) => u,
        Err(r) => return r,
    };
    let dir = struct_store_dir();
    match run_blocking(move || catalog_store(&dir, &doc_uuids)).await {
        Ok(relations) => ok_result("struct_catalog", json!({ "relations": relations }), started),
        Err(e) => super::error_result("struct_catalog", e),
    }
}

pub async fn run_query(
    _runtime: &RagRuntime,
    _auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let started = std::time::Instant::now();
    let mut normalized = args.clone();
    contracts::normalize_doc_id_alias(&mut normalized);
    let args: StructQueryArgs = match serde_json::from_value(normalized) {
        Ok(a) => a,
        Err(e) => return super::error_result("struct_query", format!("invalid args: {e}")),
    };
    if args.sql.trim().is_empty() {
        return super::error_result("struct_query", "parse: empty sql".to_string());
    }
    let doc_uuids = match resolve_doc_uuids(&args.doc_ids, "struct_query") {
        Ok(u) => u,
        Err(r) => return r,
    };
    let dir = struct_store_dir();
    match run_blocking(move || query_store(&dir, &doc_uuids, &args.sql)).await {
        // 查询层可修复错误（forbidden/unknown_relation/…）：ok=false 进 data，
        // 仍按 Ok ToolResult 回传，让模型读到结构化 error 而非 tool 失败。
        Ok(data) => ok_result("struct_query", data, started),
        Err(e) => super::error_result("struct_query", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_file(dir: &Path, doc_id: Uuid) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = doc_file(dir, doc_id);
        let con = duckdb::Connection::open(&path).unwrap();
        con.execute_batch(
            "CREATE TABLE _meta (table_name VARCHAR, caption VARCHAR, unit VARCHAR, table_kind VARCHAR, confidence VARCHAR, start_line INTEGER, n_rows INTEGER, n_cols INTEGER, status VARCHAR, checks JSON, notes JSON);
             CREATE TABLE t0 (row_ord INTEGER, __src_line INTEGER, 阶段 VARCHAR, 角色 VARCHAR);
             INSERT INTO t0 VALUES (0, 1, '概念阶段', 'LPDT'), (1, 2, '验证阶段', 'PQA'), (2, 3, '验证阶段', 'SE');
             INSERT INTO _meta VALUES ('t0', '活动表', NULL, 'detail', 'high', 1, 3, 2, 'high_candidate', '[]', '[]');",
        )
        .unwrap();
        drop(con);
        path
    }

    #[test]
    fn validate_sql_accepts_simple_select() {
        let (sql, idents) = validate_sql("SELECT COUNT(*) FROM t0").unwrap();
        assert_eq!(sql, "SELECT COUNT(*) FROM t0");
        assert_eq!(idents, vec!["t0".to_string()]);
    }

    #[test]
    fn validate_sql_rejects_multi_statement_and_forbidden() {
        assert_eq!(
            validate_sql("SELECT 1; DROP TABLE t0").unwrap_err().0,
            "forbidden"
        );
        assert_eq!(
            validate_sql("ATTACH '/etc/passwd'").unwrap_err().0,
            "forbidden"
        );
        assert_eq!(
            validate_sql("SELECT * FROM read_csv('/etc/passwd')")
                .unwrap_err()
                .0,
            "forbidden"
        );
        assert_eq!(validate_sql("DELETE FROM t0").unwrap_err().0, "forbidden");
        assert_eq!(
            validate_sql("SET enable_external_access=true")
                .unwrap_err()
                .0,
            "forbidden"
        );
        assert_eq!(
            validate_sql("SELECT * FROM (SELECT * FROM t0)")
                .unwrap_err()
                .0,
            "forbidden"
        );
        assert_eq!(
            validate_sql("PREPARE s AS SELECT * FROM t0").unwrap_err().0,
            "forbidden"
        );
        // 词边界：列名含 create_time / update_at 不误伤。
        assert!(validate_sql("SELECT create_time FROM t0").is_ok());
    }

    #[test]
    fn validate_sql_collects_quoted_and_join_idents() {
        let (_, idents) =
            validate_sql("SELECT a FROM \"活动表\" JOIN t1 ON t1.x = \"活动表\".x").unwrap();
        assert!(idents.contains(&"活动表".to_string()));
        assert!(idents.contains(&"t1".to_string()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn catalog_and_query_against_fixture_file() {
        let dir = std::env::temp_dir().join(format!("struct_store_test_{}", Uuid::new_v4()));
        let doc_id = Uuid::new_v4();
        fixture_file(&dir, doc_id);

        let relations = run_blocking({
            let dir = dir.clone();
            move || {
                let con = open_readonly(&doc_file(&dir, doc_id))?;
                catalog_for_file(&con, doc_id)
            }
        })
        .await
        .unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0]["name"], "t0");
        assert_eq!(relations[0]["n_rows"], 3);
        assert_eq!(relations[0]["headers"], json!(["阶段", "角色"]));
        assert_eq!(relations[0]["confidence"], "high");

        // 只读加固：写与外部访问被拒。
        let con = open_readonly(&doc_file(&dir, doc_id)).unwrap();
        assert!(con.execute_batch("CREATE TABLE evil (x INT)").is_err());
        assert!(
            con.execute_batch("SELECT * FROM read_csv('/etc/passwd')")
                .is_err()
        );

        // 查询路径（直接走内部闭包，与 run_query 同一代码路径的关键段）。
        let (sql, idents) =
            validate_sql("SELECT 阶段, COUNT(*) FROM t0 GROUP BY 阶段 ORDER BY 阶段").unwrap();
        assert_eq!(idents, vec!["t0".to_string()]);
        let (cols, rows) = query_rows(&con, &sql, 10).unwrap();
        assert_eq!(cols, vec!["阶段".to_string(), "count_star()".to_string()]);
        assert_eq!(rows.len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn missing_file_yields_empty_relations_not_error() {
        let dir = std::env::temp_dir().join(format!("struct_store_test_{}", Uuid::new_v4()));
        let relations = run_blocking({
            let dir = dir.clone();
            move || {
                let mut out = Vec::new();
                let path = doc_file(&dir, Uuid::new_v4());
                if path.exists() {
                    let con = open_readonly(&path)?;
                    out.extend(catalog_for_file(&con, Uuid::new_v4())?);
                }
                Ok(out)
            }
        })
        .await
        .unwrap();
        assert!(
            relations.is_empty(),
            "无表格存储 → relations 空(「无表格」路径)"
        );
    }

    /// 第二个 fixture：表名 t1，用于 cross_doc 场景（与 fixture_file 的 t0 不同名）。
    fn fixture_file_t1(dir: &Path, doc_id: Uuid) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = doc_file(dir, doc_id);
        let con = duckdb::Connection::open(&path).unwrap();
        con.execute_batch(
            "CREATE TABLE t1 (row_ord INTEGER, __src_line INTEGER, x VARCHAR);
             INSERT INTO t1 VALUES (0, 10, 'a'), (1, 11, 'b');",
        )
        .unwrap();
        drop(con);
        path
    }

    #[test]
    fn query_store_error_codes_forbidden_unknown_execute() {
        let dir = std::env::temp_dir().join(format!("struct_store_test_{}", Uuid::new_v4()));
        let doc_id = Uuid::new_v4();
        fixture_file(&dir, doc_id);

        let v = query_store(&dir, &[doc_id], "DELETE FROM t0").unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "forbidden");

        let v = query_store(&dir, &[doc_id], "SELECT * FROM t9").unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "unknown_relation");

        let v = query_store(&dir, &[doc_id], "SELECT no_such_col FROM t0").unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "execute");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn query_store_no_relations_on_empty_scope() {
        let dir = std::env::temp_dir().join(format!("struct_store_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let v = query_store(&dir, &[Uuid::new_v4()], "SELECT 1").unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "no_relations");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn query_store_cross_doc_rejected() {
        let dir = std::env::temp_dir().join(format!("struct_store_test_{}", Uuid::new_v4()));
        let doc_a = Uuid::new_v4();
        let doc_b = Uuid::new_v4();
        fixture_file(&dir, doc_a);
        fixture_file_t1(&dir, doc_b);

        let v = query_store(
            &dir,
            &[doc_a, doc_b],
            "SELECT * FROM t0 JOIN t1 ON t0.row_ord = t1.row_ord",
        )
        .unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "cross_doc");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn query_store_happy_path_returns_rows_and_evidence() {
        let dir = std::env::temp_dir().join(format!("struct_store_test_{}", Uuid::new_v4()));
        let doc_id = Uuid::new_v4();
        fixture_file(&dir, doc_id);

        let v = query_store(
            &dir,
            &[doc_id],
            "SELECT row_ord, __src_line, 阶段, 角色 FROM t0 WHERE 阶段 = '验证阶段' ORDER BY row_ord",
        )
        .unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["row_count"], 2);
        assert_eq!(v["truncated"], false);
        assert_eq!(v["evidence"][0]["doc_id"], doc_id.to_string());
        // 证据值经 cell_to_string 统一为 String。
        assert_eq!(v["evidence"][0]["row_ord"], json!("1"));
        assert_eq!(v["evidence"][0]["__src_line"], json!("2"));
        assert_eq!(v["evidence"][1]["row_ord"], json!("2"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn catalog_store_skips_missing_files() {
        let dir = std::env::temp_dir().join(format!("struct_store_test_{}", Uuid::new_v4()));
        let doc_id = Uuid::new_v4();
        fixture_file(&dir, doc_id);
        let missing = Uuid::new_v4();

        let relations = catalog_store(&dir, &[missing, doc_id]).unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0]["doc_id"], doc_id.to_string());
        assert_eq!(relations[0]["name"], "t0");

        std::fs::remove_dir_all(&dir).ok();
    }
}
