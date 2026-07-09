pub mod context;
pub mod evidence_gate;
pub mod execute_plan_policy;
pub mod focus_mode;
pub mod merge;
pub mod ports;
pub mod retrieval; // export retrieval functions
pub mod runtime;

pub use evidence_gate::{
    DefaultEvidenceGate, DegradeKind, EvidenceGate, EvidenceGateConfig, EvidenceGateInput,
    EvidenceGateOutcome,
};
pub use execute_plan_policy::{
    classify_placeholder_triplet, ensure_original_query_text_dense_item, execute_plan_from_rag_plan,
    execute_plan_to_chat_request, execute_plan_to_rag_plan, validate_execute_plan,
};
pub use focus_mode::{CompressedChunk, FocusError, FocusMode, ScoreBasedFocusMode};
pub use ports::{CachePort, ContentStore, ContentStoreError, IndexedChunk};
pub use retrieval::ScoredChunk;
pub use runtime::{RagConfig, RagRuntime, RetrievalDataPlane};
