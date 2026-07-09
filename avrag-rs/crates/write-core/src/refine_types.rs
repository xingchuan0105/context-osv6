//! Types for the WriteRefine ReAct loop: budget, context, snapshots, and constants.

use std::path;

use heavytail::diagnosis::{self, PreRefineDiagnosis};
use heavytail::feedforward::fingerprint_workspace;
use heavytail::persona::PersonaCard;
use heavytail::state::WriterBudget;
use heavytail::validator;
use heavytail::StyleParams;
use heavytail::workspace::DraftWorkspace;

use crate::material_pack::MaterialPack;

/// Hard ceiling on WriteRefine ReAct iterations (always enforced).
pub const WRITE_REFINE_HARD_REACT_CAP: u8 = 6;

/// Max effective `write_refine_revise` rounds in gate / `--no-budget` harness.
pub const WRITE_REFINE_GATE_MAX_REVISE: usize = 3;

/// Budget for the WriteRefine ReAct loop (plan §4.3).
#[derive(Debug, Clone)]
pub struct RefineLoopBudget {
    /// Max **effective** revise rounds (patch successfully applied). Mirrors
    /// `WriterBudget.max_rounds` (default 5).
    pub max_rounds: usize,
    /// Max ReAct iterations (loop framework rounds). From `write_refine.yaml`
    /// budget.max_iterations (default [`WRITE_REFINE_HARD_REACT_CAP`]).
    pub max_react_iterations: u8,
    /// Max in-loop on-demand research calls (plan: 5).
    pub max_on_demand_research: usize,
    /// Per-research-worker token budget (smaller than initial-draft research).
    pub per_research_worker_tokens: usize,
    /// Total refine-token cap.
    pub max_refine_tokens: usize,
    /// When true, `write_refine_finish` is rejected while hapax or zipf bands fail.
    pub enforce_core_band_finish_gate: bool,
}

impl Default for RefineLoopBudget {
    fn default() -> Self {
        Self {
            max_rounds: 5,
            max_react_iterations: WRITE_REFINE_HARD_REACT_CAP,
            max_on_demand_research: 5,
            per_research_worker_tokens: 4_000,
            max_refine_tokens: 40_000,
            enforce_core_band_finish_gate: false,
        }
    }
}

impl RefineLoopBudget {
    /// Build from the orchestrator's `WriterBudget` plus the yaml's loop cap.
    pub fn from_writer_budget(writer: &WriterBudget, react_cap: u8) -> Self {
        Self {
            max_rounds: writer.max_rounds,
            max_react_iterations: react_cap.min(WRITE_REFINE_HARD_REACT_CAP),
            max_on_demand_research: 5,
            per_research_worker_tokens: 4_000,
            max_refine_tokens: 40_000,
            enforce_core_band_finish_gate: false,
        }
    }

    /// M4 / experiment harness: disable token and research caps.
    /// ReAct iterations remain capped at [`WRITE_REFINE_HARD_REACT_CAP`];
    /// effective revise capped at [`WRITE_REFINE_GATE_MAX_REVISE`].
    pub fn unlimited() -> Self {
        Self {
            max_rounds: WRITE_REFINE_GATE_MAX_REVISE,
            max_react_iterations: WRITE_REFINE_HARD_REACT_CAP,
            max_on_demand_research: usize::MAX,
            per_research_worker_tokens: usize::MAX,
            max_refine_tokens: usize::MAX,
            enforce_core_band_finish_gate: false,
        }
    }

    pub fn tokens_capped(&self) -> bool {
        self.max_refine_tokens != usize::MAX
    }

    pub fn revise_rounds_capped(&self) -> bool {
        self.max_rounds != usize::MAX
    }

    pub fn react_iterations_capped(&self) -> bool {
        self.max_react_iterations != u8::MAX
    }

    pub fn research_capped(&self) -> bool {
        self.max_on_demand_research != usize::MAX
    }
}

/// Why the loop exited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// Agent called `write_refine_finish`.
    AgentFinish,
    /// ReAct iteration cap reached.
    IterationCap,
    /// Token cap reached.
    TokenCap,
    /// Revise-round cap reached and Agent did not call finish.
    ReviseRoundCap,
}

/// The per-round blackboard carried by the loop runner (plan §5.2).
pub struct RefineContext {
    pub workspace: DraftWorkspace,
    pub diagnosis: PreRefineDiagnosis,
    pub material_pack: MaterialPack,
    pub persona: Option<PersonaCard>,
    pub research_calls_used: usize,
    pub revise_rounds_used: usize,
    pub react_iteration: u8,
    pub tokens_used: usize,
    pub finish_reason: Option<FinishReason>,
    pub bands_satisfied: bool,
    /// Best draft retained by composite score S (plan §4.4 soft-exit invariant).
    /// Tracked as an owned snapshot so the loop can restore the historical best
    /// when a late revise lowers S. Kept consistent with `WriterState::best`.
    pub best_snapshot: Option<BestSnapshot>,
}

/// Owned best-version snapshot used to restore the workspace at loop exit.
#[derive(Debug, Clone)]
pub struct BestSnapshot {
    pub score: f64,
    pub workspace: DraftWorkspace,
}

impl RefineContext {
    pub fn new(
        workspace: DraftWorkspace,
        diagnosis: PreRefineDiagnosis,
        material_pack: MaterialPack,
        persona: Option<PersonaCard>,
    ) -> Self {
        Self {
            workspace,
            diagnosis,
            material_pack,
            persona,
            research_calls_used: 0,
            revise_rounds_used: 0,
            react_iteration: 0,
            tokens_used: 0,
            finish_reason: None,
            bands_satisfied: false,
            best_snapshot: None,
        }
    }

