//! Single physical tool catalog (TN Wave 1 residual / Wave 6).
//!
//! One [`ToolCatalog`] holds every executable tool as a [`RegisteredTool`]:
//! metadata for policy/disclosure + an execution kind (skill vs RAG).
//! SkillComponent bodies remain in [`crate::skills::SkillRegistry`]; the catalog
//! is the single lookup table for ids, meta, and dispatch routing.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::capability::{
    ActivationPhase, Permission, RetryPolicy, RiskLevel, ToolMetadata,
};
use crate::skills::{SkillComponent, SkillRegistry, builtin_registry_cached};

/// How a registered tool is executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecKind {
    /// Builtin SkillComponent in SkillRegistry.
    Skill,
    /// RagRuntime channel tool (`dense_retrieval`, …).
    Rag,
}

/// One entry in the unified tool catalog.
#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub meta: ToolMetadata,
    pub exec: ToolExecKind,
}

/// Process-wide tool id → RegisteredTool map.
pub struct ToolCatalog {
    tools: HashMap<String, RegisteredTool>,
}

static CATALOG: OnceLock<ToolCatalog> = OnceLock::new();

/// RAG tool ids (not SkillComponent builtins).
pub const RAG_TOOL_IDS: &[&str] = &[
    "dense_retrieval",
    "lexical_retrieval",
    "graph_retrieval",
    "index_lookup",
    "doc_summary",
    "doc_metadata",
    "doc_profile",
    "doc_scan",
];

impl ToolCatalog {
    pub fn standard_cached() -> &'static Self {
        CATALOG.get_or_init(Self::build_standard)
    }

    fn build_standard() -> Self {
        let mut tools = HashMap::new();
        let skill_reg = builtin_registry_cached();
        for skill in skill_reg.iter() {
            // Write refine control-loop tools are owned by WriteApp / WriteRefineLoopRunner
            // (ADR-0007 T2). They stay in SkillRegistry for Write mode disclosure only,
            // never as UnifiedAgent ReAct ToolCatalog entries.
            if skill.id().starts_with("write_refine") {
                continue;
            }
            let meta = meta_from_skill(skill);
            tools.insert(
                skill.id().to_string(),
                RegisteredTool {
                    meta,
                    exec: ToolExecKind::Skill,
                },
            );
        }
        for id in RAG_TOOL_IDS {
            tools.insert(
                (*id).to_string(),
                RegisteredTool {
                    meta: rag_tool_metadata(id),
                    exec: ToolExecKind::Rag,
                },
            );
        }
        Self { tools }
    }

    pub fn get(&self, id: &str) -> Option<&RegisteredTool> {
        self.tools.get(id)
    }

    pub fn tool_meta(&self, id: &str) -> Option<&ToolMetadata> {
        self.tools.get(id).map(|t| &t.meta)
    }

    pub fn is_rag(&self, id: &str) -> bool {
        matches!(
            self.tools.get(id).map(|t| t.exec),
            Some(ToolExecKind::Rag)
        )
    }

    pub fn is_skill(&self, id: &str) -> bool {
        matches!(
            self.tools.get(id).map(|t| t.exec),
            Some(ToolExecKind::Skill)
        )
    }

    pub fn list(&self) -> Vec<&RegisteredTool> {
        let mut v: Vec<_> = self.tools.values().collect();
        v.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
        v
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Skill registry used for execution (same process singleton as catalog build).
    pub fn skill_registry(&self) -> &'static SkillRegistry {
        builtin_registry_cached()
    }
}

fn meta_from_skill(skill: &dyn SkillComponent) -> ToolMetadata {
    let spec = skill.spec();
    let (permissions, applicable_strategies) = skill_policy_defaults(skill.id());
    ToolMetadata {
        id: spec.name.clone(),
        version: spec.version.clone(),
        owner: "builtin".to_string(),
        description: spec.description.clone(),
        input_schema: spec.input_schema.clone(),
        output_schema: spec.output_schema.clone(),
        risk_level: infer_risk(skill.id()),
        permissions,
        external_deps: vec![],
        deprecation: None,
        retry_policy: RetryPolicy::default(),
        activation_phase: ActivationPhase::PlanAndEvaluate,
        applicable_strategies,
    }
}

