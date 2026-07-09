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
pub use ports::{CachePort, ContentStore, ContentStoreError, IndexedChunk};
pub use retrieval::ScoredChunk;
pub use runtime::{RagConfig, RagRuntime, RetrievalDataPlane};
