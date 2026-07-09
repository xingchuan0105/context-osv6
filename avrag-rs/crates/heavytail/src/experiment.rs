//! M3 three-arm experiment: metric aggregation and decision-rule rendering (spec §16).

use crate::metrics::FingerprintReport;
use crate::score::Score;
use crate::validator::ValidationReport;

/// Mean absolute difference below which two arms are treated as equivalent on CV.
pub const CV_RETIRE_THRESHOLD: f64 = 0.05;

/// Hapax ratio difference treated as "similar" for feedforward retirement.
pub const HAPAX_SIMILAR_THRESHOLD: f64 = 0.05;

/// Per-metric gap treated as negligible when comparing arms a vs b.
pub const METRIC_NEGLIGIBLE_THRESHOLD: f64 = 0.05;

/// Fraction of topics that must pass all bands for "MPC hints sufficient".
pub const MPC_SUFFICIENT_PASS_RATE: f64 = 0.8;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TopicRunResult {
    pub topic_idx: usize,
    pub topic: String,
    pub draft_path: String,
    pub fingerprint: FingerprintReport,
    pub score: Score,
    pub validation: ValidationReport,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArmSummary {
    pub arm: String,
    pub topic_count: usize,
    pub mean_cv: f64,
    pub mean_hapax: f64,
    pub mean_burst: f64,
    pub mean_zipf: f64,
    pub mean_composite_s: f64,
    pub bands_pass_count: usize,
    pub bands_pass_rate: f64,
}

impl ArmSummary {
    pub fn from_results(arm: &str, results: &[TopicRunResult]) -> Self {
        let n = results.len().max(1) as f64;
        let topic_count = results.len();
        let mean_cv = results.iter().map(|r| r.fingerprint.cv).sum::<f64>() / n;
        let mean_hapax = results.iter().map(|r| r.fingerprint.hapax_ratio).sum::<f64>() / n;
        let mean_burst = results.iter().map(|r| r.fingerprint.autocorr_lag1).sum::<f64>() / n;
        let mean_zipf = results.iter().map(|r| r.fingerprint.zipf_exponent).sum::<f64>() / n;
        let mean_composite_s = results.iter().map(|r| r.score.s).sum::<f64>() / n;
        let bands_pass_count = results
            .iter()
            .filter(|r| r.validation.passed)
            .count();
        let bands_pass_rate = if topic_count == 0 {
            0.0
        } else {
            bands_pass_count as f64 / topic_count as f64
        };

        Self {
            arm: arm.to_string(),
            topic_count,
            mean_cv,
            mean_hapax,
            mean_burst,
            mean_zipf,
            mean_composite_s,
            bands_pass_count,
            bands_pass_rate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M3Verdict {
    pub rule: &'static str,
    pub verdict: String,
    pub triggered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct M3DecisionReport {
    pub verdicts: Vec<M3Verdict>,
    pub retire_feedforward: bool,
    pub mpc_hints_sufficient: bool,
    pub drop_deficit_hints: bool,
}

pub fn mean_abs_diff(a: f64, b: f64) -> f64 {
    (a - b).abs()
}

pub fn metrics_negligible(a: &ArmSummary, b: &ArmSummary) -> bool {
    mean_abs_diff(a.mean_cv, b.mean_cv) < METRIC_NEGLIGIBLE_THRESHOLD
        && mean_abs_diff(a.mean_hapax, b.mean_hapax) < METRIC_NEGLIGIBLE_THRESHOLD
        && mean_abs_diff(a.mean_burst, b.mean_burst) < METRIC_NEGLIGIBLE_THRESHOLD
        && mean_abs_diff(a.mean_zipf, b.mean_zipf) < METRIC_NEGLIGIBLE_THRESHOLD
}

pub fn evaluate_m3_decisions(
    arm_a: &ArmSummary,
    arm_b: &ArmSummary,
    arm_c: &ArmSummary,
) -> M3DecisionReport {
    let cv_gap = mean_abs_diff(arm_b.mean_cv, arm_c.mean_cv);
    let hapax_gap = mean_abs_diff(arm_b.mean_hapax, arm_c.mean_hapax);
    let retire_feedforward =
        cv_gap < CV_RETIRE_THRESHOLD && hapax_gap < HAPAX_SIMILAR_THRESHOLD;

    let mpc_hints_sufficient = arm_b.bands_pass_rate >= MPC_SUFFICIENT_PASS_RATE;

    let drop_deficit_hints = metrics_negligible(arm_a, arm_b);

    let verdicts = vec![
        M3Verdict {
            rule: "if mean|CV_b − CV_c| < 0.05 and hapax similar → RETIRE feedforward (arm C)",
            verdict: if retire_feedforward {
                "RETIRE feedforward (arm C)".to_string()
            } else {
                "KEEP feedforward (arm C) — CV or hapax still diverges".to_string()
            },
            triggered: retire_feedforward,
        },
        M3Verdict {
            rule: "if arm b + 0 rounds already in bands → MPC hints sufficient",
            verdict: if mpc_hints_sufficient {
                format!(
                    "MPC hints sufficient ({}/{} topics pass all bands at draft)",
                    arm_b.bands_pass_count, arm_b.topic_count
                )
            } else {
                format!(
                    "MPC hints NOT sufficient ({}/{} pass; need ≥ {:.0}% )",
                    arm_b.bands_pass_count,
                    arm_b.topic_count,
                    MPC_SUFFICIENT_PASS_RATE * 100.0
                )
            },
            triggered: mpc_hints_sufficient,
        },
        M3Verdict {
            rule: "if |metrics_a − metrics_b| negligible → DROP deficit hints",
            verdict: if drop_deficit_hints {
                "DROP deficit hints — arm a ≈ arm b on core metrics".to_string()
            } else {
                "KEEP deficit hints — arm b measurably differs from arm a".to_string()
            },
            triggered: drop_deficit_hints,
        },
    ];

    M3DecisionReport {
        verdicts,
        retire_feedforward,
        mpc_hints_sufficient,
        drop_deficit_hints,
    }
}

pub fn render_summary_markdown(
    run_id: &str,
    topics: &[String],
    arms: &[ArmSummary],
    arm_b_lines: Option<&ArmSummary>,
    decisions: &M3DecisionReport,
) -> String {
    let mut out = String::new();
    out.push_str("# HeavyTail M3 Experiment Summary\n\n");
    out.push_str(&format!("Run: `{run_id}`\n"));
    out.push_str(&format!("Topics: {}\n\n", topics.len()));

    out.push_str("## Per-arm metric means\n\n");
    out.push_str("| arm | n | mean CV | mean hapax | mean burst | mean zipf | mean S | bands pass |\n");
    out.push_str("|-----|---|---------|------------|------------|-----------|--------|------------|\n");
    for arm in arms {
        out.push_str(&format!(
            "| {} | {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {}/{} ({:.0}%) |\n",
            arm.arm,
            arm.topic_count,
            arm.mean_cv,
            arm.mean_hapax,
            arm.mean_burst,
            arm.mean_zipf,
            arm.mean_composite_s,
            arm.bands_pass_count,
            arm.topic_count,
            arm.bands_pass_rate * 100.0
        ));
    }
    if let Some(lines) = arm_b_lines {
        out.push_str(&format!(
            "| {} (R4 A/B) | {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {}/{} ({:.0}%) |\n",
            lines.arm,
            lines.topic_count,
            lines.mean_cv,
            lines.mean_hapax,
            lines.mean_burst,
            lines.mean_zipf,
            lines.mean_composite_s,
            lines.bands_pass_count,
            lines.topic_count,
            lines.bands_pass_rate * 100.0
        ));
    }

    out.push_str("\n## M3 decision rules (spec §16)\n\n");
    for v in &decisions.verdicts {
        out.push_str(&format!("- **Rule:** {}\n", v.rule));
        out.push_str(&format!("  - **Verdict:** {}\n", v.verdict));
    }

    out.push_str("\n## Recommended actions\n\n");
    if decisions.retire_feedforward {
        out.push_str("- Retire arm-C feedforward scheduler (`feedforward.rs` production path).\n");
    } else {
        out.push_str("- Keep arm-C feedforward for further comparison or M4 refinement.\n");
    }
    if decisions.mpc_hints_sufficient {
        out.push_str("- Default drafting: priming + MPC deficit hints (arm b).\n");
    } else {
        out.push_str("- MPC hints alone insufficient at draft — plan M4 refinement on arm-b outputs.\n");
    }
    if decisions.drop_deficit_hints {
        out.push_str("- Drop MPC deficit hints; priming-only or plain drafting may suffice.\n");
    } else {
        out.push_str("- Keep MPC deficit hints enabled.\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_result(cv: f64, hapax: f64, burst: f64, zipf: f64, s: f64, passed: bool) -> TopicRunResult {
        TopicRunResult {
            topic_idx: 1,
            topic: "测试".into(),
            draft_path: "arm-a/topic-01.draft.txt".into(),
            fingerprint: FingerprintReport {
                sentence_lengths: vec![10, 20, 30],
                mean_length: 20.0,
                cv,
                autocorr_lag1: burst,
                lognormal_ks_stat: 0.1,
                total_tokens: 30,
                vocab_size: 20,
                ttr: 0.6,
                hapax_ratio: hapax,
                zipf_exponent: zipf,
                word_freq: BTreeMap::new(),
            },
            score: Score {
                s,
                len: 0.5,
                burst: 0.5,
                hapax: 0.5,
                zipf: 0.5,
            },
            validation: ValidationReport {
                fingerprint: FingerprintReport {
                    sentence_lengths: vec![10, 20, 30],
                    mean_length: 20.0,
                    cv,
                    autocorr_lag1: burst,
                    lognormal_ks_stat: 0.1,
                    total_tokens: 30,
                    vocab_size: 20,
                    ttr: 0.6,
                    hapax_ratio: hapax,
                    zipf_exponent: zipf,
                    word_freq: BTreeMap::new(),
                },
                passed,
                metric_results: vec![],
            },
        }
    }

    fn arm_from(values: &[(f64, f64, f64, f64, f64, bool)], name: &str) -> ArmSummary {
        let results: Vec<_> = values
            .iter()
            .map(|&(cv, hapax, burst, zipf, s, passed)| {
                sample_result(cv, hapax, burst, zipf, s, passed)
            })
            .collect();
        ArmSummary::from_results(name, &results)
    }

    #[test]
    fn retire_feedforward_when_b_and_c_match() {
        let b = arm_from(&[(0.70, 0.40, 0.30, 1.0, 0.6, true); 10], "b");
        let c = arm_from(&[(0.72, 0.41, 0.31, 1.01, 0.58, false); 10], "c");
        let a = arm_from(&[(0.50, 0.30, 0.20, 0.9, 0.4, false); 10], "a");
        let report = evaluate_m3_decisions(&a, &b, &c);
        assert!(report.retire_feedforward);
        assert!(report
            .verdicts
            .iter()
            .any(|v| v.verdict.contains("RETIRE feedforward")));
    }

    #[test]
    fn keep_feedforward_when_cv_diverges() {
        let b = arm_from(&[(0.70, 0.40, 0.30, 1.0, 0.6, true); 10], "b");
        let c = arm_from(&[(0.90, 0.41, 0.30, 1.0, 0.6, false); 10], "c");
        let a = arm_from(&[(0.50, 0.30, 0.20, 0.9, 0.4, false); 10], "a");
        let report = evaluate_m3_decisions(&a, &b, &c);
        assert!(!report.retire_feedforward);
    }

    #[test]
    fn mpc_sufficient_when_enough_band_passes() {
        let b = arm_from(&[
            (0.7, 0.4, 0.3, 1.0, 0.6, true),
            (0.7, 0.4, 0.3, 1.0, 0.6, true),
            (0.7, 0.4, 0.3, 1.0, 0.6, true),
            (0.7, 0.4, 0.3, 1.0, 0.6, true),
            (0.7, 0.4, 0.3, 1.0, 0.6, false),
        ], "b");
        let c = arm_from(&[(0.7, 0.4, 0.3, 1.0, 0.6, false); 5], "c");
        let a = arm_from(&[(0.5, 0.3, 0.2, 0.9, 0.4, false); 5], "a");
        let report = evaluate_m3_decisions(&a, &b, &c);
        assert!(report.mpc_hints_sufficient);
    }

    #[test]
    fn drop_deficit_hints_when_a_equals_b() {
        let a = arm_from(&[(0.70, 0.40, 0.30, 1.0, 0.6, false); 5], "a");
        let b = arm_from(&[(0.71, 0.405, 0.305, 1.005, 0.61, true); 5], "b");
        let c = arm_from(&[(0.90, 0.35, 0.25, 0.95, 0.5, false); 5], "c");
        let report = evaluate_m3_decisions(&a, &b, &c);
        assert!(report.drop_deficit_hints);
    }

    #[test]
    fn summary_markdown_includes_verdict_lines() {
        let a = arm_from(&[(0.5, 0.3, 0.2, 0.9, 0.4, false); 2], "a");
        let b = arm_from(&[(0.7, 0.4, 0.3, 1.0, 0.6, true); 2], "b");
        let c = arm_from(&[(0.71, 0.41, 0.31, 1.01, 0.58, false); 2], "c");
        let decisions = evaluate_m3_decisions(&a, &b, &c);
        let md = render_summary_markdown(
            "20260706-120000",
            &["主题一".into(), "主题二".into()],
            &[a, b, c],
            None,
            &decisions,
        );
        assert!(md.contains("# HeavyTail M3 Experiment Summary"));
        assert!(md.contains("M3 decision rules"));
        assert!(md.contains("RETIRE feedforward"));
        assert!(md.contains("| arm | n | mean CV |"));
    }
}
