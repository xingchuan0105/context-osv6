//! Capability Registry — unified registration layer for v5 architecture.
//!
//! All tools and skills are registered here. Strategies query this registry
//! at runtime to discover available capabilities. Replaces the v4 ModeBundle
//! hard-coded tool lists.

mod api;
mod metadata;
mod policy;
mod registry;
mod router;
mod schemas;

pub use api::{
    CapabilitiesResponse, SkillCapability, StrategySchema, ToolCapability, TransitionSchema,
    build_capabilities_response,
};
pub use metadata::{
    ActivationPhase, Deprecation, Permission, RetryPolicy, RiskLevel, SkillMetadata, ToolMetadata,
};
pub use policy::{
    ContextRiskLevel, EnforcementAction, EnforcementCondition, EnforcementRule, PolicyEnforcer,
    permissive, standard_rules, strict,
};
pub use registry::CapabilityRegistry;
pub use schemas::{chat_schema, rag_schema, search_schema, standard_strategy_schemas};
pub use router::{RouterCondition, RouterPolicy, RouterRule, RoutingDecision, standard_policy};
