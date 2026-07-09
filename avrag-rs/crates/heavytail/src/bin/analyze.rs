//! CLI: fingerprint analysis and human-vs-AI corpus comparison (M1 gate).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use heavytail::metrics::{analyze_sentences, FingerprintReport};
use heavytail::score::composite;
use heavytail::segment::split_sentences;
use heavytail::sensitivity::{length_sensitivity, SensitivityRow};
use heavytail::StyleParams;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        process::exit(1);
    }

    let result = if args[0] == "compare" {
        if args.len() != 3 {
            eprintln!("Usage: heavytail-analyze compare <dir_human> <dir_ai>");
            process::exit(1);
        }
        compare_corpora(Path::new(&args[1]), Path::new(&args[2]))
    } else {
        let json = args.iter().any(|a| a == "--json");
        let file = args
            .iter()
            .find(|a| *a != "--json")
            .cloned()
            .unwrap_or_else(|| {
                eprintln!("Usage: heavytail-analyze <file.txt> [--json]");
                process::exit(1);
            });
        analyze_file(Path::new(&file), json)
    };

    if let Err(err) = result {
        eprintln!("error: {err:#}");
        process::exit(1);
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  heavytail-analyze <file.txt> [--json]");
    eprintln!("  heavytail-analyze compare <dir_human> <dir_ai>");
}

fn analyze_prose(prose: &str) -> FingerprintReport {
    let sentences: Vec<(String, usize)> = split_sentences(prose, false)
        .into_iter()
        .map(|s| (s.text, s.para_idx))
        .collect();
    analyze_sentences(&sentences)
}

