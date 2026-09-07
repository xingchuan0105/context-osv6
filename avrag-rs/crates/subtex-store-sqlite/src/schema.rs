use super::{StoreError, EMBEDDING_DIM, SCHEMA_VERSION};
use rusqlite::{params, Connection, OptionalExtension};

pub(super) fn migrate(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
    let version: i64 = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    if version < 1 {
        conn.execute_batch(CORE_DDL)?;
    }
    set_meta(conn, "schema_version", &SCHEMA_VERSION.to_string())?;
    Ok(())
}

pub(super) fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Probe the sqlite-vec extension and create the vector table when available.
/// When unavailable the vector layer stays "off" and lexical/outline layers
/// keep working (readiness philosophy: any stage has a usable search path).
pub(super) fn vector_available(conn: &Connection) -> Result<bool, StoreError> {
    match conn.query_row("SELECT vec_version()", [], |r| r.get::<_, String>(0)) {
        Ok(_) => {
            conn.execute_batch(&format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_vec USING vec0(\
                     chunk_id TEXT PRIMARY KEY, \
                     embedding float[{EMBEDDING_DIM}]\
                 );"
            ))?;
            set_meta(conn, "vector_engine", "sqlite-vec")?;
            Ok(true)
        }
        Err(_) => {
            set_meta(conn, "vector_engine", "off")?;
            Ok(false)
        }
    }
}

const CORE_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    content_hash TEXT,
    size INTEGER,
    mtime_ms INTEGER,
    first_seen_at TEXT NOT NULL,
    indexed_at TEXT
);

CREATE TABLE IF NOT EXISTS chunks (
    rowid INTEGER PRIMARY KEY,
    chunk_id TEXT NOT NULL UNIQUE,
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    content TEXT NOT NULL,
    chunk_type TEXT NOT NULL DEFAULT 'text',
    heading_path TEXT,
    page INTEGER,
    line_start INTEGER,
    line_end INTEGER,
    embedded INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id, seq);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    content,
    content = 'chunks',
    content_rowid = 'rowid',
    tokenize = 'trigram'
);
CREATE TRIGGER IF NOT EXISTS chunks_fts_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, content) VALUES (new.rowid, new.content);
END;
CREATE TRIGGER IF NOT EXISTS chunks_fts_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
END;
CREATE TRIGGER IF NOT EXISTS chunks_fts_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
    INSERT INTO chunks_fts(rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    file_path TEXT,
    payload TEXT,
    state TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_jobs_state ON jobs(state, id);

CREATE TABLE IF NOT EXISTS usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    kind TEXT NOT NULL,
    model TEXT,
    quantity REAL NOT NULL DEFAULT 0,
    unit TEXT,
    detail TEXT
);

CREATE TABLE IF NOT EXISTS readiness (
    file_id TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    lexical INTEGER NOT NULL DEFAULT 0,
    outline INTEGER NOT NULL DEFAULT 0,
    vector INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS file_outline (
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    heading_path TEXT NOT NULL,
    PRIMARY KEY (file_id, seq)
);
"#;
