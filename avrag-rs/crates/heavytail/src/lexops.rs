use crate::metrics::{with_word_freq, zipf_expected, FingerprintReport};
use crate::score::composite;
use crate::StyleParams;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum LexOp {
    Promote {
        word: String,
        replacement_pool: Vec<String>,
        delta_s: f64,
    },
    Demote {
        word: String,
        current: usize,
        max_count: usize,
        delta_s: f64,
    },
}

/// Simulate one Promote op: one occurrence of `word` replaced by `replacement`.
pub fn apply_promote(
    word_freq: &BTreeMap<String, usize>,
    word: &str,
    replacement: &str,
) -> BTreeMap<String, usize> {
    let mut next = word_freq.clone();
    if let Some(c) = next.get_mut(word) {
        *c -= 1;
        if *c == 0 {
            next.remove(word);
        }
    }
    *next.entry(replacement.to_string()).or_default() += 1;
    next
}

/// Promote identity from spec §3.4: hapax types +2, vocab +1.
pub fn promote_lexical_effect(
    word_freq: &BTreeMap<String, usize>,
    word: &str,
    replacement: &str,
) -> (usize, usize) {
    let before_hapax = word_freq.values().filter(|&&c| c == 1).count();
    let before_vocab = word_freq.len();
    let after = apply_promote(word_freq, word, replacement);
    let after_hapax = after.values().filter(|&&c| c == 1).count();
    let after_vocab = after.len();
    (
        after_hapax.saturating_sub(before_hapax),
        after_vocab.saturating_sub(before_vocab),
    )
}

fn delta_s_for_word_freq(
    fp: &FingerprintReport,
    style: &StyleParams,
    word_freq: BTreeMap<String, usize>,
) -> f64 {
    let base = composite(fp, style).s;
    let modified = with_word_freq(fp, word_freq);
    composite(&modified, style).s - base
}

/// reservoir = candidate rare words filtered to words absent from the draft.
pub fn enumerate_lexops(
    fp: &FingerprintReport,
    reservoir: &[String],
    style: &StyleParams,
) -> Vec<LexOp> {
    let replacement_pool: Vec<String> = reservoir
        .iter()
        .filter(|w| !fp.word_freq.contains_key(w.as_str()))
        .cloned()
        .collect();

    let mut ops = Vec::new();

    if !replacement_pool.is_empty() {
        let mut freq2: Vec<String> = fp
            .word_freq
            .iter()
            .filter(|(_, c)| **c == 2)
            .map(|(w, _)| w.clone())
            .collect();
        freq2.sort();

        for word in freq2 {
            let next = apply_promote(&fp.word_freq, &word, &replacement_pool[0]);
            let delta_s = delta_s_for_word_freq(fp, style, next);
            ops.push(LexOp::Promote {
                word,
                replacement_pool: replacement_pool.clone(),
                delta_s,
            });
        }

        let mut freq3: Vec<String> = fp
            .word_freq
            .iter()
            .filter(|(_, c)| **c == 3)
            .map(|(w, _)| w.clone())
            .collect();
        freq3.sort();

        for word in freq3 {
            let next = apply_promote(&fp.word_freq, &word, &replacement_pool[0]);
            let delta_s = delta_s_for_word_freq(fp, style, next);
            ops.push(LexOp::Promote {
                word,
                replacement_pool: replacement_pool.clone(),
                delta_s,
            });
        }
    }

    let mut ranked: Vec<(String, usize)> = fp
        .word_freq
        .iter()
        .map(|(w, &c)| (w.clone(), c))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    for (rank_idx, (word, current)) in ranked.into_iter().enumerate() {
        let rank = rank_idx + 1;
        let expected = zipf_expected(
            rank,
            fp.total_tokens,
            fp.vocab_size,
            style.zipf_exponent,
        );
        if current as f64 > expected * 2.0 {
            let max_count = expected.ceil().max(1.0) as usize;
            let mut next = fp.word_freq.clone();
            *next.get_mut(&word).unwrap() = max_count;
            let delta_s = delta_s_for_word_freq(fp, style, next);
            ops.push(LexOp::Demote {
                word,
                current,
                max_count,
                delta_s,
            });
        }
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{length_metrics, FingerprintReport};
    use std::collections::BTreeMap;

    fn fp_from_freq(word_freq: BTreeMap<String, usize>, lengths: Vec<usize>) -> FingerprintReport {
        let (mean_length, cv, autocorr_lag1, lognormal_ks_stat) = length_metrics(&lengths);
        let total_tokens: usize = word_freq.values().sum();
        let vocab_size = word_freq.len();
        let hapax_ratio = if vocab_size == 0 {
            0.0
        } else {
            word_freq.values().filter(|&&c| c == 1).count() as f64 / vocab_size as f64
        };
        FingerprintReport {
            sentence_lengths: lengths,
            mean_length,
            cv,
            autocorr_lag1,
            lognormal_ks_stat,
            total_tokens,
            vocab_size,
            ttr: if total_tokens == 0 {
                0.0
            } else {
                vocab_size as f64 / total_tokens as f64
            },
            hapax_ratio,
            zipf_exponent: 1.0,
            word_freq,
        }
    }

    #[test]
    fn promote_yields_hapax_plus_two_vocab_plus_one() {
        let mut freq = BTreeMap::new();
        freq.insert("重复词".into(), 2);
        freq.insert("已有词".into(), 1);
        freq.insert("其他".into(), 3);

        let (dh, dv) = promote_lexical_effect(&freq, "重复词", "罕见词");
        assert_eq!(dh, 2, "hapax types should increase by 2");
        assert_eq!(dv, 1, "vocab should increase by 1");
    }

    #[test]
    fn enumerate_promote_ops_sorted_freq2_before_freq3() {
        let mut freq = BTreeMap::new();
        freq.insert("双频".into(), 2);
        freq.insert("三频".into(), 3);
        freq.insert("单频".into(), 1);
        let fp = fp_from_freq(freq, vec![20; 5]);
        let style = StyleParams::default();
        let ops = enumerate_lexops(&fp, &["新词".into()], &style);
        let promotes: Vec<_> = ops
            .iter()
            .filter_map(|op| match op {
                LexOp::Promote { word, .. } => Some(word.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(promotes, vec!["双频", "三频"]);
    }
}
