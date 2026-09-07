use crate::schema;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use subtex_core::RootHandle;
use thiserror::Error;

pub const SCHEMA_VERSION: i64 = 1;
/// Cloud embeddings in M1 (DashScope / SiliconFlow) are 1024-dimensional.
pub const EMBEDDING_DIM: usize = 1024;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobRecord {
    pub id: i64,
    pub kind: String,
    pub file_path: Option<String>,
    pub payload: Option<Value>,
    pub state: String,
    pub attempts: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadinessSummary {
    pub files: i64,
    pub lexical_ready: i64,
    pub outline_ready: i64,
    pub vector_ready: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageTotal {
    pub kind: String,
    pub model: Option<String>,
    pub unit: Option<String>,
    pub quantity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkRecord {
    pub chunk_id: String,
    pub seq: i64,
    pub content: String,
    pub chunk_type: String,
    pub heading_path: Option<String>,
    pub page: Option<i64>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub chunk_id: String,
    pub file_id: String,
    pub path: String,
    pub heading_path: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub content: String,
    /// Channel score, higher = better. Lexical: negated bm25 (LIKE fallback: 0);
    /// vector: 1 / (1 + distance).
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HybridHit {
    pub hit: SearchHit,
    pub rrf_score: f32,
    /// 1-based rank in the lexical channel, when it ranked there.
    pub lexical_rank: Option<usize>,
    /// 1-based rank in the vector channel, when it ranked there.
    pub vector_rank: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HybridSearchResult {
    pub hits: Vec<HybridHit>,
    pub lexical_used: bool,
    pub vector_used: bool,
    pub vector_available: bool,
    pub readiness: ReadinessSummary,
}

/// `Connection` is `Send` but not `Sync`; the mutex makes `&SubtexStore`
/// shareable (the `RetrievalReadPort` futures are `Send`). Operations are
/// short critical sections and never hold the lock across an `.await`.
pub struct SubtexStore {
    conn: Mutex<Connection>,
    vector_available: bool,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

static VEC_REGISTERED: OnceLock<bool> = OnceLock::new();

/// Register the bundled sqlite-vec entry point once per process. Every
/// connection opened afterwards loads it; when registration fails the store
/// degrades to lexical + outline only.
fn register_vec_extension() -> bool {
    *VEC_REGISTERED.get_or_init(|| unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        ))) == rusqlite::ffi::SQLITE_OK
    })
}

impl SubtexStore {
    /// Open (creating if needed) the store for `handle`, run migrations, and
    /// refresh `root.txt` (known-root enumeration reads it, never the disk).
    pub fn open(handle: &RootHandle) -> Result<Self, StoreError> {
        std::fs::create_dir_all(handle.store_dir())?;
        std::fs::create_dir_all(handle.scratch_dir())?;
        std::fs::write(
            handle.root_pointer_path(),
            format!("{}\n", handle.root().display()),
        )?;
        register_vec_extension();

        let conn = Connection::open(handle.db_path())?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // subtexd (writer) and avrag-api (tools) share the store across
        // processes; wait instead of failing with SQLITE_BUSY.
        conn.execute_batch(
            "PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
        )?;
        schema::migrate(&conn)?;
        let vector_available = schema::vector_available(&conn)?;
        schema::set_meta(&conn, "root_path", &handle.root().to_string_lossy())?;
        schema::set_meta(&conn, "embedding_dim", &EMBEDDING_DIM.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
            vector_available,
        })
    }

    pub fn vector_available(&self) -> bool {
        self.vector_available
    }

    pub fn schema_version(&self) -> Result<i64, StoreError> {
        Ok(self
            .meta_get("schema_version")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    pub fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT value FROM meta WHERE key = ?1",
                    params![key],
                    |r| r.get(0),
                )
                .optional()?)
        })
    }

    /// Register or refresh a scanned file; the file id is stable per path.
    pub fn upsert_file(
        &self,
        path: &str,
        content_hash: &str,
        size: u64,
        mtime_ms: u64,
    ) -> Result<String, StoreError> {
        self.with_conn(|conn| {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO files(id, path, content_hash, size, mtime_ms, first_seen_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(path) DO UPDATE SET
                     content_hash = excluded.content_hash,
                     size = excluded.size,
                     mtime_ms = excluded.mtime_ms",
                params![new_id, path, content_hash, size as i64, mtime_ms as i64, now_iso()],
            )?;
            Ok(conn.query_row("SELECT id FROM files WHERE path = ?1", params![path], |r| {
                r.get(0)
            })?)
        })
    }

    /// Per-file readiness state machine: lexical → outline → vector.
    pub fn set_file_readiness(
        &self,
        file_id: &str,
        lexical: bool,
        outline: bool,
        vector: bool,
    ) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO readiness(file_id, lexical, outline, vector, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(file_id) DO UPDATE SET
                     lexical = excluded.lexical,
                     outline = excluded.outline,
                     vector = excluded.vector,
                     updated_at = excluded.updated_at",
                params![file_id, lexical, outline, vector, now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn readiness_summary(&self) -> Result<ReadinessSummary, StoreError> {
        self.with_conn(|conn| {
            let files: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
            let lexical_ready: i64 = conn
                .query_row("SELECT COUNT(*) FROM readiness WHERE lexical = 1", [], |r| r.get(0))?;
            let outline_ready: i64 = conn
                .query_row("SELECT COUNT(*) FROM readiness WHERE outline = 1", [], |r| r.get(0))?;
            let vector_ready: i64 = conn
                .query_row("SELECT COUNT(*) FROM readiness WHERE vector = 1", [], |r| r.get(0))?;
            Ok(ReadinessSummary {
                files,
                lexical_ready,
                outline_ready,
                vector_ready,
            })
        })
    }

    /// Enqueue a job for the `subtexd` consumer. Crash-safe: the store is the
    /// queue, so a job survives process death as long as the store exists.
    pub fn enqueue_job(
        &self,
        kind: &str,
        file_path: Option<&str>,
        payload: Option<&Value>,
    ) -> Result<i64, StoreError> {
        self.enqueue_job_in_state(kind, file_path, payload, "pending")
    }

    /// Enqueue with a non-default state (e.g. transcription batches start in
    /// `needs_confirmation` until the threshold check or the agent confirms).
    pub fn enqueue_job_in_state(
        &self,
        kind: &str,
        file_path: Option<&str>,
        payload: Option<&Value>,
        state: &str,
    ) -> Result<i64, StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO jobs(kind, file_path, payload, state, attempts, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, 0, ?5, ?5)",
                params![kind, file_path, payload.map(|p| p.to_string()), state, now_iso()],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Claim the oldest pending job (FIFO). Single statement, so it is atomic
    /// even against a concurrent consumer.
    pub fn claim_next_job(&self) -> Result<Option<JobRecord>, StoreError> {
        self.with_conn(|conn| {
            let claimed = conn.query_row(
                "UPDATE jobs SET state = 'running', attempts = attempts + 1, updated_at = ?1
                 WHERE id = (SELECT id FROM jobs WHERE state = 'pending' ORDER BY id LIMIT 1)
                 RETURNING id, kind, file_path, payload, state, attempts, last_error",
                params![now_iso()],
                map_job,
            );
            match claimed {
                Ok(job) => Ok(Some(job)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    /// Most recent job for one `kind` + `file_path` — used to dedupe
    /// transcription jobs against content hash changes.
    pub fn latest_job(&self, kind: &str, file_path: &str) -> Result<Option<JobRecord>, StoreError> {
        self.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id, kind, file_path, payload, state, attempts, last_error
                     FROM jobs WHERE kind = ?1 AND file_path = ?2
                     ORDER BY id DESC LIMIT 1",
                    params![kind, file_path],
                    map_job,
                )
                .optional()?)
        })
    }

    /// Every job of one kind, newest first (transcription progress queries).
    pub fn jobs_of_kind(&self, kind: &str) -> Result<Vec<JobRecord>, StoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, kind, file_path, payload, state, attempts, last_error
                 FROM jobs WHERE kind = ?1 ORDER BY id DESC",
            )?;
            let rows = stmt.query_map(params![kind], map_job)?;
            collect_rows(rows)
        })
    }

    pub fn set_job_state(&self, id: i64, state: &str) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE jobs SET state = ?1, updated_at = ?2 WHERE id = ?3",
                params![state, now_iso(), id],
            )?;
            Ok(())
        })
    }

    pub fn complete_job(&self, id: i64) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE jobs SET state = 'done', updated_at = ?1 WHERE id = ?2",
                params![now_iso(), id],
            )?;
            Ok(())
        })
    }

    pub fn fail_job(&self, id: i64, error: &str) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE jobs SET state = 'failed', last_error = ?1, updated_at = ?2 WHERE id = ?3",
                params![error, now_iso(), id],
            )?;
            Ok(())
        })
    }

    /// Startup sweep after a crash: anything left 'running' goes back to
    /// pending so the claim loop picks it up again.
    pub fn requeue_running(&self) -> Result<usize, StoreError> {
        self.with_conn(|conn| {
            Ok(conn.execute(
                "UPDATE jobs SET state = 'pending', updated_at = ?1 WHERE state = 'running'",
                params![now_iso()],
            )?)
        })
    }

    pub fn pending_job_count(&self) -> Result<i64, StoreError> {
        self.with_conn(|conn| {
            Ok(conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE state = 'pending'",
                [],
                |r| r.get(0),
            )?)
        })
    }

    /// Consumable ledger (embedding calls, transcription time). M1 keeps this
    /// inside the store; the credits account integration is M3.
    pub fn record_usage(
        &self,
        kind: &str,
        model: Option<&str>,
        quantity: f64,
        unit: Option<&str>,
        detail: Option<&Value>,
    ) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO usage(ts, kind, model, quantity, unit, detail) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![now_iso(), kind, model, quantity, unit, detail.map(|d| d.to_string())],
            )?;
            Ok(())
        })
    }

    pub fn usage_totals(&self) -> Result<Vec<UsageTotal>, StoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT kind, model, unit, SUM(quantity) FROM usage
                 GROUP BY kind, model, unit ORDER BY kind, model, unit",
            )?;
            let totals = stmt.query_map([], |row| {
                Ok(UsageTotal {
                    kind: row.get(0)?,
                    model: row.get(1)?,
                    unit: row.get(2)?,
                    quantity: row.get(3)?,
                })
            })?;
            let mut out = Vec::new();
            for total in totals {
                out.push(total?);
            }
            Ok(out)
        })
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let conn = self.conn.lock().expect("subtex store mutex poisoned");
        f(&conn)
    }
}

