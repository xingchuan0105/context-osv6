pub mod context;
pub mod evidence_gate;
pub mod merge;
pub mod retrieval; // export retrieval functions
pub mod runtime;

pub use evidence_gate::{
    DegradeKind, DefaultEvidenceGate, EvidenceGate, EvidenceGateConfig, EvidenceGateInput,
    EvidenceGateOutcome,
};
pub use retrieval::ScoredChunk;
pub use runtime::{RagConfig, RagRuntime, RetrievalDataPlane};
