use std::collections::HashMap;

/// A Skill wraps a prompt text (system prompt) that guides the LLM's
/// behaviour for a specific phase (planning, evaluation, answering, etc).
///
/// Perplexity-style structure:
/// - Index-tier: name + description (routing trigger, "Load when...")
/// - Load-tier:  system_prompt body (from SKILL.md)
/// - Runtime:    assets, references, subskills (lazy-loaded from spokes)
#[derive(Clone)]
pub struct Skill {
    id: String,
    description: String,
    system_prompt: String,
    version: String,
    dependencies: Vec<String>,
    metadata: HashMap<String, String>,
    /// Runtime-tier: assets/ directory contents (file_name → content).
    /// Loaded at build time via include_str!.
    assets: HashMap<String, String>,
    /// Runtime-tier: references/ directory contents (file_name → content).
    references: HashMap<String, String>,
    /// JSON Schema for tool arguments, extracted from reference/args-schema.md.
    input_schema: Option<String>,
    /// JSON Schema for tool output, extracted from reference/output-schema.md.
    output_schema: Option<String>,
}

impl Skill {
    /// Legacy constructor for tests and fallback.
    pub fn new(id: impl Into<String>, description: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            system_prompt: system_prompt.into(),
            version: "1.0".to_string(),
            dependencies: Vec::new(),
            metadata: HashMap::new(),
            assets: HashMap::new(),
            references: HashMap::new(),
            input_schema: None,
            output_schema: None,
        }
    }

    /// Full constructor used by PromptRegistry after parsing frontmatter.
    pub fn with_meta(
        id: impl Into<String>,
        description: impl Into<String>,
        system_prompt: impl Into<String>,
        version: impl Into<String>,
        dependencies: Vec<String>,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            system_prompt: system_prompt.into(),
            version: version.into(),
            dependencies,
            metadata,
            assets: HashMap::new(),
            references: HashMap::new(),
            input_schema: None,
            output_schema: None,
        }
    }

    /// Attach runtime assets (build-time loaded from assets/ directory).
    pub fn with_assets(mut self, assets: HashMap<String, String>) -> Self {
        self.assets = assets;
        self
    }

    /// Attach runtime references (build-time loaded from references/ directory).
    pub fn with_references(mut self, references: HashMap<String, String>) -> Self {
        self.references = references;
        self
    }

    pub fn with_input_schema(mut self, schema: impl Into<String>) -> Self {
        self.input_schema = Some(schema.into());
        self
    }

    pub fn with_output_schema(mut self, schema: impl Into<String>) -> Self {
        self.output_schema = Some(schema.into());
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Index-tier routing trigger — "Load when..." format.
    ///
    /// This is NOT shown to the LLM in the skill body; it is used for
    /// index/catalog generation so the planner knows when to load this skill.
    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    pub fn assets(&self) -> &HashMap<String, String> {
        &self.assets
    }

    pub fn references(&self) -> &HashMap<String, String> {
        &self.references
    }

    pub fn input_schema(&self) -> Option<&str> {
        self.input_schema.as_deref()
    }

    pub fn output_schema(&self) -> Option<&str> {
        self.output_schema.as_deref()
    }

    /// Render at the Load tier: full system prompt.
    fn render_load(&self, ctx: &super::DisclosureContext) -> String {
        let is_first = !ctx.seen_unit_ids.contains(&self.id);
        let mut output = format!(
            "## Skill: {} (v{})\n{}",
            self.id, self.version, self.system_prompt
        );
        if !is_first {
            output.push_str("\n\n_(This skill has been disclosed in a previous iteration.)_");
        }
        if ctx.round > 0 {
            output.push_str(&format!("\n_(Iteration {}.)_", ctx.round));
        }
        output
    }

    /// Render at the Runtime tier: load + assets + references.
    fn render_runtime(&self, ctx: &super::DisclosureContext) -> String {
        let mut output = self.render_load(ctx);
        if !self.assets.is_empty() {
            output.push_str("\n\n### Assets\n");
            for (name, content) in &self.assets {
                output.push_str(&format!("\n#### {}\n{}", name, content));
            }
        }
        if !self.references.is_empty() {
            output.push_str("\n\n### References\n");
            for (name, content) in &self.references {
                output.push_str(&format!("\n#### {}\n{}", name, content));
            }
        }
        output
    }
}

impl super::DisclosureUnit for Skill {
    fn id(&self) -> &str {
        &self.id
    }

    fn render(&self, ctx: &super::DisclosureContext) -> String {
        match ctx.tier {
            // Index tier is deprecated for Skills in v5 — skills are disclosed
            // via their full body in the Plan phase (Bundle/Strategy layer).
            super::DisclosureTier::Index => String::new(),
            super::DisclosureTier::Load => self.render_load(ctx),
            super::DisclosureTier::Runtime => self.render_runtime(ctx),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn dependencies(&self) -> &[String] {
        &self.dependencies
    }
}