/// Standard reciprocal-rank-fusion constant.
const RRF_K: f32 = 60.0;
/// Trigram tokenizer cannot MATCH shorter queries; those use LIKE fallback.
const MIN_TRIGRAM_QUERY_CHARS: usize = 3;

impl SubtexStore {
    /// Replace all chunks of a file (reindex path). Old FTS entries and vector
    /// rows are cleaned in the same transaction.
    pub fn replace_file_chunks(
        &self,
        file_id: &str,
        chunks: &[ChunkRecord],
    ) -> Result<usize, StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "DELETE FROM chunks_vec WHERE chunk_id IN (SELECT chunk_id FROM chunks WHERE file_id = ?1)",
                params![file_id],
            )?;
            tx.execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;
            for c in chunks {
                tx.execute(
                    "INSERT INTO chunks(chunk_id, file_id, seq, content, chunk_type, heading_path, page, line_start, line_end)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        c.chunk_id,
                        file_id,
                        c.seq,
                        c.content,
                        c.chunk_type,
                        c.heading_path,
                        c.page,
                        c.line_start,
                        c.line_end
                    ],
                )?;
            }
            tx.commit()?;
            Ok(chunks.len())
        })
    }

    /// Store embedding vectors for chunks (insert into vec0 + mark embedded).
    pub fn set_chunk_vectors(&self, vectors: &[(String, Vec<f32>)]) -> Result<usize, StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let mut written = 0usize;
            for (chunk_id, vector) in vectors {
                let json = serde_json::to_string(vector)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                tx.execute(
                    "INSERT OR REPLACE INTO chunks_vec(chunk_id, embedding) VALUES(?1, ?2)",
                    params![chunk_id, json],
                )?;
                tx.execute(
                    "UPDATE chunks SET embedded = 1 WHERE chunk_id = ?1",
                    params![chunk_id],
                )?;
                written += 1;
            }
            tx.commit()?;
            Ok(written)
        })
    }

    /// Remove a file and all its derived rows (chunks + FTS via triggers,
    /// vectors explicitly, readiness via cascade). Returns false when absent.
    pub fn delete_file(&self, path: &str) -> Result<bool, StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "DELETE FROM chunks_vec WHERE chunk_id IN (
                     SELECT c.chunk_id FROM chunks c JOIN files f ON f.id = c.file_id WHERE f.path = ?1
                 )",
                params![path],
            )?;
            let removed = tx.execute("DELETE FROM files WHERE path = ?1", params![path])?;
            tx.commit()?;
            Ok(removed > 0)
        })
    }

    pub fn mark_file_indexed(&self, file_id: &str) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE files SET indexed_at = ?1 WHERE id = ?2",
                params![now_iso(), file_id],
            )?;
            Ok(())
        })
    }

    /// Lexical channel: FTS5 bm25 (trigram tokenizer); queries shorter than a
    /// trigram fall back to a LIKE scan so every stage can still answer.
    pub fn search_lexical(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, StoreError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        self.with_conn(|conn| {
            if query.chars().count() >= MIN_TRIGRAM_QUERY_CHARS {
                let mut stmt = conn.prepare(
                    "SELECT c.chunk_id, c.file_id, f.path, c.heading_path, c.line_start, c.line_end, c.content, bm25(chunks_fts)
                     FROM chunks_fts
                     JOIN chunks c ON c.rowid = chunks_fts.rowid
                     JOIN files f ON f.id = c.file_id
                     WHERE chunks_fts MATCH ?1
                     ORDER BY bm25(chunks_fts)
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![query, limit as i64], |row| {
                    let rank: f64 = row.get(7)?;
                    Ok(SearchHit {
                        chunk_id: row.get(0)?,
                        file_id: row.get(1)?,
                        path: row.get(2)?,
                        heading_path: row.get(3)?,
                        line_start: row.get(4)?,
                        line_end: row.get(5)?,
                        content: row.get(6)?,
                        score: -(rank as f32),
                    })
                })?;
                collect_rows(rows)
            } else {
                let pattern = format!("%{}%", escape_like(query));
                let mut stmt = conn.prepare(
                    "SELECT c.chunk_id, c.file_id, f.path, c.heading_path, c.line_start, c.line_end, c.content
                     FROM chunks c JOIN files f ON f.id = c.file_id
                     WHERE c.content LIKE ?1 ESCAPE '\\'
                     ORDER BY c.seq
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![pattern, limit as i64], |row| {
                    Ok(SearchHit {
                        chunk_id: row.get(0)?,
                        file_id: row.get(1)?,
                        path: row.get(2)?,
                        heading_path: row.get(3)?,
                        line_start: row.get(4)?,
                        line_end: row.get(5)?,
                        content: row.get(6)?,
                        score: 0.0,
                    })
                })?;
                collect_rows(rows)
            }
        })
    }

    /// Vector channel: sqlite-vec KNN. Empty when the vector layer is off.
    pub fn search_vector(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchHit>, StoreError> {
        if !self.vector_available {
            return Ok(Vec::new());
        }
        self.with_conn(|conn| {
            let json = serde_json::to_string(query_vector)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let mut knn = conn.prepare(
                "SELECT chunk_id, distance FROM chunks_vec WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance",
            )?;
            let knn_rows = knn.query_map(params![json, limit as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?;
            let mut knn_pairs = Vec::new();
            for row in knn_rows {
                knn_pairs.push(row?);
            }

            let mut hits = Vec::with_capacity(knn_pairs.len());
            let mut lookup = conn.prepare(
                "SELECT c.chunk_id, c.file_id, f.path, c.heading_path, c.line_start, c.line_end, c.content
                 FROM chunks c JOIN files f ON f.id = c.file_id WHERE c.chunk_id = ?1",
            )?;
            for (chunk_id, distance) in knn_pairs {
                let hit = lookup
                    .query_row(params![chunk_id], |row| {
                        Ok(SearchHit {
                            chunk_id: row.get(0)?,
                            file_id: row.get(1)?,
                            path: row.get(2)?,
                            heading_path: row.get(3)?,
                            line_start: row.get(4)?,
                            line_end: row.get(5)?,
                            content: row.get(6)?,
                            score: 1.0 / (1.0 + distance as f32),
                        })
                    })
                    .optional()?;
                if let Some(hit) = hit {
                    hits.push(hit);
                }
            }
            Ok(hits)
        })
    }

    /// Hybrid channel: lexical + vector fused by reciprocal rank fusion.
    /// `query_vector` is `None` (or the layer is off) → lexical only.
    pub fn search_hybrid(
        &self,
        query: &str,
        query_vector: Option<&[f32]>,
        limit: usize,
    ) -> Result<HybridSearchResult, StoreError> {
        let lexical = self.search_lexical(query, limit)?;
        let vector = match query_vector {
            Some(v) if self.vector_available => self.search_vector(v, limit)?,
            _ => Vec::new(),
        };

        let mut order: Vec<String> = Vec::with_capacity(lexical.len() + vector.len());
        let mut entries: HashMap<String, (f32, Option<usize>, Option<usize>, SearchHit)> =
            HashMap::new();
        for (idx, hit) in lexical.iter().enumerate() {
            let rank = idx + 1;
            let rrf = 1.0 / (RRF_K + rank as f32);
            order.push(hit.chunk_id.clone());
            entries.insert(hit.chunk_id.clone(), (rrf, Some(rank), None, hit.clone()));
        }
        for (idx, hit) in vector.iter().enumerate() {
            let rank = idx + 1;
            let rrf = 1.0 / (RRF_K + rank as f32);
            match entries.get_mut(&hit.chunk_id) {
                Some((score, _, vector_rank, _)) => {
                    *score += rrf;
                    *vector_rank = Some(rank);
                }
                None => {
                    order.push(hit.chunk_id.clone());
                    entries.insert(hit.chunk_id.clone(), (rrf, None, Some(rank), hit.clone()));
                }
            }
        }

        let mut hits: Vec<HybridHit> = order
            .into_iter()
            .filter_map(|id| {
                entries.remove(&id).map(|(rrf_score, lexical_rank, vector_rank, hit)| HybridHit {
                    hit,
                    rrf_score,
                    lexical_rank,
                    vector_rank,
                })
            })
            .collect();
        hits.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(limit);

        Ok(HybridSearchResult {
            hits,
            lexical_used: !lexical.is_empty(),
            vector_used: !vector.is_empty(),
            vector_available: self.vector_available,
            readiness: self.readiness_summary()?,
        })
    }

    /// Outline sources: files that have at least one chunk, ordered by path.
    pub fn outline_files(&self) -> Result<Vec<(String, String, i64)>, StoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT f.id, f.path, COUNT(c.rowid) FROM files f
                 JOIN chunks c ON c.file_id = f.id
                 GROUP BY f.id ORDER BY f.path",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            collect_rows(rows)
        })
    }

    /// Replace the per-file heading outline (extracted from Heading blocks at
    /// index time — independent of chunk merging, which can fold short
    /// headings into the previous chunk).
    pub fn replace_file_outline(&self, file_id: &str, entries: &[(i64, String)]) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM file_outline WHERE file_id = ?1", params![file_id])?;
            for (seq, heading_path) in entries {
                tx.execute(
                    "INSERT INTO file_outline(file_id, seq, heading_path) VALUES(?1, ?2, ?3)",
                    params![file_id, seq, heading_path],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    /// Heading paths of a file in document order.
    pub fn file_outline(&self, file_id: &str) -> Result<Vec<String>, StoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT heading_path FROM file_outline WHERE file_id = ?1 ORDER BY seq",
            )?;
            let rows = stmt.query_map(params![file_id], |row| row.get(0))?;
            collect_rows(rows)
        })
    }

    pub fn file_first_chunk_content(&self, file_id: &str) -> Result<Option<String>, StoreError> {
        self.with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT content FROM chunks WHERE file_id = ?1 ORDER BY seq LIMIT 1",
                    params![file_id],
                    |r| r.get(0),
                )
                .optional()?)
        })
    }

    /// `rel_path -> content_hash` of every known file — the "previous" map for
    /// the next `scan_diff` (per-file content-hash incremental indexing).
    pub fn file_hashes(&self) -> Result<HashMap<String, String>, StoreError> {
        Ok(self
            .file_scan_state()?
            .into_iter()
            .map(|(path, state)| (path, state.content_hash))
            .collect())
    }

    /// Full scan-cache state: hash + size + mtime per known file, so the
    /// periodic reconcile sweep can skip re-hashing unchanged files.
    pub fn file_scan_state(
        &self,
    ) -> Result<HashMap<String, subtex_core::scanner::CachedEntry>, StoreError> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT path, content_hash, size, mtime_ms FROM files")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            let mut map = HashMap::new();
            for row in rows {
                let (path, hash, size, mtime) = row?;
                map.insert(
                    path,
                    subtex_core::scanner::CachedEntry {
                        content_hash: hash,
                        size: size.max(0) as u64,
                        mtime_ms: mtime.max(0) as u64,
                    },
                );
            }
            Ok(map)
        })
    }
}

