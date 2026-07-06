use crate::metrics::{with_lengths, FingerprintReport};
use crate::score::composite;
use crate::StyleParams;

pub const CANDIDATE_GRID: &[usize] = &[5, 8, 12, 16, 20, 26, 34, 44, 56, 72, 90];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SensitivityRow {
    pub sentence_idx: usize,
    pub current_len: usize,
    pub candidate_len: usize,
    pub delta_s: f64,
}

pub fn length_sensitivity(fp: &FingerprintReport, style: &StyleParams) -> Vec<SensitivityRow> {
    let base_s = composite(fp, style).s;
    let mut rows = Vec::new();

    for (sentence_idx, &current_len) in fp.sentence_lengths.iter().enumerate() {
        for &candidate_len in CANDIDATE_GRID {
            let mut lengths = fp.sentence_lengths.clone();
            lengths[sentence_idx] = candidate_len;
            let modified = with_lengths(fp, &lengths);
            let delta_s = composite(&modified, style).s - base_s;
            rows.push(SensitivityRow {
                sentence_idx,
                current_len,
                candidate_len,
                delta_s,
            });
        }
    }

    rows
}

/// Brute-force recompute used by tests to guard incremental-update bugs.
pub fn brute_delta_s(
    fp: &FingerprintReport,
    style: &StyleParams,
    sentence_idx: usize,
    candidate_len: usize,
) -> f64 {
    let base_s = composite(fp, style).s;
    let mut lengths = fp.sentence_lengths.clone();
    lengths[sentence_idx] = candidate_len;
    let modified = with_lengths(fp, &lengths);
    composite(&modified, style).s - base_s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{length_metrics, with_word_freq};
    use std::collections::BTreeMap;

    fn synthetic_fp(lengths: Vec<usize>, hapax: f64) -> FingerprintReport {
        let (mean_length, cv, autocorr_lag1, lognormal_ks_stat) = length_metrics(&lengths);
        let vocab = 100usize;
        let hapax_types = (hapax * vocab as f64).round() as usize;
        let mut word_freq = BTreeMap::new();
        for i in 0..hapax_types {
            word_freq.insert(format!("hapax{i}"), 1);
        }
        for i in 0..(vocab - hapax_types) {
            word_freq.insert(format!("common{i}"), 2);
        }
        let total_tokens: usize = word_freq.values().sum();
        FingerprintReport {
            sentence_lengths: lengths,
            mean_length,
            cv,
            autocorr_lag1,
            lognormal_ks_stat,
            total_tokens,
            vocab_size: vocab,
            ttr: vocab as f64 / total_tokens as f64,
            hapax_ratio: hapax,
            zipf_exponent: 1.0,
            word_freq,
        }
    }

    #[test]
    fn delta_s_matches_brute_recompute() {
        let style = StyleParams::default();
        let fp = synthetic_fp(vec![20; 30], 0.35);
        let rows = length_sensitivity(&fp, &style);
        for row in rows {
            let brute = brute_delta_s(&fp, &style, row.sentence_idx, row.candidate_len);
            assert!(
                (row.delta_s - brute).abs() < 1e-12,
                "idx={} cand={}: table={} brute={}",
                row.sentence_idx,
                row.candidate_len,
                row.delta_s,
                brute
            );
        }
    }

    #[test]
    fn uniform_draft_best_rows_push_toward_extremes() {
        let style = StyleParams::default();
        let fp = synthetic_fp(vec![20; 40], 0.35);
        let rows = length_sensitivity(&fp, &style);

        for sentence_idx in 0..fp.sentence_lengths.len() {
            let sentence_rows: Vec<_> = rows
                .iter()
                .filter(|r| r.sentence_idx == sentence_idx)
                .collect();
            let best = sentence_rows
                .iter()
                .max_by(|a, b| {
                    a.delta_s
                        .partial_cmp(&b.delta_s)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            let grid_min = *CANDIDATE_GRID.first().unwrap();
            let grid_max = *CANDIDATE_GRID.last().unwrap();
            assert!(
                best.candidate_len == grid_min || best.candidate_len == grid_max,
                "sentence {sentence_idx}: best candidate {} not at grid extreme",
                best.candidate_len
            );
        }
    }

    #[test]
    fn brute_matches_with_word_freq_unchanged() {
        let style = StyleParams::default();
        let mut freq = BTreeMap::new();
        freq.insert("alpha".into(), 2);
        freq.insert("beta".into(), 1);
        let base = synthetic_fp(vec![15, 25, 35, 45], 0.4);
        let fp = with_word_freq(&base, freq);
        let row = length_sensitivity(&fp, &style)
            .into_iter()
            .find(|r| r.sentence_idx == 2 && r.candidate_len == 72)
            .unwrap();
        assert!((row.delta_s - brute_delta_s(&fp, &style, 2, 72)).abs() < 1e-12);
    }
}