fn skill_policy_defaults(id: &str) -> (Vec<Permission>, Vec<String>) {
    let all = vec!["rag".into(), "search".into(), "chat".into()];
    let search = vec!["search".into()];
    match id {
        "web_search" | "web_fetch" => (vec![Permission::ExternalNetwork], search),
        "code_interpreter" => (vec![Permission::CodeExecution], all),
        "conversation_history_load" | "user_profile_load" => (vec![], all),
        "calculator" | "weather_query" => (vec![], all),
        _ if id.starts_with("write_refine") => (vec![], vec!["write".into()]),
        _ => (vec![], all),
    }
}

fn infer_risk(id: &str) -> RiskLevel {
    match id {
        "web_search" | "web_fetch" | "code_interpreter" => RiskLevel::High,
        "calculator" | "weather_query" | "conversation_history_load" | "user_profile_load" => {
            RiskLevel::Low
        }
        _ if id.starts_with("write_refine") => RiskLevel::Medium,
        _ => RiskLevel::Low,
    }
}

fn rag_tool_metadata(id: &str) -> ToolMetadata {
    let description = match id {
        "dense_retrieval" => "Dense vector retrieval over notebook documents",
        "lexical_retrieval" => "Lexical / BM25 retrieval over notebook documents",
        "graph_retrieval" => "Graph relation retrieval over notebook documents",
        "index_lookup" => "Lookup document index / section lookup",
        "doc_summary" => "Fetch document summary",
        "doc_metadata" => "Fetch document metadata",
        "doc_profile" => "Fetch document profile",
        "doc_scan" => "Scan document chunks for agent codegen",
        _ => "RAG runtime tool",
    };
    ToolMetadata {
        id: id.to_string(),
        version: "1.0.0".to_string(),
        owner: "rag-runtime".to_string(),
        description: description.to_string(),
        input_schema: serde_json::Value::Null,
        output_schema: serde_json::Value::Null,
        risk_level: RiskLevel::Medium,
        permissions: Vec::new(),
        external_deps: Vec::new(),
        deprecation: None,
        retry_policy: RetryPolicy::default(),
        activation_phase: ActivationPhase::PlanAndEvaluate,
        applicable_strategies: vec!["rag".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_registers_skills_and_rag() {
        let cat = ToolCatalog::standard_cached();
        assert!(cat.get("calculator").is_some());
        assert!(cat.get("web_search").is_some());
        assert!(cat.get("dense_retrieval").is_some());
        assert!(cat.is_skill("calculator"));
        assert!(cat.is_rag("dense_retrieval"));
        assert!(!cat.is_rag("calculator"));
        assert!(cat.len() >= RAG_TOOL_IDS.len() + 5);
    }

    #[test]
    fn skill_meta_has_schema() {
        let cat = ToolCatalog::standard_cached();
        let calc = cat.tool_meta("calculator").expect("calculator");
        assert_eq!(calc.id, "calculator");
        assert!(calc.input_schema.get("properties").is_some() || calc.input_schema.is_object());
    }

    #[test]
    fn write_refine_not_in_react_tool_catalog() {
        let cat = ToolCatalog::standard_cached();
        for id in [
            "write_refine_revise",
            "write_refine_research",
            "write_refine_finish",
            "write_refine_lexical",
        ] {
            assert!(
                cat.get(id).is_none(),
                "{id} must not be in ToolCatalog (WriteApp control ring only)"
            );
        }
        assert!(
            cat.skill_registry().get("write_refine_revise").is_none(),
            "write_refine must not be on SkillRegistry (WriteApp-local ToolSpec only)"
        );
    }
}
