//! Lead Agent + RAG/Web Workers contracts.
//!
//! Design: `docs/plans/2026-08-11-lead-rag-web-workers-design.md`.

mod brief;
mod evidence_pack;
mod plan_context;
mod web_merge;

pub use brief::{
    ActivatedCaps, PreferredSource, SubTask, TaskBrief, TaskBriefGateError, effective_web_queries,
    validate_task_brief,
};
pub use evidence_pack::{
    Coverage, EvidenceItem, EvidencePack, PackGateOutcome, apply_pack_gate, count_tool_ok,
};
pub use plan_context::{DocScopeSummary, LeadPlanContext};
pub use web_merge::{
    MergedWebHit, MergedWebHits, hits_to_evidence_items, merge_search_responses, normalize_url_key,
};
