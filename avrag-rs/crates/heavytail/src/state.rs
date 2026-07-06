//! Writer orchestrator state and file checkpoints (spec §5.4, §12).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::metrics::FingerprintReport;
use crate::score::Score;
use crate::skeleton::{MaterialCard, Skeleton};
use crate::workspace::DraftWorkspace;

const STATE_FILE: &str = "state.json";
const MATERIAL_CARDS_FILE: &str = "material_cards.json";
const SKELETON_FILE: &str = "skeleton.json";
const ARTICLE_DRAFT_FILE: &str = "article.draft";

/// Per-job refinement and research token budget (spec §12).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WriterBudget {
    pub research_tokens_per_worker: usize,
    pub max_rounds: usize,
    pub max_rhythm_ops: usize,
    pub max_lexical_ops: usize,
    pub total_token_cap: usize,
}

impl Default for WriterBudget {
    fn default() -> Self {
        Self {
            research_tokens_per_worker: 8_000,
            max_rounds: 5,
            max_rhythm_ops: 6,
            max_lexical_ops: 4,
            total_token_cap: 100_000,
        }
    }
}

/// Orchestrator phase (spec §12).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WriterPhase {
    Research,
    Skeleton,
    Drafting { section: usize },
    Refining { round: usize },
    Validating,
    Done,
    Failed,
}

/// Per-directive compliance from a refinement round (spec §10 step 5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ComplianceRecord {
    pub directive: String,
    pub complied: bool,
    pub asked: Option<String>,
    pub achieved: Option<String>,
}

/// One refinement round's artifacts (spec §5.4 sidecars + §12).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoundRecord {
    pub fingerprint: FingerprintReport,
    pub directives_json: String,
    pub patch_raw: String,
    pub compliance: Vec<ComplianceRecord>,
    pub score: Score,
}

/// Best draft retained by composite score S (spec §12).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BestVersion {
    /// Zero-based index into `WriterState::rounds`.
    pub round: usize,
    pub score: f64,
    pub canonical_text: String,
}

/// Checkpointed orchestrator state (spec §12).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WriterState {
    pub phase: WriterPhase,
    pub cards: Vec<MaterialCard>,
    pub skeleton: Option<Skeleton>,
    pub workspace: DraftWorkspace,
    pub rounds: Vec<RoundRecord>,
    pub best: Option<BestVersion>,
    pub tokens_used: usize,
}

impl Default for WriterState {
    fn default() -> Self {
        Self {
            phase: WriterPhase::Research,
            cards: Vec::new(),
            skeleton: None,
            workspace: DraftWorkspace::default(),
            rounds: Vec::new(),
            best: None,
            tokens_used: 0,
        }
    }
}

impl WriterState {
    /// Append a round and update `best` when composite S improves.
    pub fn record_round(&mut self, r: RoundRecord) {
        let round_idx = self.rounds.len();
        let score_s = r.score.s;
        self.rounds.push(r);

        let canonical = self.workspace.render_canonical();
        let replace = match &self.best {
            None => true,
            Some(best) => score_s > best.score,
        };
        if replace {
            self.best = Some(BestVersion {
                round: round_idx,
                score: score_s,
                canonical_text: canonical,
            });
        }
    }

    /// Write `state.json` and per-round sidecars under `dir` (spec §5.4).
    pub fn checkpoint(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir).with_context(|| format!("create checkpoint dir {}", dir.display()))?;

        for (idx, round) in self.rounds.iter().enumerate() {
            let round_num = idx + 1;
            write_json(
                &round_fingerprint_path(dir, round_num),
                &round.fingerprint,
            )?;
            fs::write(
                round_directives_path(dir, round_num),
                &round.directives_json,
            )
            .with_context(|| format!("write round-{round_num} directives"))?;
            fs::write(round_patch_path(dir, round_num), &round.patch_raw)
                .with_context(|| format!("write round-{round_num} patch"))?;
        }

        if !self.cards.is_empty() {
            write_json(&dir.join(MATERIAL_CARDS_FILE), &self.cards)?;
        }
        if let Some(ref skeleton) = self.skeleton {
            write_json(&dir.join(SKELETON_FILE), skeleton)?;
        }

        let draft_text = self
            .best
            .as_ref()
            .map(|b| b.canonical_text.clone())
            .unwrap_or_else(|| self.workspace.render_canonical());
        if !draft_text.is_empty() {
            fs::write(dir.join(ARTICLE_DRAFT_FILE), draft_text)
                .context("write article.draft")?;
        }

        write_json(&dir.join(STATE_FILE), self)?;
        Ok(())
    }

    /// Restore orchestrator state from `dir/state.json`.
    pub fn restore(dir: &Path) -> Result<Self> {
        let path = dir.join(STATE_FILE);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read checkpoint {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parse checkpoint {}", path.display()))
    }
}

