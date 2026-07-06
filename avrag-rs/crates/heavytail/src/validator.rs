//! Tolerance-band validation against style targets (spec §11).

use crate::metrics::FingerprintReport;
use crate::score::bands_for;
use crate::StyleParams;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetricCheck {
    pub metric: String,
    pub actual: f64,
    pub target: (f64, f64),
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValidationReport {
    pub fingerprint: FingerprintReport,
    pub passed: bool,
    pub metric_results: Vec<MetricCheck>,
}

/// Validate fingerprint metrics against spec §11 target bands.
///
/// `passed` is true only when every metric lies inside its target band (not merely
/// inside the hard-fail bounds used by band scoring).
pub fn validate(fp: &FingerprintReport, style: &StyleParams) -> ValidationReport {
    let bands = bands_for(style);
    let checks = [
        ("cv", fp.cv, bands[0].1.target),
        ("hapax_ratio", fp.hapax_ratio, bands[1].1.target),
        ("burstiness", fp.autocorr_lag1, bands[2].1.target),
        ("zipf_exponent", fp.zipf_exponent, bands[3].1.target),
    ];

    let metric_results: Vec<MetricCheck> = checks
        .into_iter()
        .map(|(metric, actual, target)| {
            let passed = actual >= target.0 && actual <= target.1;
            MetricCheck {
                metric: metric.to_string(),
                actual,
                target,
                passed,
            }
        })
        .collect();

    let passed = metric_results.iter().all(|m| m.passed);

    ValidationReport {
        fingerprint: fp.clone(),
        passed,
        metric_results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn fp_with(cv: f64, hapax: f64, burst: f64, zipf: f64) -> FingerprintReport {
        FingerprintReport {
            sentence_lengths: vec![12, 18, 9],
            mean_length: 13.0,
            cv,
            autocorr_lag1: burst,
            lognormal_ks_stat: 0.1,
            total_tokens: 20,
            vocab_size: 15,
            ttr: 0.75,
            hapax_ratio: hapax,
            zipf_exponent: zipf,
            word_freq: BTreeMap::new(),
        }
    }

    #[test]
    fn validate_passes_when_all_metrics_in_target_bands() {
        let style = StyleParams::default();
        let fp = fp_with(0.75, 0.45, 0.35, 1.0);
        let report = validate(&fp, &style);
        assert!(report.passed);
        assert!(report.metric_results.iter().all(|m| m.passed));
    }

    #[test]
    fn validate_cv_band_edges() {
        let style = StyleParams::default();
        let target = bands_for(&style)[0].1.target;

        let low_edge = fp_with(target.0, 0.45, 0.35, 1.0);
        assert!(validate(&low_edge, &style).passed);

        let high_edge = fp_with(target.1, 0.45, 0.35, 1.0);
        assert!(validate(&high_edge, &style).passed);

        let below = fp_with(target.0 - 0.01, 0.45, 0.35, 1.0);
        let report = validate(&below, &style);
        assert!(!report.passed);
        assert!(!report.metric_results[0].passed);

        let above = fp_with(target.1 + 0.01, 0.45, 0.35, 1.0);
        assert!(!validate(&above, &style).passed);
    }

    #[test]
    fn validate_hapax_band() {
        let style = StyleParams::default();
        assert!(validate(&fp_with(0.75, 0.35, 0.35, 1.0), &style).passed);
        assert!(validate(&fp_with(0.75, 0.55, 0.35, 1.0), &style).passed);
        assert!(!validate(&fp_with(0.75, 0.34, 0.35, 1.0), &style).passed);
        assert!(!validate(&fp_with(0.75, 0.56, 0.35, 1.0), &style).passed);
    }

    #[test]
    fn validate_burstiness_band() {
        let style = StyleParams::default();
        assert!(validate(&fp_with(0.75, 0.45, 0.1, 1.0), &style).passed);
        assert!(validate(&fp_with(0.75, 0.45, 0.6, 1.0), &style).passed);
        assert!(!validate(&fp_with(0.75, 0.45, 0.09, 1.0), &style).passed);
        assert!(!validate(&fp_with(0.75, 0.45, 0.61, 1.0), &style).passed);
    }

    #[test]
    fn validate_zipf_band() {
        let style = StyleParams::default();
        assert!(validate(&fp_with(0.75, 0.45, 0.35, 0.8), &style).passed);
        assert!(validate(&fp_with(0.75, 0.45, 0.35, 1.3), &style).passed);
        assert!(!validate(&fp_with(0.75, 0.45, 0.35, 0.79), &style).passed);
        assert!(!validate(&fp_with(0.75, 0.45, 0.35, 1.31), &style).passed);
    }

    #[test]
    fn validate_reports_each_metric() {
        let style = StyleParams::default();
        let report = validate(&fp_with(0.75, 0.45, 0.35, 1.0), &style);
        assert_eq!(report.metric_results.len(), 4);
        let names: Vec<_> = report
            .metric_results
            .iter()
            .map(|m| m.metric.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["cv", "hapax_ratio", "burstiness", "zipf_exponent"]
        );
    }
}
