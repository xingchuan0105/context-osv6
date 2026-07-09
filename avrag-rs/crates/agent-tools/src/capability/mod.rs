//! Capability Registry — prompt skill and strategy metadata.
//!
//! ADR-0007 keeps LLM-facing native tool schemas out of PromptRegistry.
//! Mode configs own tool disclosure through `tool_pool`; schemas are
//! resolved from the capability registry at call time. Runtime executors
//! may keep their own non-prompt metadata for enforcement.

mod api;
mod metadata;
mod policy;
mod registry;
mod schemas;

pub use api::{
    CapabilitiesResponse, ModeSchema, SkillCapability, ToolCapability, build_capabilities_response,
};
pub use metadata::{
    ActivationPhase, Deprecation, Permission, RetryPolicy, RiskLevel, SkillMetadata, ToolMetadata,
};
pub use policy::{
    ContextRiskLevel, EnforcementAction, EnforcementCondition, EnforcementRule, PolicyEnforcer,
    permissive, standard_rules, strict,
};
pub use registry::CapabilityRegistry;

pub use schemas::{
    chat_mode_schema, rag_mode_schema, search_mode_schema, standard_mode_schemas,
    write_mode_schema,
};
