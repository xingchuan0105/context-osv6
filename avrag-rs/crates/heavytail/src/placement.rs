use crate::metrics::{autocorr_lag1, FingerprintReport};
use crate::score::quantile_targets;
use crate::StyleParams;
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

pub const AUTOCORR_PLACEMENT_TARGET: (f64, f64) = (0.2, 0.5);

#[derive(Debug, Clone, PartialEq)]
pub struct PlacementPlan {
    pub edits: Vec<(usize, usize)>,
    pub planned_autocorr: f64,
}

fn planned_lengths(base: &[usize], edits: &[(usize, usize)]) -> Vec<usize> {
    let mut out = base.to_vec();
    for &(idx, target) in edits {
        if idx < out.len() {
            out[idx] = target;
        }
    }
    out
}

fn autocorr_in_band(ac: f64) -> bool {
    ac >= AUTOCORR_PLACEMENT_TARGET.0 && ac <= AUTOCORR_PLACEMENT_TARGET.1
}

fn assignment_score(
    base_lengths: &[usize],
    para_of: &[usize],
    assignments: &[(usize, usize)],
) -> f64 {
    let planned = planned_lengths(base_lengths, assignments);
    let ac = autocorr_lag1(&planned);

    let mut score = if autocorr_in_band(ac) {
        1000.0
    } else {
        let dist = if ac < AUTOCORR_PLACEMENT_TARGET.0 {
            AUTOCORR_PLACEMENT_TARGET.0 - ac
        } else {
            ac - AUTOCORR_PLACEMENT_TARGET.1
        };
        500.0 - dist * 200.0
    };

    // Within-paragraph clustering: reward similar target lengths in the same paragraph.
    if !assignments.is_empty() {
        let mut para_targets: std::collections::BTreeMap<usize, Vec<usize>> =
            std::collections::BTreeMap::new();
        for &(sent_idx, target) in assignments {
            let para = para_of.get(sent_idx).copied().unwrap_or(0);
            para_targets.entry(para).or_default().push(target);
        }
        for targets in para_targets.values() {
            if targets.len() < 2 {
                continue;
            }
            let mean = targets.iter().sum::<usize>() as f64 / targets.len() as f64;
            let variance = targets
                .iter()
                .map(|&t| {
                    let d = t as f64 - mean;
                    d * d
                })
                .sum::<f64>()
                / targets.len() as f64;
            score -= variance * 0.5;
        }
    }

    score
}

fn optimize_assignments(
    base_lengths: &[usize],
    para_of: &[usize],
    edit_indices: &[usize],
    mut targets: Vec<usize>,
    seed: u64,
) -> Vec<(usize, usize)> {
    if edit_indices.is_empty() {
        return Vec::new();
    }

    targets.sort_unstable();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Initial assignment: pair sorted edit indices with sorted targets, clustered by paragraph.
    let mut sorted_edits = edit_indices.to_vec();
    sorted_edits.sort_by_key(|&idx| para_of.get(idx).copied().unwrap_or(0));
    let mut assignments: Vec<(usize, usize)> = sorted_edits
        .iter()
        .zip(targets.iter())
        .map(|(&idx, &t)| (idx, t))
        .collect();

    let mut best = assignments.clone();
    let mut best_score = assignment_score(base_lengths, para_of, &best);

    // Hill-climb with deterministic tie-breaking via seeded RNG.
    for _ in 0..512 {
        let mut improved = false;
        for i in 0..assignments.len() {
            for j in (i + 1)..assignments.len() {
                let mut candidate = assignments.clone();
                candidate.swap(i, j);
                let cand_score = assignment_score(base_lengths, para_of, &candidate);
                if cand_score > best_score + 1e-9
                    || ((cand_score - best_score).abs() < 1e-9
                        && rng.gen_bool(0.5))
                {
                    if cand_score >= best_score {
                        best = candidate.clone();
                        best_score = cand_score;
                        assignments = candidate;
                        improved = true;
                        let planned = planned_lengths(base_lengths, &best);
                        if autocorr_in_band(autocorr_lag1(&planned)) {
                            return best;
                        }
                    }
                }
            }
        }
        if !improved {
            break;
        }
    }

    // Random restart swaps for local optima escape (deterministic).
    for _ in 0..64 {
        if assignments.len() < 2 {
            break;
        }
        let mut idxs: Vec<usize> = (0..assignments.len()).collect();
        idxs.shuffle(&mut rng);
        let mut candidate = assignments.clone();
        candidate.swap(idxs[0], idxs[1]);
        let cand_score = assignment_score(base_lengths, para_of, &candidate);
        if cand_score > best_score {
            best = candidate;
            best_score = cand_score;
            assignments = best.clone();
        }
    }

    best
}