fn map_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobRecord> {
    let payload: Option<String> = row.get(3)?;
    Ok(JobRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        file_path: row.get(2)?,
        payload: payload.and_then(|p| serde_json::from_str(&p).ok()),
        state: row.get(4)?,
        attempts: row.get(5)?,
        last_error: row.get(6)?,
    })
}

fn collect_rows<T, E>(rows: impl Iterator<Item = Result<T, E>>) -> Result<Vec<T>, StoreError>
where
    E: Into<StoreError>,
{
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(Into::into)?);
    }
    Ok(out)
}

fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Fixture {
        _dir: TempDir,
        handle: RootHandle,
    }

    fn fixture() -> Fixture {
        let dir = TempDir::new().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let handle = RootHandle::with_data_dir(&root, dir.path().join("data")).expect("handle");
        Fixture { _dir: dir, handle }
    }

    #[test]
    fn open_creates_layout_root_pointer_and_schema() {
        let fx = fixture();
        let store = SubtexStore::open(&fx.handle).unwrap();

        assert!(fx.handle.db_path().is_file());
        assert!(fx.handle.scratch_dir().is_dir());
        assert_eq!(
            std::fs::read_to_string(fx.handle.root_pointer_path()).unwrap(),
            format!("{}\n", fx.handle.root().display())
        );
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        let has_vec_table: i64 = store
            .with_conn(|conn| {
                Ok(conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name = 'chunks_vec'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(has_vec_table, i64::from(store.vector_available()));
        assert_eq!(
            store.meta_get("root_path").unwrap().as_deref(),
            Some(fx.handle.root().to_string_lossy().as_ref())
        );
    }

    #[test]
    fn reopen_preserves_jobs_files_readiness_usage() {
        let fx = fixture();
        let file_id = {
            let store = SubtexStore::open(&fx.handle).unwrap();
            store.enqueue_job("parse", Some("a.md"), None).unwrap();
            store.enqueue_job("parse", Some("b.md"), None).unwrap();
            let job = store.claim_next_job().unwrap().unwrap();
            store.complete_job(job.id).unwrap();
            let id = store.upsert_file("a.md", "hash-a", 12, 1000).unwrap();
            store.set_file_readiness(&id, true, true, false).unwrap();
            store
                .record_usage("embedding", Some("text-embedding-v4"), 100.0, Some("tokens"), None)
                .unwrap();
            id
        };

        let store = SubtexStore::open(&fx.handle).unwrap();
        assert_eq!(store.pending_job_count().unwrap(), 1);
        assert_eq!(
            store.upsert_file("a.md", "hash-a", 12, 1000).unwrap(),
            file_id,
            "file id must be stable per path"
        );
        assert_eq!(
            store.readiness_summary().unwrap(),
            ReadinessSummary { files: 1, lexical_ready: 1, outline_ready: 1, vector_ready: 0 }
        );
        let totals = store.usage_totals().unwrap();
        assert_eq!(totals.len(), 1);
        assert_eq!(totals[0].kind, "embedding");
        assert_eq!(totals[0].quantity, 100.0);
    }

    #[test]
    fn claim_is_fifo_and_requeue_recovers_after_crash() {
        let fx = fixture();
        let store = SubtexStore::open(&fx.handle).unwrap();
        let a = store.enqueue_job("parse", Some("a.md"), None).unwrap();
        let b = store.enqueue_job("parse", Some("b.md"), None).unwrap();

        let first = store.claim_next_job().unwrap().unwrap();
        assert_eq!(first.id, a);
        let second = store.claim_next_job().unwrap().unwrap();
        assert_eq!(second.id, b);
        assert!(store.claim_next_job().unwrap().is_none());

        assert_eq!(store.requeue_running().unwrap(), 2);
        let again = store.claim_next_job().unwrap().unwrap();
        assert_eq!(again.id, a);
        assert_eq!(again.attempts, 2);

        store.complete_job(again.id).unwrap();
        store.fail_job(second.id, "embedding backend unreachable").unwrap();
        assert_eq!(store.pending_job_count().unwrap(), 0);
        assert!(store.claim_next_job().unwrap().is_none());
        let state: String = store
            .with_conn(|conn| {
                Ok(conn.query_row(
                    "SELECT state FROM jobs WHERE id = ?1",
                    params![second.id],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(state, "failed");
    }

    #[test]
    fn readiness_summary_counts_per_layer() {
        let fx = fixture();
        let store = SubtexStore::open(&fx.handle).unwrap();
        let f1 = store.upsert_file("a.md", "h1", 1, 1).unwrap();
        let f2 = store.upsert_file("b.md", "h2", 1, 1).unwrap();
        store.set_file_readiness(&f1, true, false, false).unwrap();
        store.set_file_readiness(&f2, true, true, false).unwrap();

        assert_eq!(
            store.readiness_summary().unwrap(),
            ReadinessSummary { files: 2, lexical_ready: 2, outline_ready: 1, vector_ready: 0 }
        );
    }

    #[test]
    fn usage_totals_aggregate_by_kind_model_unit() {
        let fx = fixture();
        let store = SubtexStore::open(&fx.handle).unwrap();
        store
            .record_usage("embedding", Some("text-embedding-v4"), 100.0, Some("tokens"), None)
            .unwrap();
        store
            .record_usage("embedding", Some("text-embedding-v4"), 50.0, Some("tokens"), None)
            .unwrap();
        store
            .record_usage(
                "transcription",
                Some("qwen-audio-3.0-asr-flash-filetrans"),
                3600.0,
                Some("seconds"),
                None,
            )
            .unwrap();

        let totals = store.usage_totals().unwrap();
        assert_eq!(totals.len(), 2);
        assert_eq!(totals[0].kind, "embedding");
        assert_eq!(totals[0].quantity, 150.0);
        assert_eq!(totals[1].kind, "transcription");
        assert_eq!(totals[1].quantity, 3600.0);
    }

    #[test]
    fn vector_layer_consistent_with_availability() {
        let fx = fixture();
        let store = SubtexStore::open(&fx.handle).unwrap();
        let engine = store.meta_get("vector_engine").unwrap();
        if store.vector_available() {
            assert_eq!(engine.as_deref(), Some("sqlite-vec"));
            store
                .with_conn(|conn| {
                    Ok(conn.query_row("SELECT vec_version()", [], |r| r.get::<_, String>(0))?)
                })
                .unwrap();
        } else {
            assert_eq!(engine.as_deref(), Some("off"));
        }
    }

    fn unit_vector(active_dim: usize) -> Vec<f32> {
        let mut v = vec![0.0; EMBEDDING_DIM];
        v[active_dim] = 1.0;
        v
    }

    #[test]
    fn replace_chunks_updates_fts_vectors_and_delete_cleans_all() {
        let fx = fixture();
        let store = SubtexStore::open(&fx.handle).unwrap();
        let file_id = store.upsert_file("notes.md", "h", 10, 1).unwrap();
        let c1 = ChunkRecord {
            chunk_id: "c1".into(),
            seq: 0,
            content: "数据库连接池使用 pgvector 存储向量".into(),
            chunk_type: "paragraph".into(),
            heading_path: Some("项目笔记".into()),
            page: None,
            line_start: Some(2),
            line_end: Some(4),
        };
        let c2 = ChunkRecord {
            chunk_id: "c2".into(),
            seq: 1,
            content: "rust async runtime notes".into(),
            chunk_type: "paragraph".into(),
            heading_path: None,
            page: None,
            line_start: None,
            line_end: None,
        };
        assert_eq!(store.replace_file_chunks(&file_id, &[c1, c2]).unwrap(), 2);

        let hits = store.search_lexical("数据库", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "c1");
        assert_eq!(hits[0].path, "notes.md");
        assert_eq!(hits[0].line_start, Some(2));
        assert_ne!(hits[0].score, 0.0);

        let short = store.search_lexical("库", 5).unwrap();
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].chunk_id, "c1");
        assert_eq!(short[0].score, 0.0, "LIKE fallback has no bm25 score");

        if store.vector_available() {
            let e1 = unit_vector(0);
            let e2 = unit_vector(1);
            store
                .set_chunk_vectors(&[("c1".into(), e1.clone()), ("c2".into(), e2)])
                .unwrap();

            let vec_hits = store.search_vector(&e1, 2).unwrap();
            assert_eq!(vec_hits[0].chunk_id, "c1");
            assert!((vec_hits[0].score - 1.0).abs() < 1e-6, "distance 0 → score 1");

            let hybrid = store.search_hybrid("数据库", Some(&e1), 5).unwrap();
            assert!(hybrid.lexical_used && hybrid.vector_used);
            assert_eq!(hybrid.hits[0].hit.chunk_id, "c1");
            assert_eq!(hybrid.hits[0].lexical_rank, Some(1));
            assert_eq!(hybrid.hits[0].vector_rank, Some(1));
            assert!(hybrid.hits[0].rrf_score > hybrid.hits[1].rrf_score);

            store
                .replace_file_chunks(&file_id, &[ChunkRecord {
                    chunk_id: "c2".into(),
                    seq: 1,
                    content: "rust async runtime notes".into(),
                    chunk_type: "paragraph".into(),
                    heading_path: None,
                    page: None,
                    line_start: None,
                    line_end: None,
                }])
                .unwrap();
            let embedded: i64 = store
                .with_conn(|conn| {
                    Ok(conn.query_row("SELECT COUNT(*) FROM chunks WHERE embedded = 1", [], |r| r.get(0))?)
                })
                .unwrap();
            assert_eq!(embedded, 0, "replacement resets vector state");
            assert!(store.search_vector(&e1, 2).unwrap().is_empty());
        }

        assert!(store.delete_file("notes.md").unwrap());
        assert!(store.search_lexical("数据库", 5).unwrap().is_empty());
        if store.vector_available() {
            assert!(store.search_vector(&unit_vector(0), 2).unwrap().is_empty());
        }
        assert_eq!(store.readiness_summary().unwrap().files, 0);
        assert!(!store.delete_file("notes.md").unwrap());
    }

    #[test]
    fn outline_rows_replace_and_order() {
        let fx = fixture();
        let store = SubtexStore::open(&fx.handle).unwrap();
        let file_id = store.upsert_file("a.md", "h", 1, 1).unwrap();
        store
            .replace_file_outline(
                &file_id,
                &[(0, "安装".into()), (1, "安装 > 前置要求".into()), (2, "安装".into())],
            )
            .unwrap();
        assert_eq!(
            store.file_outline(&file_id).unwrap(),
            vec!["安装", "安装 > 前置要求", "安装"]
        );
        store.replace_file_outline(&file_id, &[(0, "部署".into())]).unwrap();
        assert_eq!(store.file_outline(&file_id).unwrap(), vec!["部署"]);
        assert!(store.delete_file("a.md").unwrap());
        assert!(store.file_outline(&file_id).unwrap().is_empty());
    }
}
