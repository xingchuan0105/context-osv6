/// Unified interface for anything that can be disclosed progressively
/// to an LLM planner: both Tools (with JSON schema) and Skills (prompt text).
pub trait DisclosureUnit: Send + Sync {
    /// Unique identifier for this unit.
    fn id(&self) -> &str;

    /// Render this unit into the prompt context at the given disclosure tier.
    fn render(&self, ctx: &DisclosureContext) -> String;

    /// Downcast helper for concrete type extraction.
    fn as_any(&self) -> &dyn std::any::Any;

    /// IDs of other [`DisclosureUnit`]s this unit depends on.
    /// Rendered automatically before this unit when dependencies are resolved.
    fn dependencies(&self) -> &[String] {
        &[]
    }
}

/// Progressive disclosure tier — controls how much of a unit is revealed.
///
/// Perplexity-style three-tier loading:
/// - Index:  name + description only (~50 tokens). Used in Plan phase so the
///   planner knows what skills exist and when to load them.
/// - Load:   full SKILL.md body (~500-5000 tokens). Used when a skill is
///   actively needed in Execute / Answer phase.
/// - Runtime: full body + assets/references (unbounded). Used when a skill
///   declares it needs runtime resources (schemas, examples, etc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisclosureTier {
    /// Index tier: id + description only. Lightweight catalog entry.
    #[default]
    Index,
    /// Load tier: full system prompt / tool spec.
    Load,
    /// Runtime tier: load tier + assets + references.
    Runtime,
}

/// Context passed to [`DisclosureUnit::render`] so units can adapt their
/// output based on runtime state (e.g. history length, tier, etc).
#[derive(Debug, Clone, Default)]
pub struct DisclosureContext {
    /// Eval-loop round number (0-indexed).  Incremented every time the loop
    /// transitions from Evaluate back to Plan.
    pub round: usize,
    /// IDs of [`DisclosureUnit`]s that have already been rendered in prior
    /// turns.  A unit can use this to omit examples or verbose descriptions on
    /// subsequent disclosures.
    pub seen_unit_ids: std::collections::HashSet<String>,
    /// Disclosure tier controlling how much detail to reveal.
    pub tier: DisclosureTier,
}

impl DisclosureContext {
    /// Create a context for the given tier (used when rendering outside the
    /// normal phase flow, e.g. for skill catalog generation).
    pub fn with_tier(tier: DisclosureTier) -> Self {
        Self {
            tier,
            ..Self::default()
        }
    }
}
