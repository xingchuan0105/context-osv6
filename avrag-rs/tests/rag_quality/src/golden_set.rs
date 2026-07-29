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

    /// The expected answer (or key facts that should appear). `reference_answer`
    /// is the v2 name (ADR-0012 §2.6); both spellings deserialize into this field.
    #[serde(alias = "reference_answer")]
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

    /// New-paradigm capability tags (`["rag"]`, `["search"]`, `["rag","search"]`,
    /// `[]` = pure chat). Empty = derive from legacy `mode` (rag/search/chat).
    #[serde(default)]
    pub capabilities: Vec<String>,

    /// Scope the runner should use for this example: `"all"` (default — all
    /// corpus docs, matching the historical runner), `"empty"` (no docs — tests
    /// the empty-selection rule), or a corpus key (`"thesis"`, `"adr_pair"`,
    /// `"consulting_platform"`, `"consulting_compensation"`, `"ipd"`, `"baiyao"`).
    #[serde(default = "default_doc_scope_hint")]
    pub doc_scope_hint: String,

    /// Typed citation minimums for the orchestrator citation protocol
    /// (`[[cite:chunk]]` doc / `[[web:n]]` web). `None` = legacy
    /// `expected_citations` presence semantics only.
    #[serde(default)]
    pub expect_citations: Option<CitationExpectation>,

    /// Case needs outbound web search (Brave proxy). Runner may skip when the
    /// environment has no search network (`E2E_SKIP_NETWORK_CASES=1`).
    #[serde(default)]
    pub requires_network: bool,

    /// Same-session history to send before `query` (memory / coreference cases):
    /// flattened to `messages: [{role:user|assistant, content}]` in order.
    #[serde(default)]
    pub prior_turns: Vec<PriorTurn>,

    /// When set, runner injects `client_context.local_time` so time-aware
    /// answers (via `user_context`) become deterministic (e.g. "2026-07-19 15:04").
    #[serde(default)]
    pub client_time: Option<String>,

    /// Optional judge rubric notes (ADR-0012 §2.6): extra grading conventions
    /// written into the LLM-judge prompt (e.g. "accept 2019 年/2019年"). Not
    /// used by the deterministic v1 scoring path.
    #[serde(default)]
    pub rubric_notes: Option<String>,

    /// Memory / follow-up questions answered from conversation context rather
    /// than retrieval: the retrieval track is not applicable (skip
    /// RETRIEVAL_MISS, exclude from retrieval means, judge faithfulness as
    /// not_applicable). eval_v2 only; the legacy path ignores this field.
    #[serde(default)]
    pub expect_no_retrieval: bool,
}

/// A scripted prior user→assistant exchange (see `prior_turns`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorTurn {
    pub query: String,
    pub answer: String,
}

/// Typed citation minimums under the orchestrator paradigm.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CitationExpectation {
    #[serde(default)]
    pub min_doc: u32,
    #[serde(default)]
    pub min_web: u32,
}

fn default_doc_scope_hint() -> String {
    "all".to_string()
}

impl GoldenExample {
    /// The v2 `reference_answer` view of `expected_answer` (ADR-0012 §2.6).
    pub fn reference_answer(&self) -> &str {
        &self.expected_answer
    }

    /// Capabilities to send on the wire. Falls back to the legacy `mode`
    /// mapping (`rag`/`search` → single capability, anything else → pure chat)
    /// so pre-v3 golden files behave exactly as before.
    pub fn resolved_capabilities(&self) -> Vec<String> {
        if !self.capabilities.is_empty() {
            return self.capabilities.clone();
        }
        match self.mode.as_str() {
            "rag" => vec!["rag".to_string()],
            "search" => vec!["search".to_string()],
            _ => Vec::new(),
        }
    }
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
    fn reference_answer_alias_and_optional_rubric_notes() {
        // v2 spelling (ADR-0012 §2.6) deserializes into `expected_answer`.
        let v2 = r#"{
            "query": "q",
            "reference_answer": "Y公司2019年在大连建厂。",
            "source_chunks": [],
            "rubric_notes": "接受「2019 年」「2019年」"
        }"#;
        let ex: GoldenExample = serde_json::from_str(v2).unwrap();
        assert_eq!(ex.reference_answer(), "Y公司2019年在大连建厂。");
        assert_eq!(ex.rubric_notes.as_deref(), Some("接受「2019 年」「2019年」"));

        // Legacy spelling still works; rubric_notes stays optional.
        let legacy = r#"{"query": "q", "expected_answer": "a", "source_chunks": []}"#;
        let ex: GoldenExample = serde_json::from_str(legacy).unwrap();
        assert_eq!(ex.reference_answer(), "a");
        assert_eq!(ex.rubric_notes, None);
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
    fn realistic_v4_loads_grouped_orchestrator_fields() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden_set_realistic.json");
        let dataset = GoldenDataset::load(&path).expect("load realistic v4 golden set");
        assert_eq!(dataset.len(), 149, "113 legacy + 36 paradigm/group cases");

        let expect = [
            ("orchestrator_paradigm", 8),
            ("rag_search_joint", 6),
            ("chat_builtin_tools", 4),
            ("rag_codegen_channels", 7),
            ("memory_coreference", 3),
            ("search_web", 2),
            ("new_corpus_factual", 6),
        ];
        for (name, n) in expect {
            let subset = dataset
                .subsets
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("missing subset {name}"));
            assert_eq!(subset.examples.len(), n, "subset {name} size");
        }

        // Capability tags drive the new paradigm surface.
        let joint = dataset
            .subsets
            .iter()
            .find(|s| s.name == "rag_search_joint")
            .unwrap();
        assert!(
            joint
                .examples
                .iter()
                .all(|e| e.capabilities == ["rag", "search"])
        );
        // Memory cases carry scripted history; time case carries client_context.
        let mem = dataset
            .subsets
            .iter()
            .find(|s| s.name == "memory_coreference")
            .unwrap();
        assert!(mem.examples.iter().all(|e| !e.prior_turns.is_empty()));
        let tools = dataset
            .subsets
            .iter()
            .find(|s| s.name == "chat_builtin_tools")
            .unwrap();
        assert!(tools.examples.iter().any(|e| e.client_time.is_some()));
        // Legacy fallback unchanged.
        let legacy = dataset
            .subsets
            .iter()
            .find(|s| s.name == "thesis_factual")
            .unwrap()
            .examples[0]
            .clone();
        assert_eq!(legacy.resolved_capabilities(), vec!["rag".to_string()]);
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
