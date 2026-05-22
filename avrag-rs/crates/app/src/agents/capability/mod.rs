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

pub use api::{build_capabilities_response, CapabilitiesResponse, SkillCapability, StrategySchema, ToolCapability, TransitionSchema};
pub use metadata::{ActivationPhase, Deprecation, Permission, RetryPolicy, RiskLevel, SkillMetadata, ToolMetadata};
pub use policy::{permissive, strict, standard_rules, ContextRiskLevel, EnforcementAction, EnforcementCondition, EnforcementRule, PolicyEnforcer};
pub use registry::CapabilityRegistry;
pub use router::{standard_policy, RouterCondition, RouterPolicy, RouterRule, RoutingDecision};
