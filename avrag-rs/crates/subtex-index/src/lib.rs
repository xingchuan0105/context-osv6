//! Subtex indexing pipeline (A-line).
//!
//! Write path: scan diff → light/heavy parse → `build_ir_chunk_plan`
//! structure-aware chunking → store chunks (FTS5) → cloud embeddings
//! (sqlite-vec) → per-file readiness. Read path: token-budgeted outline.
//!
//! Light formats (markdown / text / code / csv) parse in pure Rust so a
//! reindex never spawns a subprocess per file; heavy formats (pdf, office)
//! reuse the existing ingestion subprocess parsers and degrade to an honest
//! `Unsupported` outcome when their binaries are missing.

pub mod audio;
pub mod embed;
pub mod index;
pub mod outline;
pub mod parse;

pub use audio::{audio_duration_secs, transcribe_file, CONFIRM_THRESHOLD_SECS, TRANSCRIPTS_DIR};
pub use embed::{CloudEmbedder, EmbeddingSettings, StoreUsageObserver, TextEmbedder};
pub use index::{DiffOutcome, FileIndexOutcome, Indexer};
pub use outline::{render_outline, Outline};
