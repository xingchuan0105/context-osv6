//! Golden dataset types for RAG quality evaluation.
//!
//! PRD §13.2: "黄金集规模：100~500 条 {query, expected_answer, source_chunks}"

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single golden-set example.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenExample {
    /// Natural-language query
    pub query: String,

    /// The expected answer (or key facts that should appear)
    pub expected_answer: String,

    /// Chunks that should be retrieved for this query.
    /// Each entry is a chunk content substring or keywords that must appear in retrieved chunks.
    pub source_chunks: Vec<ChunkMatch>,

    /// Citations that the answer should reference (chunk indices).
    #[serde(default)]
    pub expected_citations: Vec<u32>,

    /// Which RAG mode this example targets.
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Human-readable description of the example's intent.
    #[serde(default)]
    pub description: String,

    /// Whether this example tests a "hard" case (low recall risk).
    #[serde(default)]
    pub is_adversarial: bool,
}

fn default_mode() -> String {
    "rag".to_string()
}

/// How to match a source chunk in retrieved results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChunkMatch {
    /// Match by keywords that must all appear in the chunk.
    Keywords { keywords: Vec<String> },

    /// Match by exact or near-exact substring.
    Substring { text: String },

    /// Match by chunk ID (requires deterministic chunking).
    ChunkId { id: String },
}

impl ChunkMatch {
    /// Returns `true` if `retrieved_content` satisfies this match criterion.
    pub fn matches(&self, retrieved_content: &str) -> bool {
        match self {
            ChunkMatch::Keywords { keywords } => {
                let content_lower = retrieved_content.to_lowercase();
                keywords
                    .iter()
                    .all(|kw| content_lower.contains(&kw.to_lowercase()))
            }
            ChunkMatch::Substring { text } => retrieved_content
                .to_lowercase()
                .contains(&text.to_lowercase()),
            ChunkMatch::ChunkId { .. } => {
                // ChunkId matching requires cross-referencing by ID — handled by harness.
                true
            }
        }
    }
}

/// A curated subset of the full golden set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenSubset {
    pub name: String,
    pub description: String,
    pub examples: Vec<GoldenExample>,
}

/// The full golden dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenDataset {
    pub version: String,
    pub created_at: String,
    pub subsets: Vec<GoldenSubset>,
}

impl GoldenDataset {
    /// Load a golden set from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let dataset: GoldenDataset = serde_json::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("invalid golden set JSON: {}", e))?;
        Ok(dataset)
    }

    /// All examples across all subsets.
    pub fn all_examples(&self) -> impl Iterator<Item = &GoldenExample> {
        self.subsets.iter().flat_map(|s| s.examples.iter())
    }

    /// Examples filtered by mode.
    pub fn by_mode(&self, mode: &str) -> impl Iterator<Item = &GoldenExample> {
        self.all_examples().filter(move |e| e.mode == mode)
    }

    /// Total example count.
    pub fn len(&self) -> usize {
        self.all_examples().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_match() {
        let kw_match = ChunkMatch::Keywords {
            keywords: vec!["machine learning".into(), "neural network".into()],
        };
        assert!(
            kw_match.matches("Machine learning models like neural networks are widely used today.")
        );
        assert!(!kw_match.matches("Deep learning uses neural networks."));
        // Only one keyword matches
    }

    #[test]
    fn test_substring_match() {
        let sub_match = ChunkMatch::Substring {
            text: "transformer architecture".into(),
        };
        assert!(sub_match.matches("The transformer architecture revolutionized NLP."));
        assert!(sub_match.matches("The TRANSFORMER architecture is powerful.")); // case-insensitive
    }

    #[test]
    fn sample_sanity_set_has_required_phase6_coverage() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden_set.sample.json");
        let dataset = GoldenDataset::load(path).unwrap();

        assert_eq!(dataset.len(), 20);
        for subset_name in ["keyword", "semantic", "multimodal", "graph"] {
            let subset = dataset
                .subsets
                .iter()
                .find(|subset| subset.name == subset_name)
                .unwrap_or_else(|| panic!("missing subset {subset_name}"));
            assert!(
                subset.examples.len() >= 4,
                "subset {subset_name} must include at least 4 examples"
            );
            assert!(
                subset
                    .examples
                    .iter()
                    .all(|example| !example.source_chunks.is_empty()),
                "subset {subset_name} examples must declare expected evidence"
            );
        }
    }
}
