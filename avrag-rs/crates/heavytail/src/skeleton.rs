//! Skeleton stage types (minimal for writer state; Task 11 expands this).

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialKind {
    Fact,
    Quote,
    Figure,
    Term,
    Inspiration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MaterialCard {
    pub id: String,
    pub kind: MaterialKind,
    pub content: String,
    /// MVP placeholder for `SourceRef`; integration maps real citations in Task 17.
    pub source: serde_json::Value,
    pub section_hint: Option<String>,
    pub rare_terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParagraphPlan {
    pub rhythm: crate::workspace::RhythmMode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkeletonSection {
    pub heading: String,
    pub key_points: Vec<String>,
    pub card_refs: Vec<String>,
    pub target_chars: usize,
    pub paragraphs: Vec<ParagraphPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Skeleton {
    pub title: String,
    pub sections: Vec<SkeletonSection>,
}
