//! Process-wide Subtex state (F6 inbox queue + F7 credits).
//!
//! Lives at `<data>/global.db` so the inbox can span drop points and attached
//! roots. Per-root `index.db` stays disposable derived state.

use crate::store::StoreError;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use subtex_core::{millicredits_for, Confidence, Suggestion, GLOBAL_DB_FILE};

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Clone, PartialEq)]
pub struct InboxItem {
    pub id: i64,
    pub path: String,
    pub drop_point: String,
    pub target_root: Option<String>,
    pub dest_name: String,
    pub confidence: String,
    pub reason: String,
    pub rule_id: Option<String>,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveRecord {
    pub id: i64,
    pub suggestion_id: i64,
    pub src: String,
    pub dest: String,
    pub undone: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreditRow {
    pub kind: String,
    pub millicredits: i64,
    pub quantity: f64,
    pub unit: Option<String>,
}

pub struct GlobalStore {
    conn: Mutex<Connection>,
}

impl GlobalStore {
    pub fn db_path(data_dir: &Path) -> PathBuf {
        data_dir.join(GLOBAL_DB_FILE)
    }

    pub fn open(data_dir: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(data_dir)?;
        let conn = Connection::open(Self::db_path(data_dir))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
        )?;
        conn.execute_batch(DDL)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn upsert_suggestion(&self, suggestion: &Suggestion, hash: &str) -> Result<i64, StoreError> {
        let confidence = match suggestion.confidence {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        };
        let path = suggestion.path.to_string_lossy().to_string();
        let drop_point = suggestion.drop_point.to_string_lossy().to_string();
        let target = suggestion
            .target_root
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
        self.with_conn(|conn| {
            if let Some(id) = conn
                .query_row(
                    "SELECT id FROM suggestions WHERE path = ?1 AND state = 'pending'",
                    params![&path],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
            {
                conn.execute(
                    "UPDATE suggestions SET content_hash = ?1, drop_point = ?2, target_root = ?3,
                     dest_name = ?4, confidence = ?5, reason = ?6, rule_id = ?7, updated_at = ?8
                     WHERE id = ?9",
                    params![
                        hash,
                        drop_point,
                        target,
                        suggestion.dest_name,
                        confidence,
                        suggestion.reason,
                        suggestion.rule_id,
                        now_iso(),
                        id
                    ],
                )?;
                return Ok(id);
            }
            conn.execute(
                "INSERT INTO suggestions(path, content_hash, drop_point, target_root, dest_name,
                 confidence, reason, rule_id, state, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?9)",
                params![
                    path,
                    hash,
                    drop_point,
                    target,
                    suggestion.dest_name,
                    confidence,
                    suggestion.reason,
                    suggestion.rule_id,
                    now_iso()
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn pending(&self) -> Result<Vec<InboxItem>, StoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, drop_point, target_root, dest_name, confidence, reason, rule_id, state
                 FROM suggestions WHERE state = 'pending' ORDER BY id",
            )?;
            let rows = stmt.query_map([], map_item)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn latest_by_path(&self, path: &str) -> Result<Option<InboxItem>, StoreError> {
        self.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id, path, drop_point, target_root, dest_name, confidence, reason, rule_id, state
                     FROM suggestions WHERE path = ?1 ORDER BY id DESC LIMIT 1",
                    params![path],
                    map_item,
                )
                .optional()?)
        })
    }

    pub fn get(&self, id: i64) -> Result<Option<InboxItem>, StoreError> {
        self.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id, path, drop_point, target_root, dest_name, confidence, reason, rule_id, state
                     FROM suggestions WHERE id = ?1",
                    params![id],
                    map_item,
                )
                .optional()?)
        })
    }

    pub fn set_state(&self, id: i64, state: &str) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE suggestions SET state = ?1, updated_at = ?2 WHERE id = ?3",
                params![state, now_iso(), id],
            )?;
            Ok(())
        })
    }

    pub fn record_move(&self, suggestion_id: i64, src: &Path, dest: &Path) -> Result<i64, StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO moves(suggestion_id, src, dest, created_at) VALUES(?1, ?2, ?3, ?4)",
                params![
                    suggestion_id,
                    src.to_string_lossy().as_ref(),
                    dest.to_string_lossy().as_ref(),
                    now_iso()
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn latest_move(&self, suggestion_id: i64) -> Result<Option<MoveRecord>, StoreError> {
        self.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id, suggestion_id, src, dest, undone_at FROM moves
                     WHERE suggestion_id = ?1 ORDER BY id DESC LIMIT 1",
                    params![suggestion_id],
                    |r| {
                        Ok(MoveRecord {
                            id: r.get(0)?,
                            suggestion_id: r.get(1)?,
                            src: r.get(2)?,
                            dest: r.get(3)?,
                            undone: r.get::<_, Option<String>>(4)?.is_some(),
                        })
                    },
                )
                .optional()?)
        })
    }

    pub fn mark_undone(&self, move_id: i64) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE moves SET undone_at = ?1 WHERE id = ?2",
                params![now_iso(), move_id],
            )?;
            Ok(())
        })
    }

    pub fn bump_accept(&self, pattern: &str) -> Result<i64, StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO accept_stats(pattern, accepts) VALUES(?1, 1)
                 ON CONFLICT(pattern) DO UPDATE SET accepts = accepts + 1",
                params![pattern],
            )?;
            Ok(conn.query_row(
                "SELECT accepts FROM accept_stats WHERE pattern = ?1",
                params![pattern],
                |r| r.get(0),
            )?)
        })
    }

    /// Append a credit line when the kind maps to millicredits. Returns the
    /// millicredits recorded (0 = no row).
    pub fn record_credit(
        &self,
        kind: &str,
        model: Option<&str>,
        quantity: f64,
        unit: Option<&str>,
        source_root: Option<&str>,
    ) -> Result<i64, StoreError> {
        let millicredits = millicredits_for(kind, quantity, unit);
        if millicredits <= 0 {
            return Ok(0);
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO credits(ts, kind, model, quantity, unit, millicredits, source_root)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    now_iso(),
                    kind,
                    model,
                    quantity,
                    unit,
                    millicredits,
                    source_root
                ],
            )?;
            Ok(millicredits)
        })
    }

    pub fn credit_totals(&self) -> Result<Vec<CreditRow>, StoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT kind, unit, SUM(quantity), SUM(millicredits) FROM credits
                 GROUP BY kind, unit ORDER BY kind",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(CreditRow {
                    kind: r.get(0)?,
                    unit: r.get(1)?,
                    quantity: r.get(2)?,
                    millicredits: r.get(3)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let conn = self.conn.lock().expect("global store mutex poisoned");
        f(&conn)
    }
}

