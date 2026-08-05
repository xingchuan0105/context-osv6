pub mod test_doubles;
pub mod context;
pub mod evidence_gate;
pub mod focus_mode;
pub mod merge;
pub mod ports;
pub mod retrieval; // export retrieval functions
pub mod runtime;

pub use evidence_gate::{
    DefaultEvidenceGate, DegradeKind, EvidenceGate, EvidenceGateConfig, EvidenceGateInput,
    EvidenceGateOutcome,
};
pub use focus_mode::{CompressedChunk, FocusError, FocusMode, ScoreBasedFocusMode};
pub use merge::{
    adjacent_merge_enabled, adjacent_merge_shortlist_longlist, cut_top_k, dual_threshold_cut,
    global_rrf_merge, hydrate_cursors_from_store, rrf_merge,
};
pub use ports::{CachePort, ContentStore, ContentStoreError, IndexedChunk};
pub use retrieval::ScoredChunk;
pub use runtime::{RagConfig, RagRuntime, RetrievalDataPlane};