fn analyze_file(path: &Path, json: bool) -> anyhow::Result<()> {
    let prose = fs::read_to_string(path)?;
    let fp = analyze_prose(&prose);
    let style = StyleParams::default();
    let score = composite(&fp, &style);
    let mut sensitivity = length_sensitivity(&fp, &style);
    sensitivity.sort_by(|a, b| {
        b.delta_s
            .abs()
            .partial_cmp(&a.delta_s.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top: Vec<_> = sensitivity.into_iter().take(20).collect();

    if json {
        let out = serde_json::json!({
            "file": path.display().to_string(),
            "fingerprint": fp,
            "score": score,
            "top_sensitivity": top,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_fingerprint(path, &fp, &score);
        print_sensitivity(&top);
    }
    Ok(())
}

fn print_fingerprint(path: &Path, fp: &FingerprintReport, score: &heavytail::score::Score) {
    println!("File: {}", path.display());
    println!("Sentences: {}", fp.sentence_lengths.len());
    println!("Mean length: {:.2}", fp.mean_length);
    println!("CV: {:.4}", fp.cv);
    println!("Autocorr lag-1: {:.4}", fp.autocorr_lag1);
    println!("Hapax ratio: {:.4}", fp.hapax_ratio);
    println!("Zipf exponent: {:.4}", fp.zipf_exponent);
    println!("Lognormal KS: {:.4}", fp.lognormal_ks_stat);
    println!("TTR: {:.4}", fp.ttr);
    println!(
        "Composite S: {:.4} (len={:.3} burst={:.3} hapax={:.3} zipf={:.3})",
        score.s, score.len, score.burst, score.hapax, score.zipf
    );
}

fn print_sensitivity(rows: &[SensitivityRow]) {
    println!();
    println!("Top sensitivity rows (by |ΔS|):");
    println!(
        "{:>4} {:>6} {:>6} {:>8} {:>8}",
        "idx", "cur", "cand", "delta_s", "|delta|"
    );
    for row in rows {
        println!(
            "{:>4} {:>6} {:>6} {:>8.4} {:>8.4}",
            row.sentence_idx,
            row.current_len,
            row.candidate_len,
            row.delta_s,
            row.delta_s.abs()
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct MetricSpec {
    label: &'static str,
    extract: fn(&FingerprintReport) -> f64,
}

const COMPARE_METRICS: &[MetricSpec] = &[
    MetricSpec {
        label: "CV",
        extract: |fp| fp.cv,
    },
    MetricSpec {
        label: "Hapax",
        extract: |fp| fp.hapax_ratio,
    },
    MetricSpec {
        label: "Burstiness",
        extract: |fp| fp.autocorr_lag1,
    },
    MetricSpec {
        label: "Zipf",
        extract: |fp| fp.zipf_exponent,
    },
    MetricSpec {
        label: "KS",
        extract: |fp| fp.lognormal_ks_stat,
    },
];

#[derive(Debug)]
struct CompareRow {
    label: String,
    cohens_d: f64,
    verdict: &'static str,
}

fn compare_corpora(human_dir: &Path, ai_dir: &Path) -> anyhow::Result<()> {
    let human_fps = load_corpus(human_dir)?;
    let ai_fps = load_corpus(ai_dir)?;

    if human_fps.is_empty() || ai_fps.is_empty() {
        anyhow::bail!(
            "both corpora need at least one *.txt file (human={}, ai={})",
            human_fps.len(),
            ai_fps.len()
        );
    }

    println!(
        "Corpus compare: human={} files, ai={} files",
        human_fps.len(),
        ai_fps.len()
    );
    println!();
    println!(
        "{:<12} {:>10} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Metric", "Human μ", "Human σ", "AI μ", "AI σ", "Cohen d", "Verdict"
    );

    let mut rows = Vec::new();
    for spec in COMPARE_METRICS {
        let human_vals: Vec<f64> = human_fps.iter().map(spec.extract).collect();
        let ai_vals: Vec<f64> = ai_fps.iter().map(spec.extract).collect();
        let human_mean = mean(&human_vals);
        let ai_mean = mean(&ai_vals);
        let human_std = sample_std(&human_vals);
        let ai_std = sample_std(&ai_vals);
        let d = cohens_d(&human_vals, &ai_vals);
        let verdict = verdict_label(d);
        rows.push(CompareRow {
            label: spec.label.to_string(),
            cohens_d: d,
            verdict,
        });
        println!(
            "{:<12} {:>10.4} {:>10.4} {:>10.4} {:>10.4} {:>10.4} {:>12}",
            spec.label,
            human_mean,
            human_std,
            ai_mean,
            ai_std,
            d,
            verdict
        );
    }

    println!();
    print_gate_verdict(&rows);
    Ok(())
}

fn load_corpus(dir: &Path) -> anyhow::Result<Vec<FingerprintReport>> {
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let prose = fs::read_to_string(&p)?;
            Ok(analyze_prose(&prose))
        })
        .collect()
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn sample_std(xs: &[f64]) -> f64 {
    let n = xs.len();
    if n < 2 {
        return 0.0;
    }
    let m = mean(xs);
    let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64;
    var.sqrt()
}

fn cohens_d(human: &[f64], ai: &[f64]) -> f64 {
    let n1 = human.len();
    let n2 = ai.len();
    if n1 < 2 || n2 < 2 {
        return 0.0;
    }
    let v1 = sample_std(human).powi(2);
    let v2 = sample_std(ai).powi(2);
    let pooled = ((n1 - 1) as f64 * v1 + (n2 - 1) as f64 * v2) / (n1 + n2 - 2) as f64;
    if pooled <= f64::EPSILON {
        return 0.0;
    }
    (mean(human) - mean(ai)) / pooled.sqrt()
}

fn verdict_label(d: f64) -> &'static str {
    let abs = d.abs();
    if abs >= 0.8 {
        "SEPARATES"
    } else if abs >= 0.3 {
        "WEAK"
    } else {
        "NO"
    }
}

fn print_gate_verdict(rows: &[CompareRow]) {
    let gate_metrics = ["CV", "Hapax", "Burstiness"];
    let mut gate_pass = true;
    println!("GATE 1 (M1): CV, hapax, burstiness must SEPARATE (|d| ≥ 0.8)");
    for label in gate_metrics {
        if let Some(row) = rows.iter().find(|r| r.label == label) {
            let pass = row.cohens_d.abs() >= 0.8;
            if !pass {
                gate_pass = false;
            }
            println!(
                "  {label}: d={:.4} → {} ({})",
                row.cohens_d,
                row.verdict,
                if pass { "PASS" } else { "FAIL" }
            );
        }
    }
    println!();
    if gate_pass {
        println!("OVERALL: GATE 1 PASS — metrics separate human vs AI corpora.");
    } else {
        println!("OVERALL: GATE 1 FAIL — premise may not hold; escalate to user.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cohens_d_known_values() {
        let a: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let b: Vec<f64> = (0..10).map(|i| i as f64 + 5.0).collect();
        let d = cohens_d(&a, &b);
        assert!(d.abs() > 1.0, "expected large separation, got {d}");
    }

    #[test]
    fn verdict_thresholds() {
        assert_eq!(verdict_label(1.0), "SEPARATES");
        assert_eq!(verdict_label(-0.9), "SEPARATES");
        assert_eq!(verdict_label(0.5), "WEAK");
        assert_eq!(verdict_label(0.1), "NO");
    }
}