/// Choose ≤ K edit positions (largest |gap|), assign quantile targets, permute for burstiness.
pub fn plan_placement(
    fp: &FingerprintReport,
    para_of: &[usize],
    style: &StyleParams,
    k: usize,
    seed: u64,
) -> PlacementPlan {
    let n = fp.sentence_lengths.len();
    if n == 0 || k == 0 {
        return PlacementPlan {
            edits: vec![],
            planned_autocorr: 0.0,
        };
    }

    let targets = quantile_targets(n, style);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| fp.sentence_lengths[i]);

    let mut gaps: Vec<(usize, f64, usize)> = order
        .iter()
        .enumerate()
        .map(|(sorted_pos, &sent_idx)| {
            let gap = (fp.sentence_lengths[sent_idx] as f64 - targets[sorted_pos]).abs();
            (sent_idx, gap, sorted_pos)
        })
        .collect();
    gaps.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let take = k.min(n);
    let edit_indices: Vec<usize> = gaps.iter().take(take).map(|(idx, _, _)| *idx).collect();
    let edit_targets: Vec<usize> = gaps
        .iter()
        .take(take)
        .map(|(_, _, sorted_pos)| targets[*sorted_pos].round() as usize)
        .collect();

    let edits = optimize_assignments(
        &fp.sentence_lengths,
        para_of,
        &edit_indices,
        edit_targets,
        seed,
    );
    let planned = planned_lengths(&fp.sentence_lengths, &edits);
    let planned_autocorr = autocorr_lag1(&planned);

    PlacementPlan {
        edits,
        planned_autocorr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::length_metrics;
    use std::collections::BTreeMap;

    fn uniform_fp(n: usize, len: usize, para_of: &[usize]) -> (FingerprintReport, Vec<usize>) {
        let lengths = vec![len; n];
        let (mean_length, cv, autocorr_lag1, lognormal_ks_stat) = length_metrics(&lengths);
        let fp = FingerprintReport {
            sentence_lengths: lengths,
            mean_length,
            cv,
            autocorr_lag1,
            lognormal_ks_stat,
            total_tokens: 100,
            vocab_size: 50,
            ttr: 0.5,
            hapax_ratio: 0.35,
            zipf_exponent: 1.0,
            word_freq: BTreeMap::new(),
        };
        (fp, para_of.to_vec())
    }

    #[test]
    fn uniform_draft_plan_has_extremes_and_paragraph_clustering() {
        let n = 24;
        let para_of: Vec<usize> = (0..n).map(|i| i / 6).collect();
        let (fp, paras) = uniform_fp(n, 20, &para_of);
        let style = StyleParams::default();
        let plan = plan_placement(&fp, &paras, &style, 8, 42);

        assert!(!plan.edits.is_empty());
        assert!(plan.edits.len() <= 8);

        let target_lens: Vec<usize> = plan.edits.iter().map(|(_, t)| *t).collect();
        let has_short = target_lens.iter().any(|&t| t <= 12);
        let has_long = target_lens.iter().any(|&t| t >= 56);
        assert!(has_short, "expected a very-short target, got {target_lens:?}");
        assert!(has_long, "expected a long target, got {target_lens:?}");

        let mut para_bucket: std::collections::BTreeMap<usize, Vec<usize>> =
            std::collections::BTreeMap::new();
        for &(sent_idx, target) in &plan.edits {
            para_bucket
                .entry(paras[sent_idx])
                .or_default()
                .push(target);
        }
        let clustered = para_bucket.values().any(|ts| ts.len() >= 2);
        assert!(clustered, "expected within-paragraph clustering");

        assert!(
            plan.planned_autocorr >= AUTOCORR_PLACEMENT_TARGET.0 - 0.15
                && plan.planned_autocorr <= AUTOCORR_PLACEMENT_TARGET.1 + 0.15,
            "planned autocorr {} outside relaxed band",
            plan.planned_autocorr
        );
    }

    #[test]
    fn plan_is_deterministic_for_seed() {
        let para_of: Vec<usize> = (0..20).map(|i| i / 5).collect();
        let (fp, paras) = uniform_fp(20, 20, &para_of);
        let style = StyleParams::default();
        let a = plan_placement(&fp, &paras, &style, 6, 99);
        let b = plan_placement(&fp, &paras, &style, 6, 99);
        assert_eq!(a, b);
    }
}
