use crate::math::inv_norm_cdf;
use crate::metrics::FingerprintReport;
use crate::StyleParams;

pub const L_MIN: f64 = 5.0;
pub const L_MAX: f64 = 100.0;

/// q_(i) = exp(μ_z + σ_z · Φ⁻¹((i−0.5)/n)), clamped to [L_MIN, L_MAX].
pub fn quantile_targets(n: usize, style: &StyleParams) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let sigma_z = (1.0 + style.cv * style.cv).ln().sqrt();
    let mu_z = style.median_length.ln();
    (0..n)
        .map(|i| {
            let p = (i as f64 + 0.5) / n as f64;
            let q = (mu_z + sigma_z * inv_norm_cdf(p)).exp();
            q.clamp(L_MIN, L_MAX)
        })
        .collect()
}

/// W1 = mean |sorted(lengths) − targets|.
pub fn w1(lengths: &[usize], targets: &[f64]) -> f64 {
    let n = lengths.len().min(targets.len());
    if n == 0 {
        return 0.0;
    }
    let mut sorted = lengths[..n].to_vec();
    sorted.sort_unstable();
    sorted
        .iter()
        .zip(&targets[..n])
        .map(|(&l, &t)| (l as f64 - t).abs())
        .sum::<f64>()
        / n as f64
}

pub fn expected_mean_length(style: &StyleParams) -> f64 {
    let sigma_z = (1.0 + style.cv * style.cv).ln().sqrt();
    style.median_length * (sigma_z * sigma_z / 2.0).exp()
}

/// Normalized W1 = W1 / E[l].
pub fn w1_normalized(lengths: &[usize], style: &StyleParams) -> f64 {
    let targets = quantile_targets(lengths.len(), style);
    let raw = w1(lengths, &targets);
    let denom = expected_mean_length(style);
    if denom < f64::EPSILON {
        0.0
    } else {
        raw / denom
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Bands {
    pub target: (f64, f64),
    pub hard: (f64, f64),
}

/// 1.0 inside target band; linear decay to 0.0 at the hard bound; 0.0 beyond.
pub fn band_score(x: f64, b: &Bands) -> f64 {
    let (t_lo, t_hi) = b.target;
    let (h_lo, h_hi) = b.hard;
    if x >= t_lo && x <= t_hi {
        return 1.0;
    }
    if x < h_lo || x > h_hi {
        return 0.0;
    }
    if x < t_lo {
        ((x - h_lo) / (t_lo - h_lo)).clamp(0.0, 1.0)
    } else {
        ((h_hi - x) / (h_hi - t_hi)).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    pub s: f64,
    pub len: f64,
    pub burst: f64,
    pub hapax: f64,
    pub zipf: f64,
}

pub fn bands_for(style: &StyleParams) -> [(&'static str, Bands); 4] {
    [
        (
            "cv",
            Bands {
                target: (style.cv * 0.85, style.cv * 1.15),
                hard: (0.50, 1.00),
            },
        ),
        (
            "hapax",
            Bands {
                target: (0.35, 0.55),
                hard: (0.30, 1.0),
            },
        ),
        (
            "burst",
            Bands {
                target: (0.1, 0.6),
                hard: (0.0, 0.8),
            },
        ),
        (
            "zipf",
            Bands {
                target: (0.8, 1.3),
                hard: (0.6, 1.5),
            },
        ),
    ]
}

/// Weights 0.4/0.2/0.25/0.15; len = clamp(1 − Ŵ1/0.5, 0, 1).
pub fn composite(fp: &FingerprintReport, style: &StyleParams) -> Score {
    let bands = bands_for(style);
    let w1n = w1_normalized(&fp.sentence_lengths, style);
    let len = (1.0 - w1n / 0.5).clamp(0.0, 1.0);
    let burst = band_score(fp.autocorr_lag1, &bands[2].1);
    let hapax = band_score(fp.hapax_ratio, &bands[1].1);
    let zipf = band_score(fp.zipf_exponent, &bands[3].1);
    let s = 0.4 * len + 0.2 * burst + 0.25 * hapax + 0.15 * zipf;
    Score {
        s,
        len,
        burst,
        hapax,
        zipf,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantile_targets_spec_worked_example() {
        let style = StyleParams {
            cv: 0.75,
            median_length: 20.0,
            ..Default::default()
        };
        let targets = quantile_targets(50, &style);
        assert_eq!(targets.len(), 50);

        let positions = [0, 4, 12, 24, 37, 44, 48, 49];
        let expected = [5.0, 8.0, 13.0, 20.0, 31.0, 45.0, 70.0, 95.0];
        for (&pos, &exp) in positions.iter().zip(expected.iter()) {
            assert!(
                (targets[pos] - exp).abs() <= 1.0,
                "pos {pos}: got {}, want ~{exp}",
                targets[pos]
            );
        }

        for w in targets.windows(2) {
            assert!(w[0] <= w[1]);
        }
        assert!(*targets.first().unwrap() >= L_MIN);
        assert!(*targets.last().unwrap() <= L_MAX);
    }

    #[test]
    fn band_score_inside_and_outside() {
        let b = Bands {
            target: (0.8, 1.2),
            hard: (0.5, 1.5),
        };
        assert!((band_score(1.0, &b) - 1.0).abs() < 1e-9);
        assert!((band_score(0.5, &b) - 0.0).abs() < 1e-9);
        assert!((band_score(0.65, &b) - 0.5).abs() < 1e-9);
    }
}
