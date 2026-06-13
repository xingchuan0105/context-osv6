use std::collections::HashMap;
use std::sync::OnceLock;

use app_core::ChatPersistencePort;
use contracts::{ToolResult, ToolSpec};
use serde_json::Value;

use super::SkillComponent;

static BUILTIN_REGISTRY: OnceLock<SkillRegistry> = OnceLock::new();

/// Runtime context passed to every `SkillComponent::execute` call.
///
/// Skills declare which dependencies they need via this context rather
/// than hard-coding global singletons, making them testable and
/// environment-agnostic.
pub struct ExecutionContext<'a> {
    /// Optional search provider — required by `web_search` and related skills.
    pub search_provider: Option<&'a dyn avrag_search::SearchProvider>,
    /// Auth + PG session context for memory retrieval tools.
    pub auth: Option<&'a avrag_auth::AuthContext>,
    pub session_id: Option<uuid::Uuid>,
    pub chat_persistence: Option<&'a dyn ChatPersistencePort>,
}

impl<'a> ExecutionContext<'a> {
    pub fn new(search_provider: Option<&'a dyn avrag_search::SearchProvider>) -> Self {
        Self {
            search_provider,
            auth: None,
            session_id: None,
            chat_persistence: None,
        }
    }

    pub fn with_memory(
        search_provider: Option<&'a dyn avrag_search::SearchProvider>,
        auth: Option<&'a avrag_auth::AuthContext>,
        session_id: Option<uuid::Uuid>,
        chat_persistence: Option<&'a dyn ChatPersistencePort>,
    ) -> Self {
        Self {
            search_provider,
            auth,
            session_id,
            chat_persistence,
        }
    }
}

/// Central registry for all `SkillComponent` instances.
///
/// lookup is O(1) via `HashMap`.  The built-in registry is lazily
/// initialised once per process via `builtin_registry_cached`.
pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn SkillComponent>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Register a skill component.
    pub fn register(&mut self, skill: Box<dyn SkillComponent>) {
        let id = skill.id().to_string();
        self.skills.insert(id, skill);
    }

    /// Look up a skill by its canonical id.
    pub fn get(&self, id: &str) -> Option<&dyn SkillComponent> {
        self.skills.get(id).map(|b| b.as_ref())
    }

    /// Returns true if a skill with the given id is registered.
    pub fn contains(&self, id: &str) -> bool {
        self.skills.contains_key(id)
    }

    /// Index-tier view: (name, description) pairs for every registered skill.
    ///
    /// Injected into the planner system prompt so the LLM knows which
    /// skills are available and when to call them.
    pub fn index(&self) -> Vec<(&str, &str)> {
        self.skills
            .values()
            .map(|s| (s.id(), s.description()))
            .collect()
    }

    /// Load-tier view: full `ToolSpec` for every registered skill.
    pub fn all_specs(&self) -> Vec<ToolSpec> {
        self.skills.values().map(|s| s.spec()).collect()
    }

    /// Execute a single tool call by looking up the registered skill.
    pub async fn execute<'a>(
        &self,
        id: &str,
        args: &Value,
        ctx: &'a ExecutionContext<'a>,
    ) -> ToolResult {
        match self.get(id) {
            Some(skill) => skill.execute(args, ctx).await,
            None => ToolResult {
                tool: id.to_string(),
                version: "1.0".to_string(),
                status: contracts::ToolStatus::NotImplemented,
                data: None,
                trace: None,
            },
        }
    }

    /// Iterate over all registered skills.
    pub fn iter(&self) -> impl Iterator<Item = &dyn SkillComponent> {
        self.skills.values().map(|b| b.as_ref())
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Return the lazily-initialised built-in registry.
///
/// Prefer this over constructing a new registry in hot paths.
pub fn builtin_registry_cached() -> &'static SkillRegistry {
    BUILTIN_REGISTRY.get_or_init(|| {
        let mut registry = SkillRegistry::new();
        super::builtin::register_all(&mut registry);
        registry
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ToolStatus;

    struct DummySkill;

    #[async_trait::async_trait]
    impl SkillComponent for DummySkill {
        fn id(&self) -> &str {
            "dummy"
        }
        fn version(&self) -> &str {
            "1.0"
        }
        fn description(&self) -> &str {
            "Load when testing the registry."
        }
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: "dummy".to_string(),
                version: "1.0".to_string(),
                description: self.description().to_string(),
                input_schema: serde_json::json!({"type": "object"}),
                output_schema: serde_json::json!({}),
            }
        }
        async fn execute<'a>(&self, _args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
            ToolResult {
                tool: "dummy".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({"ok": true})),
                trace: None,
            }
        }
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = SkillRegistry::new();
        reg.register(Box::new(DummySkill));
        assert!(reg.contains("dummy"));
        assert!(!reg.contains("missing"));
        let skill = reg.get("dummy").unwrap();
        assert_eq!(skill.id(), "dummy");
        assert_eq!(skill.description(), "Load when testing the registry.");
    }

    #[test]
    fn registry_index_returns_name_description_pairs() {
        let mut reg = SkillRegistry::new();
        reg.register(Box::new(DummySkill));
        let idx = reg.index();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].0, "dummy");
        assert!(idx[0].1.contains("Load when"));
    }

    #[test]
    fn registry_all_specs_matches_registered() {
        let mut reg = SkillRegistry::new();
        reg.register(Box::new(DummySkill));
        let specs = reg.all_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "dummy");
    }

    #[tokio::test]
    async fn registry_execute_known_skill() {
        let mut reg = SkillRegistry::new();
        reg.register(Box::new(DummySkill));
        let ctx = ExecutionContext::new(None);
        let result = reg.execute("dummy", &serde_json::json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::Ok);
    }

    #[tokio::test]
    async fn registry_execute_unknown_skill_returns_not_implemented() {
        let reg = SkillRegistry::new();
        let ctx = ExecutionContext::new(None);
        let result = reg.execute("unknown", &serde_json::json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }

    #[test]
    fn builtin_registry_cached_is_lazy() {
        let r1 = builtin_registry_cached();
        let r2 = builtin_registry_cached();
        assert!(std::ptr::eq(r1, r2));
    }

    #[test]
    fn builtin_registry_has_all_atomic_tools() {
        let reg = builtin_registry_cached();
        assert!(reg.contains("calculator"));
        assert!(reg.contains("code_interpreter"));
        assert!(reg.contains("weather_query"));
        assert!(reg.contains("web_search"));
        assert!(reg.contains("web_fetch"));
    }
}
