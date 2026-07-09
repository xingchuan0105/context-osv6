//! Pre-refine diagnosis for WriteRefine agent loop.

use crate::feedforward::fingerprint_workspace;
use crate::metrics::FingerprintReport;
use crate::score::{self, Score};
use crate::validator::{self, ValidationReport};
use crate::workspace::DraftWorkspace;
use crate::StyleParams;

/// Lexical steering hint for the model / force-lexical synthesizer.
#[derive(Debug, Clone)]
pub struct WordHint {
    pub word: String,
    pub reason: String,
    pub action: String,
}

/// Snapshot of workspace quality before / during refine rounds.
#[derive(Debug, Clone)]
pub struct PreRefineDiagnosis {
    pub validation: ValidationReport,
    pub fingerprint: FingerprintReport,
    /// Composite style score S (higher is better).
    pub score_s: f64,
    pub score: Score,
    pub word_hints: Vec<WordHint>,
}

/// Diagnose the workspace against style bands and reservoir terms.
pub fn diagnose_pre_refine(
    workspace: &DraftWorkspace,
    style: &StyleParams,
    reservoir: &[String],
) -> PreRefineDiagnosis {
    let fingerprint = fingerprint_workspace(workspace);
    let validation = validator::validate(&fingerprint, style);
    let score = score::composite(&fingerprint, style);
    let word_hints = build_word_hints(&fingerprint, &validation, reservoir);
    PreRefineDiagnosis {
        validation,
        fingerprint,
        score_s: score.s,
        score,
        word_hints,
    }
}

/// Chinese brief for the per-round user message.
pub fn render_diagnosis_brief_zh(diag: &PreRefineDiagnosis, reservoir: &[String]) -> String {
    let mut out = String::from("## 诊断摘要\n\n");
    out.push_str(&format!("- 综合分 S：{:.3}\n", diag.score_s));
    out.push_str(&format!(
        "- 校验：{}\n",
        if diag.validation.passed {
            "已过关"
        } else {
            "未过关"
        }
    ));
    for m in &diag.validation.metric_results {
        out.push_str(&format!(
            "  - {} actual={:.3} target=[{:.3},{:.3}] {}\n",
            m.metric,
            m.actual,
            m.target.0,
            m.target.1,
            if m.passed { "ok" } else { "fail" }
        ));
    }
    if !diag.word_hints.is_empty() {
        out.push_str("\n### 用词提示\n");
        for h in diag.word_hints.iter().take(8) {
            out.push_str(&format!("- {}：{}（{}）\n", h.word, h.action, h.reason));
        }
    }
    if !reservoir.is_empty() {
        out.push_str("\n### 词库（节选）\n");
        out.push_str(
            &reservoir
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join("、"),
        );
        out.push('\n');
    }
    out
}

fn build_word_hints(
    fp: &FingerprintReport,
    validation: &ValidationReport,
    reservoir: &[String],
) -> Vec<WordHint> {
    let mut hints = Vec::new();
    let zipf_fail = validation
        .metric_results
        .iter()
        .any(|m| m.metric == "zipf_exponent" && !m.passed);
    if zipf_fail {
        if let Some((word, count)) = fp.word_freq.iter().max_by_key(|(_, c)| *c) {
            hints.push(WordHint {
                word: word.clone(),
                reason: format!("Zipf 峰值 count={count}"),
                action: "减到更均匀".into(),
            });
        }
    }
    let hapax_fail = validation
        .metric_results
        .iter()
        .any(|m| m.metric == "hapax_ratio" && !m.passed);
    if hapax_fail {
        if let Some(term) = reservoir.iter().find(|t| t.chars().count() >= 2) {
            hints.push(WordHint {
                word: term.clone(),
                reason: "hapax 未过关".into(),
                action: "repeat_term 提升 hapax".into(),
            });
        }
    }
    hints
}