    /// Recompute diagnosis from the current workspace and update `bands_satisfied`.
    pub fn recompute(&mut self, style: &StyleParams, reservoir: &[String]) {
        let fp = fingerprint_workspace(&self.workspace);
        let validation = validator::validate(&fp, style);
        self.bands_satisfied = validation.passed;
        self.diagnosis = diagnosis::diagnose_pre_refine(&self.workspace, style, reservoir);
    }

    /// Persist a refine-level checkpoint under `{dir}/refine/` (plan §5.2).
    ///
    /// Writes `context.json` (counters + best score + workspace) and
    /// `material_pack.json`, so a mid-refine failure leaves enough state for
    /// inspection or future resume. Best-effort: callers log errors, never
    /// abort the loop on checkpoint failure.
    pub fn checkpoint(&self, dir: &path::Path) -> std::io::Result<()> {
        use serde::Serialize;
        let refine_dir = dir.join("refine");
        std::fs::create_dir_all(&refine_dir)?;

        #[derive(Serialize)]
        struct RefineCheckpoint<'a> {
            react_iteration: u8,
            revise_rounds_used: usize,
            research_calls_used: usize,
            tokens_used: usize,
            bands_satisfied: bool,
            finish_reason: Option<String>,
            best_score: Option<f64>,
            workspace: &'a DraftWorkspace,
        }
        let payload = RefineCheckpoint {
            react_iteration: self.react_iteration,
            revise_rounds_used: self.revise_rounds_used,
            research_calls_used: self.research_calls_used,
            tokens_used: self.tokens_used,
            bands_satisfied: self.bands_satisfied,
            finish_reason: self
                .finish_reason
                .as_ref()
                .map(|r| format!("{r:?}").to_lowercase()),
            best_score: self.best_snapshot.as_ref().map(|b| b.score),
            workspace: &self.workspace,
        };
        std::fs::write(
            refine_dir.join("context.json"),
            serde_json::to_vec_pretty(&payload)?,
        )?;
        std::fs::write(
            refine_dir.join("material_pack.json"),
            serde_json::to_vec_pretty(&self.material_pack)?,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MaterialPack;
    use heavytail::diagnosis::diagnose_pre_refine;
    use heavytail::workspace::{DraftWorkspace, ParagraphRecord, RhythmMode, SentenceRecord};
    use heavytail::workspace::SentenceId;

    fn make_workspace() -> DraftWorkspace {
        let mut ws = DraftWorkspace::default();
        ws.sentences = vec![
            SentenceRecord {
                id: SentenceId("s01".into()),
                text: "这是一句长度恰好二十字左右的示例句子。".into(),
                para: 0,
                tombstone: false,
            },
            SentenceRecord {
                id: SentenceId("s02".into()),
                text: "这是另一句差不多长度的中文示例句子。".into(),
                para: 0,
                tombstone: false,
            },
        ];
        ws.paragraphs = vec![ParagraphRecord {
            idx: 0,
            rhythm: RhythmMode::Mixed,
        }];
        ws
    }

    #[test]
    fn finish_reason_variants_are_distinct() {
        assert_ne!(FinishReason::AgentFinish, FinishReason::IterationCap);
        assert_ne!(FinishReason::TokenCap, FinishReason::ReviseRoundCap);
    }

    #[test]
    fn refine_context_new_initializes_counters() {
        let ws = make_workspace();
        let style = StyleParams::default();
        let diag = diagnose_pre_refine(&ws, &style, &[]);
        let ctx = RefineContext::new(ws, diag, MaterialPack::default(), None);
        assert_eq!(ctx.research_calls_used, 0);
        assert_eq!(ctx.revise_rounds_used, 0);
        assert_eq!(ctx.react_iteration, 0);
        assert_eq!(ctx.tokens_used, 0);
        assert!(ctx.finish_reason.is_none());
    }

    #[test]
    fn refine_context_recompute_updates_bands_satisfied() {
        let ws = make_workspace();
        let style = StyleParams::default();
        let diag = diagnose_pre_refine(&ws, &style, &[]);
        let mut ctx = RefineContext::new(ws, diag, MaterialPack::default(), None);
        assert!(!ctx.bands_satisfied);
        ctx.recompute(&style, &[]);
        assert_eq!(ctx.bands_satisfied, ctx.diagnosis.validation.passed);
    }

    #[test]
    fn refine_context_checkpoint_writes_artifacts() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut ctx = {
            let ws = make_workspace();
            let style = StyleParams::default();
            let diag = diagnose_pre_refine(&ws, &style, &[]);
            RefineContext::new(ws, diag, MaterialPack::default(), None)
        };
        ctx.revise_rounds_used = 2;
        ctx.research_calls_used = 1;
        ctx.tokens_used = 1234;

        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "heavytail-refine-ckpt-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        ctx.checkpoint(&dir).expect("checkpoint writes");

        let context = std::fs::read_to_string(dir.join("refine").join("context.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&context).unwrap();
        assert_eq!(json["revise_rounds_used"], 2);
        assert_eq!(json["research_calls_used"], 1);
        assert_eq!(json["tokens_used"], 1234);
        assert!(json["workspace"].is_object());
        assert!(dir.join("refine").join("material_pack.json").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
