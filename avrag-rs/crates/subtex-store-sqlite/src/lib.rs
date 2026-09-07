//! SQLite index store for the Subtex directory plugin.
//!
//! One store per project root under the Subtex application-data directory
//! (see `subtex_core::RootHandle`). The project directory stays the source of
//! truth: this store only holds derived index state, is disposable, and is
//! deleted by removing its directory.
//!
//! W0 scope: schema (meta / files / chunks / FTS5 / vec0 / jobs / usage /
//! readiness), job handoff between `avrag-api` (writer) and `subtexd`
//! (consumer), and a `RetrievalReadPort` implementation whose search methods
//! are filled in by the W1 indexing slice.

mod port;
mod schema;
mod store;

pub use store::{
    ChunkRecord, HybridHit, HybridSearchResult, JobRecord, ReadinessSummary, SearchHit, StoreError,
    SubtexStore, EMBEDDING_DIM, SCHEMA_VERSION,
};
