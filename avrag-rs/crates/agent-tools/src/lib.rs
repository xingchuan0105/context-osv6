//! Agent tool surface (TN Wave 6).
//!
//! Owns:
//! - [`tool_registry`] — single execute entry (`dispatch_tool`)
//! - [`skills`] — SkillComponent builtins + registry
//! - [`capability`] — tool/mode metadata + policy enforcement
//! - [`progressive`] — PromptRegistry skill-MD loader / disclosure catalog
//! - [`rag_bridge`] — RAG tool scope force + dispatch into `RagRuntime`
//! - [`weather`] — weather HTTP helper used by the weather skill
//!
//! Orchestration (`ReActLoop`, chat pipeline) stays in `app-chat` and depends
//! on this crate so tool changes need not recompile the full orchestrator matrix
//! when only skill/meta code changes (workspace incremental compile).
//!
//! # Vocabulary (T4 — do not collapse layers)
//!
//! | Term | Crate type | Role |
//! |------|------------|------|
//! | **Tool** | [`ToolCatalog`] entry | Sole **execute** route (`dispatch_tool`) |
//! | **SkillComponent** | [`skills::SkillComponent`] | Executable builtin (is a Tool; name is legacy) |
//! | **SkillMd** | [`progressive::Skill`] | Prompt body only — **no** `execute` |
//! | **Capability** | [`CapabilityRegistry`] + [`PolicyEnforcer`] | Metadata + **policy truth** (allow/deny/tier) |
//! | **HostTool** | (app-chat orchestrator) | Intercepted by brain; **never** register here |

/// Guaranteed recent user turns injected unconditionally (memory floor).
/// Shared with app-chat AgentRequest history assembly.
pub const MAX_PROMPT_HISTORY_TURNS: usize = 2;

pub mod capability;
pub mod catalog;
pub mod geoip;
pub mod progressive;
pub mod rag_bridge;
pub mod skills;
pub mod tool_registry;
pub mod weather;

pub use capability::{
    ActivationPhase, CapabilitiesResponse, CapabilityRegistry, ContextRiskLevel, Deprecation,
    EnforcementAction, EnforcementCondition, EnforcementRule, ModeSchema, Permission,
    PolicyEnforcer, RetryPolicy, RiskLevel, SkillCapability, SkillMetadata, ToolCapability,
    ToolMetadata, build_capabilities_response, chat_mode_schema, permissive, rag_mode_schema,
    search_mode_schema, standard_mode_schemas, standard_rules, strict, write_mode_schema,
};
pub use catalog::{RAG_TOOL_IDS, RegisteredTool, ToolCatalog, ToolExecKind};
pub use progressive::{
    DisclosureContext, DisclosureTier, DisclosureUnit, PromptRegistry, Skill, Tool,
    atomic_tool_catalog, atomic_tool_catalog_cached, evaluate_calculator_expression,
    search_specific_tools, search_specific_tools_cached,
};
pub use rag_bridge::{dispatch_rag_tool, force_doc_scope, intersect_doc_scope};
pub use skills::{ExecutionContext, SkillComponent, SkillRegistry, builtin_registry_cached};
pub use tool_registry::{
    CODEGEN_SDK_METHOD_NAMES, OwnedToolDeps, SAC_SUPERSEDED_NATIVE_TOOLS, ToolDispatchContext,
    dispatch_tool, execute_with_retry, is_codegen_sdk_method_as_native_tool, is_rag_tool,
    is_sac_superseded_native_tool, reject_codegen_method_as_native_tool,
    reject_sac_superseded_native_tool, tool_meta,
};
