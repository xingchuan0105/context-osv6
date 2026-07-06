//! Statistical fingerprint metrics for analyzed sentences (spec §8).

use std::collections::BTreeMap;

use crate::math::{linreg, norm_cdf};
use crate::segment::char_len;
use crate::tokenize::{is_content_word, tokens};

/// Full fingerprint of a draft's sentence-length and lexical statistics.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FingerprintReport {
    pub sentence_lengths: Vec<usize>,
    pub mean_length: f64,
    pub cv: f64,
    pub autocorr_lag1: f64,
    /// Report-only KS statistic vs fitted log-normal; never used for targeting.
    pub lognormal_ks_stat: f64,
    pub total_tokens: usize,
    pub vocab_size: usize,
    pub ttr: f64,
    pub hapax_ratio: f64,
    pub zipf_exponent: f64,
    /// Content-word frequencies only.
    pub word_freq: BTreeMap<String, usize>,
}

/// Analyze pre-segmented sentences into a fingerprint report.
pub fn analyze_sentences(sentences: &[(String, usize)]) -> FingerprintReport {
    let sentence_lengths: Vec<usize> = sentences
        .iter()
        .map(|(text, _)| char_len(text))
        .collect();

    let mean_length = if sentence_lengths.is_empty() {
        0.0
    } else {
        sentence_lengths.iter().sum::<usize>() as f64 / sentence_lengths.len() as f64
    };

    let cv = coefficient_of_variation(&sentence_lengths);
    let autocorr_lag1 = autocorr_lag1(&sentence_lengths);
    let lognormal_ks_stat = lognormal_ks_stat(&sentence_lengths);

    let word_freq = content_word_freq(sentences);
    let total_tokens: usize = word_freq.values().sum();
    let vocab_size = word_freq.len();
    let ttr = if total_tokens == 0 {
        0.0
    } else {
        vocab_size as f64 / total_tokens as f64
    };
    let hapax_ratio = hapax_ratio(&word_freq);
    let zipf_exponent = zipf_exponent(&word_freq);

    FingerprintReport {
        sentence_lengths,
        mean_length,
        cv,
        autocorr_lag1,
        lognormal_ks_stat,
        total_tokens,
        vocab_size,
        ttr,
        hapax_ratio,
        zipf_exponent,
        word_freq,
    }
}

/// Recompute length metrics from a length vector.
pub fn length_metrics(lengths: &[usize]) -> (f64, f64, f64, f64) {
    let mean_length = if lengths.is_empty() {
        0.0
    } else {
        lengths.iter().sum::<usize>() as f64 / lengths.len() as f64
    };
    (
        mean_length,
        coefficient_of_variation(lengths),
        autocorr_lag1(lengths),
        lognormal_ks_stat(lengths),
    )
}

/// Clone a report with swapped sentence lengths and recomputed length metrics.
pub fn with_lengths(fp: &FingerprintReport, lengths: &[usize]) -> FingerprintReport {
    let (mean_length, cv, autocorr_lag1, lognormal_ks_stat) = length_metrics(lengths);
    FingerprintReport {
        sentence_lengths: lengths.to_vec(),
        mean_length,
        cv,
        autocorr_lag1,
        lognormal_ks_stat,
        ..fp.clone()
    }
}

/// Clone a report with swapped word frequencies and recomputed lexical metrics.
pub fn with_word_freq(fp: &FingerprintReport, word_freq: BTreeMap<String, usize>) -> FingerprintReport {
    let total_tokens: usize = word_freq.values().sum();
    let vocab_size = word_freq.len();
    let ttr = if total_tokens == 0 {
        0.0
    } else {
        vocab_size as f64 / total_tokens as f64
    };
    FingerprintReport {
        total_tokens,
        vocab_size,
        ttr,
        hapax_ratio: hapax_ratio(&word_freq),
        zipf_exponent: zipf_exponent(&word_freq),
        word_freq,
        ..fp.clone()
    }
}

fn content_word_freq(sentences: &[(String, usize)]) -> BTreeMap<String, usize> {
    let mut freq = BTreeMap::new();
    for (text, _) in sentences {
        for token in tokens(text) {
            if is_content_word(&token) {
                *freq.entry(token).or_insert(0) += 1;
            }
        }
    }
    freq
}