fn map_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<InboxItem> {
    Ok(InboxItem {
        id: r.get(0)?,
        path: r.get(1)?,
        drop_point: r.get(2)?,
        target_root: r.get(3)?,
        dest_name: r.get(4)?,
        confidence: r.get(5)?,
        reason: r.get(6)?,
        rule_id: r.get(7)?,
        state: r.get(8)?,
    })
}

const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS suggestions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    content_hash TEXT,
    drop_point TEXT NOT NULL,
    target_root TEXT,
    dest_name TEXT NOT NULL,
    confidence TEXT NOT NULL,
    reason TEXT NOT NULL,
    rule_id TEXT,
    state TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sug_state ON suggestions(state, id);

CREATE TABLE IF NOT EXISTS moves (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    suggestion_id INTEGER NOT NULL,
    src TEXT NOT NULL,
    dest TEXT NOT NULL,
    created_at TEXT NOT NULL,
    undone_at TEXT
);

CREATE TABLE IF NOT EXISTS accept_stats (
    pattern TEXT PRIMARY KEY,
    accepts INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS credits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    kind TEXT NOT NULL,
    model TEXT,
    quantity REAL NOT NULL DEFAULT 0,
    unit TEXT,
    millicredits INTEGER NOT NULL,
    source_root TEXT
);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use subtex_core::{suggest, InboxRule};
    use tempfile::TempDir;

    #[test]
    fn suggestion_upsert_is_stable_and_credits_map_usage() {
        let dir = TempDir::new().unwrap();
        let store = GlobalStore::open(dir.path()).unwrap();
        let drop = dir.path().join("drop");
        let root = dir.path().join("root");
        std::fs::create_dir_all(&drop).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        let file = drop.join("a.m4a");
        std::fs::write(&file, b"x").unwrap();
        let drop_c = std::fs::canonicalize(&drop).unwrap();
        let root_c = std::fs::canonicalize(&root).unwrap();
        let rule = InboxRule {
            id: "audio".into(),
            exts: vec!["m4a".into()],
            name_contains: None,
            target_root: root_c.display().to_string(),
        };
        let suggestion = suggest(&file, &drop_c, &[rule], &[root_c]);
        let id1 = store.upsert_suggestion(&suggestion, "h1").unwrap();
        let id2 = store.upsert_suggestion(&suggestion, "h1").unwrap();
        assert_eq!(id1, id2);
        assert_eq!(store.pending().unwrap().len(), 1);

        assert_eq!(
            store
                .record_credit("transcription", Some("qwen"), 30.0, Some("seconds"), None)
                .unwrap(),
            30_000
        );
        assert_eq!(
            store
                .record_credit("embedding", Some("bge"), 1500.0, Some("tokens"), None)
                .unwrap(),
            1500
        );
        assert_eq!(
            store
                .record_credit("rerank", None, 3.0, Some("tokens"), None)
                .unwrap(),
            0
        );
        let totals = store.credit_totals().unwrap();
        assert_eq!(totals.len(), 2);
        assert!(totals.iter().any(|r| r.kind == "transcription" && r.millicredits == 30_000));
        assert!(totals.iter().any(|r| r.kind == "embedding" && r.millicredits == 1500));
        let dumped = serde_json::to_string(&totals.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>()).unwrap();
        assert!(!dumped.contains("yuan"));
        assert!(!dumped.contains("余额"));
    }
}
