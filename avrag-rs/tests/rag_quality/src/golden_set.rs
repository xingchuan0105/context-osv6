//! Golden dataset types for RAG quality evaluation.
//!
//! PRD §13.2: "黄金集规模：100~500 条 {query, expected_answer, source_chunks}"

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

    /// Whether the model is expected to ANSWER (true) or REFUSE (false).
    ///
    /// Required by the generation-layer refusal gate (Phase 0.4): a correct
    /// refusal on an out-of-scope query must not be penalized, and a refusal on
    /// an in-scope query must be flagged. Defaults to `true` so existing
    /// golden sets (which assume an answer is expected) keep their semantics.
    #[serde(default = "default_expected_should_answer")]
    pub expected_should_answer: bool,

    /// Extra Chinese refusal cue words that mark an answer as a refusal for
    /// this example, beyond the default refusal lexicon. Empty by default.
    #[serde(default)]
    pub refusal_keywords: Vec<String>,

    /// Key facts that should appear in a correct answer. Used by richer
    /// generation correctness checks and LLM-as-Judge calibration.
    #[serde(default)]
    pub must_include: Vec<String>,

    /// Facts/phrases that must not appear in a correct answer.
    #[serde(default)]
    pub must_not_include: Vec<String>,

    /// Anchor terms that a good retrieval strategy should try. This is a
    /// diagnostic aid for query-generation failures, not a direct answer gate.
    #[serde(default)]
    pub retrieval_hints: Vec<String>,

    /// Human-assigned difficulty bucket for stratified reporting.
    #[serde(default)]
    pub difficulty: GoldenDifficulty,

    /// Optional graded relevance for nDCG. Keys are chunk ids; values are 0..3.
    #[serde(default)]
    pub relevance_grades: BTreeMap<String, u8>,

    /// Expected runtime tool name for tool-coverage probes (`golden_set_tools.json`).
    /// Matches `ToolResult.tool` (e.g. `doc_summary`, `doc_profile`, `graph_retrieval`).
    #[serde(default)]
    pub expected_tool: Option<String>,

    /// Expected ordered tool subsequence for multi-step probes (e.g. index two-step:
    /// `doc_profile` → `index_lookup` where `chunk_fetch` shim maps to `index_lookup`).
    #[serde(default)]
    pub expected_tool_sequence: Option<Vec<String>>,

    /// When true, probe needs `INGESTION_TRIPLET_ENABLED=1` corpus re-ingest (graph tools).
    #[serde(default)]
    pub requires_triplet_reingest: bool,
}

fn default_expected_should_answer() -> bool {
    true
}

fn default_mode() -> String {
    "rag".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoldenDifficulty {
    #[default]
    Medium,
    Easy,
    Hard,
    Adversarial,
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
                // ChunkId matching requires cross-referencing by chunk_id at the
                // harness layer (the matcher only sees `retrieved_content`). Return
                // `false` as a fail-safe: silently returning `true` here would make
                // EVERY chunk "match" the golden, forcing recall=100% and masking
                // all retrieval failures (误杀). No current golden uses ChunkId;
                // when one is added, score_retrieval/score_selection must match by
                // id explicitly.
                false
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

    #[test]
    fn tools_golden_set_loads_tool_coverage_fields() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden_set_tools.json");
        let dataset = GoldenDataset::load(path).expect("load tools golden set");
        let subset = dataset
            .subsets
            .iter()
            .find(|s| s.name == "tools_v1")
            .expect("tools_v1 subset");
        assert_eq!(subset.examples.len(), 8, "tools golden set should have 8 probes");

        let with_tool = subset
            .examples
            .iter()
            .filter(|e| e.expected_tool.is_some())
            .count();
        let with_sequence = subset
            .examples
            .iter()
            .filter(|e| e.expected_tool_sequence.as_ref().is_some_and(|s| !s.is_empty()))
            .count();
        let triplet = subset
            .examples
            .iter()
            .filter(|e| e.requires_triplet_reingest)
            .count();
        assert_eq!(with_tool, 7, "7 probes with expected_tool (2 summary + 2 metadata + 2 graph + 1 section titles)");
        assert_eq!(with_sequence, 1, "1 probe with expected_tool_sequence");
        assert_eq!(triplet, 2, "2 graph probes require triplet reingest");

        let summary_probes: Vec<_> = subset
            .examples
            .iter()
            .filter(|e| e.description.starts_with("tool_summary"))
            .collect();
        assert_eq!(summary_probes.len(), 2);
        for ex in &summary_probes {
            assert_eq!(ex.expected_tool.as_deref(), Some("doc_summary"));
        }
    }

    #[test]
    fn smoke_v5_golden_set_has_curated_probe_coverage() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden_set_smoke_v5.json");
        let dataset = GoldenDataset::load(path).expect("load smoke v5 golden set");
        let subset = dataset
            .subsets
            .iter()
            .find(|s| s.name == "smoke_v5")
            .expect("smoke_v5 subset");
        assert!(
            subset.examples.len() >= 10,
            "smoke v5 should include at least 10 probes"
        );

        let mut capability_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for example in &subset.examples {
            let label = example
                .description
                .split('—')
                .next()
                .map(str::trim)
                .unwrap_or("");
            assert!(
                !label.is_empty(),
                "smoke probe description must start with subset label: {:?}",
                example.query
            );
            *capability_counts.entry(label.to_string()).or_insert(0) += 1;
            assert_eq!(example.mode, "rag");
        }

        for required in [
            "thesis_factual",
            "thesis_synthesis",
            "thesis_numeric",
            "thesis_adversarial",
            "ipd_table",
            "baiyao_pdf",
            "cross_document",
        ] {
            assert!(
                capability_counts.get(required).copied().unwrap_or(0) >= 1,
                "smoke v5 missing capability subset {required}"
            );
        }
    }
}