/// Population CV = stddev / mean; returns 0 when n < 2 or mean is zero.
fn coefficient_of_variation(lengths: &[usize]) -> f64 {
    if lengths.len() < 2 {
        return 0.0;
    }
    let n = lengths.len() as f64;
    let mean = lengths.iter().sum::<usize>() as f64 / n;
    if mean == 0.0 {
        return 0.0;
    }
    let variance = lengths
        .iter()
        .map(|&l| {
            let d = l as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    variance.sqrt() / mean
}

/// Pearson correlation of `(l_1..l_{n-1})` vs `(l_2..l_n)`; zero when n < 2 or zero variance.
pub fn autocorr_lag1(lengths: &[usize]) -> f64 {
    if lengths.len() < 2 {
        return 0.0;
    }
    let xs: Vec<f64> = lengths[..lengths.len() - 1]
        .iter()
        .map(|&l| l as f64)
        .collect();
    let ys: Vec<f64> = lengths[1..].iter().map(|&l| l as f64).collect();
    pearson_corr(&xs, &ys)
}

fn pearson_corr(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len();
    if n == 0 || n != ys.len() {
        return 0.0;
    }
    let n_f = n as f64;
    let mean_x = xs.iter().sum::<f64>() / n_f;
    let mean_y = ys.iter().sum::<f64>() / n_f;

    let mut num = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for i in 0..n {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        num += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    if var_x == 0.0 || var_y == 0.0 {
        return 0.0;
    }
    num / (var_x.sqrt() * var_y.sqrt())
}

/// MLE log-normal fit on ln(lengths); KS = max |F_emp − Φ((ln x − μ̂)/σ̂)|.
fn lognormal_ks_stat(lengths: &[usize]) -> f64 {
    if lengths.len() < 2 {
        return 0.0;
    }
    if lengths.iter().any(|&l| l == 0) {
        return 0.0;
    }

    let log_lens: Vec<f64> = lengths.iter().map(|&l| (l as f64).ln()).collect();
    let n = log_lens.len() as f64;
    let mu = log_lens.iter().sum::<f64>() / n;
    let sigma = (log_lens.iter().map(|&x| (x - mu).powi(2)).sum::<f64>() / n).sqrt();
    if sigma == 0.0 {
        return 0.0;
    }

    let mut sorted = log_lens;
    sorted.sort_by(f64::total_cmp);

    let mut ks = 0.0_f64;
    for (i, &x) in sorted.iter().enumerate() {
        let i_f = (i + 1) as f64;
        let f_theo = norm_cdf((x - mu) / sigma);
        let d_plus = i_f / n - f_theo;
        let d_minus = f_theo - (i as f64 / n);
        ks = ks.max(d_plus).max(d_minus);
    }
    ks
}

/// |{w : freq(w) == 1}| / vocab_size over content words.
fn hapax_ratio(word_freq: &BTreeMap<String, usize>) -> f64 {
    if word_freq.is_empty() {
        return 0.0;
    }
    let hapax = word_freq.values().filter(|&&c| c == 1).count();
    hapax as f64 / word_freq.len() as f64
}

/// `-slope` of OLS `linreg(ln rank, ln freq)` over descending rank-frequency pairs.
fn zipf_exponent(word_freq: &BTreeMap<String, usize>) -> f64 {
    if word_freq.len() < 2 {
        return 0.0;
    }

    let mut ranked: Vec<usize> = word_freq.values().copied().collect();
    ranked.sort_by(|a, b| b.cmp(a));

    let mut ln_ranks = Vec::with_capacity(ranked.len());
    let mut ln_freqs = Vec::with_capacity(ranked.len());
    for (i, &freq) in ranked.iter().enumerate() {
        if freq == 0 {
            continue;
        }
        ln_ranks.push(((i + 1) as f64).ln());
        ln_freqs.push((freq as f64).ln());
    }

    if ln_ranks.len() < 2 {
        return 0.0;
    }

    let (_intercept, slope) = linreg(&ln_ranks, &ln_freqs);
    -slope
}

/// Expected Zipf frequency at 1-based rank (spec §3.4 demote threshold).
pub fn zipf_expected(rank: usize, total_tokens: usize, vocab_size: usize, exponent: f64) -> f64 {
    if rank == 0 || total_tokens == 0 || vocab_size == 0 {
        return 0.0;
    }
    let mut norm = 0.0;
    for i in 1..=vocab_size {
        norm += 1.0 / (i as f64).powf(exponent);
    }
    if norm < f64::EPSILON {
        return 0.0;
    }
    total_tokens as f64 * (1.0 / (rank as f64).powf(exponent)) / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentence(text: &str) -> (String, usize) {
        (text.to_string(), 0)
    }

    #[test]
    fn hand_computed_five_sentence_fixture() {
        let sentences = vec![
            sentence("1234567890"),
            sentence("12345678901234567890"),
            sentence("123456789012345"),
            sentence("1234567890123456789012345"),
            sentence("123456789012345678901234567890"),
        ];
        let fp = analyze_sentences(&sentences);

        assert_eq!(fp.sentence_lengths, vec![10, 20, 15, 25, 30]);
        assert!((fp.mean_length - 20.0).abs() < 1e-9);

        let expected_cv = (50.0_f64).sqrt() / 20.0;
        assert!((fp.cv - expected_cv).abs() < 1e-9);

        let xs = [10.0, 20.0, 15.0, 25.0];
        let ys = [20.0, 15.0, 25.0, 30.0];
        let expected_autocorr = pearson_corr(&xs, &ys);
        assert!((fp.autocorr_lag1 - expected_autocorr).abs() < 1e-9);

        assert!(fp.lognormal_ks_stat > 0.0);
        assert!(fp.lognormal_ks_stat < 1.0);
    }

    #[test]
    fn constant_lengths_zero_cv_and_autocorr() {
        let sentences: Vec<_> = (0..5).map(|_| sentence("abcd")).collect();
        let fp = analyze_sentences(&sentences);

        assert!(fp.sentence_lengths.iter().all(|&l| l == 4));
        assert_eq!(fp.cv, 0.0);
        assert_eq!(fp.autocorr_lag1, 0.0);
        assert_eq!(fp.lognormal_ks_stat, 0.0);
    }

    #[test]
    fn alternating_lengths_negative_autocorr() {
        let sentences: Vec<_> = (0..6)
            .map(|i| {
                let len = if i % 2 == 0 { 10 } else { 30 };
                ("x".repeat(len), 0)
            })
            .collect();
        let fp = analyze_sentences(&sentences);

        assert_eq!(fp.sentence_lengths, vec![10, 30, 10, 30, 10, 30]);
        assert!(fp.autocorr_lag1 < 0.0, "expected negative lag-1 autocorr");
    }

    #[test]
    fn synthetic_zipf_exponent_near_one() {
        let mut word_freq = BTreeMap::new();
        for rank in 1..=100 {
            let freq = (1000.0 / rank as f64).round() as usize;
            word_freq.insert(format!("word{rank:03}"), freq.max(1));
        }

        let exponent = zipf_exponent(&word_freq);
        assert!(
            (exponent - 1.0).abs() < 0.15,
            "zipf exponent {exponent} not within ±0.15 of 1.0"
        );
    }

    #[test]
    fn hapax_and_ttr_from_word_freq() {
        let mut word_freq = BTreeMap::new();
        word_freq.insert("alpha".into(), 3);
        word_freq.insert("beta".into(), 1);
        word_freq.insert("gamma".into(), 1);
        word_freq.insert("delta".into(), 2);

        assert!((hapax_ratio(&word_freq) - 0.5).abs() < 1e-9);

        let total: usize = word_freq.values().sum();
        let ttr = word_freq.len() as f64 / total as f64;
        assert!((ttr - 4.0 / 7.0).abs() < 1e-9);
    }

    #[test]
    fn empty_input_returns_zeros() {
        let fp = analyze_sentences(&[]);
        assert!(fp.sentence_lengths.is_empty());
        assert_eq!(fp.mean_length, 0.0);
        assert_eq!(fp.cv, 0.0);
        assert_eq!(fp.autocorr_lag1, 0.0);
        assert_eq!(fp.total_tokens, 0);
        assert_eq!(fp.vocab_size, 0);
        assert_eq!(fp.ttr, 0.0);
        assert_eq!(fp.hapax_ratio, 0.0);
        assert_eq!(fp.zipf_exponent, 0.0);
    }
}
