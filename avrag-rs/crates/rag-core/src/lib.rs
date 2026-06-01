pub mod context;
pub mod evidence_gate;
pub mod focus_mode;
pub mod merge;
pub mod retrieval; // export retrieval functions
pub mod runtime;

pub use evidence_gate::{
    DegradeKind, DefaultEvidenceGate, EvidenceGate, EvidenceGateConfig, EvidenceGateInput,
    EvidenceGateOutcome,
};
pub use focus_mode::{
    CompressedChunk, FocusError, FocusMode, ScoreBasedFocusMode,
};
pub use retrieval::ScoredChunk;
pub use runtime::{RagConfig, RagRuntime, RetrievalDataPlane};