fn round_fingerprint_path(dir: &Path, round: usize) -> PathBuf {
    dir.join(format!("round-{round}.fingerprint.json"))
}

fn round_directives_path(dir: &Path, round: usize) -> PathBuf {
    dir.join(format!("round-{round}.directives.json"))
}

fn round_patch_path(dir: &Path, round: usize) -> PathBuf {
    dir.join(format!("round-{round}.patch.txt"))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(value).context("serialize checkpoint JSON")?;
    fs::write(path, json).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{ParagraphRecord, RhythmMode, SentenceId, SentenceRecord};
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_job_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "heavytail-state-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_fingerprint() -> FingerprintReport {
        FingerprintReport {
            sentence_lengths: vec![12, 18, 9],
            mean_length: 13.0,
            cv: 0.35,
            autocorr_lag1: 0.2,
            lognormal_ks_stat: 0.1,
            total_tokens: 20,
            vocab_size: 15,
            ttr: 0.75,
            hapax_ratio: 0.4,
            zipf_exponent: 1.0,
            word_freq: BTreeMap::from([("示例".to_string(), 2)]),
        }
    }

    fn sample_round(score_s: f64) -> RoundRecord {
        RoundRecord {
            fingerprint: sample_fingerprint(),
            directives_json: r#"{"rhythm":[],"lexical":[]}"#.to_string(),
            patch_raw: "s01| 短句示例。\n".to_string(),
            compliance: vec![ComplianceRecord {
                directive: "REWRITE s01".to_string(),
                complied: true,
                asked: Some("约12字".to_string()),
                achieved: Some("11字".to_string()),
            }],
            score: Score {
                s: score_s,
                len: 0.8,
                burst: 0.7,
                hapax: 0.6,
                zipf: 0.9,
            },
        }
    }

    fn sample_state() -> WriterState {
        let mut state = WriterState {
            phase: WriterPhase::Refining { round: 2 },
            cards: vec![MaterialCard {
                id: "m01".to_string(),
                kind: MaterialKind::Fact,
                content: "示例事实".to_string(),
                source: serde_json::json!({"kind": "web"}),
                section_hint: Some("引言".to_string()),
                rare_terms: vec!["术语".to_string()],
            }],
            skeleton: Some(Skeleton {
                title: "测试文章".to_string(),
                sections: vec![],
            }),
            workspace: DraftWorkspace {
                sentences: vec![SentenceRecord {
                    id: SentenceId("s01".to_string()),
                    text: "短句示例。".to_string(),
                    para: 0,
                    tombstone: false,
                }],
                paragraphs: vec![ParagraphRecord {
                    idx: 0,
                    rhythm: RhythmMode::Mixed,
                }],
            },
            rounds: Vec::new(),
            best: None,
            tokens_used: 42,
        };
        state.record_round(sample_round(0.55));
        state.record_round(sample_round(0.72));
        state
    }

    use crate::skeleton::MaterialKind;

    #[test]
    fn record_round_keeps_best_by_score() {
        let mut state = WriterState::default();
        state.workspace.sentences.push(SentenceRecord {
            id: SentenceId("s01".to_string()),
            text: "第一句。".to_string(),
            para: 0,
            tombstone: false,
        });

        state.record_round(sample_round(0.5));
        assert_eq!(state.best.as_ref().unwrap().round, 0);
        assert!((state.best.as_ref().unwrap().score - 0.5).abs() < f64::EPSILON);

        state.record_round(sample_round(0.4));
        assert_eq!(state.best.as_ref().unwrap().round, 0);

        state.record_round(sample_round(0.9));
        assert_eq!(state.best.as_ref().unwrap().round, 2);
        assert!((state.best.as_ref().unwrap().score - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn checkpoint_restore_round_trip() {
        let dir = temp_job_dir();
        let original = sample_state();
        original.checkpoint(&dir).unwrap();

        let restored = WriterState::restore(&dir).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn checkpoint_writes_spec_sidecars() {
        let dir = temp_job_dir();
        let state = sample_state();
        state.checkpoint(&dir).unwrap();

        assert!(dir.join(STATE_FILE).is_file());
        assert!(dir.join(MATERIAL_CARDS_FILE).is_file());
        assert!(dir.join(SKELETON_FILE).is_file());
        assert!(dir.join(ARTICLE_DRAFT_FILE).is_file());

        for round in 1..=state.rounds.len() {
            assert!(round_fingerprint_path(&dir, round).is_file());
            assert!(round_directives_path(&dir, round).is_file());
            assert!(round_patch_path(&dir, round).is_file());
        }

        let patch = fs::read_to_string(round_patch_path(&dir, 1)).unwrap();
        assert!(patch.contains("s01|"));
    }
}
