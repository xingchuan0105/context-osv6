//! Eval v2 judge calibration tooling (ADR-0012, design §7.2 Phase 1).
//!
//! Usage:
//!   cargo run -p rag_quality --bin eval_v2_calibration -- export \
//!     --run-dir crates/app/tests/e2e_output/rag_eval_v2/<run_id> [--out labels.tsv]
//!   cargo run -p rag_quality --bin eval_v2_calibration -- kappa --labeled labels.tsv
//!
//! `export` emits a TSV (one row per question) with an empty `human_label`
//! column for manual binary labeling (0 = wrong, 1 = acceptable). `kappa`
//! reads the filled TSV and computes Cohen's κ between the judge and the
//! human labels.
//!
//! Judge→binary mapping (primary): label PASS or PARTIAL → 1, anything else
//! (INCORRECT / UNGROUNDED / REFUSAL_WRONG / SELECTION_MISS / RETRIEVAL_MISS /
//! JUDGE_ERROR / INFRA_ERROR) → 0. The alternative continuous mapping
//! (answer_correctness ≥ 0.7 → 1) is printed alongside for comparison.

use std::path::{Path, PathBuf};

use rag_quality::cohen_kappa_binary;
use rag_quality::eval_v2::ScoreV2;

const TSV_HEADER: &str = "qid\tsubset\tquery\tjudge_label\tcorrectness\tfaithfulness\treference_answer\tmodel_answer\thuman_label";

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("export") => cmd_export(&args[1..]),
        Some("kappa") => cmd_kappa(&args[1..]),
        _ => anyhow::bail!(
            "usage: eval_v2_calibration export --run-dir <dir> [--out labels.tsv] | \
             eval_v2_calibration kappa --labeled <labels.tsv>"
        ),
    }
}

/// Parse `--key value` pairs (both `--key value` forms only; no `=` joining,
/// matching the crate's other bins' minimal argv style).
fn parse_opts(args: &[String]) -> anyhow::Result<std::collections::HashMap<String, String>> {
    let mut opts = std::collections::HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].clone();
        if !key.starts_with("--") {
            anyhow::bail!("unexpected argument: {key}");
        }
        let value = args
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("missing value for {key}"))?
            .clone();
        opts.insert(key, value);
        i += 2;
    }
    Ok(opts)
}

// ---------------------------------------------------------------------------
// export
// ---------------------------------------------------------------------------

/// One TSV row (the unit of manual labeling).
#[derive(Debug, Clone, PartialEq)]
struct LabelRow {
    qid: String,
    subset: String,
    query: String,
    judge_label: String,
    correctness: Option<f64>,
    faithfulness: Option<f64>,
    reference_answer: String,
    model_answer: String,
    human_label: String,
}

/// Wrapper for the parts of `q{nnn}.artifact.json` that live outside
/// `score_v2` (older runs lack `score_v2.reference_answer/model_answer`).
#[derive(Debug, serde::Deserialize)]
struct ArtifactFile {
    #[serde(default)]
    answer: Option<String>,
    score_v2: ScoreV2,
}

fn collect_rows(run_dir: &Path) -> anyhow::Result<Vec<LabelRow>> {
    let mut artifacts: Vec<PathBuf> = std::fs::read_dir(run_dir)
        .map_err(|e| anyhow::anyhow!("read run dir {}: {e}", run_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('q') && n.ends_with(".artifact.json"))
        })
        .collect();
    artifacts.sort();
    if artifacts.is_empty() {
        anyhow::bail!("no q*.artifact.json under {}", run_dir.display());
    }

    let mut rows = Vec::new();
    for path in artifacts {
        let qid = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .trim_end_matches(".artifact.json")
            .to_string();
        let raw = std::fs::read_to_string(&path)?;
        let artifact: ArtifactFile = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?;
        let score = artifact.score_v2;
        rows.push(LabelRow {
            qid,
            subset: score.subset.clone(),
            query: score.query.clone(),
            judge_label: score.label.as_str().to_string(),
            correctness: score.judge.as_ref().map(|j| j.answer_correctness.score),
            faithfulness: score.judge.as_ref().map(|j| j.faithfulness.score),
            reference_answer: score.reference_answer.clone().unwrap_or_default(),
            // score_v2.model_answer (P3+) wins; the artifact-level `answer`
            // field covers runs written before that field existed.
            model_answer: score
                .model_answer
                .clone()
                .or(artifact.answer)
                .unwrap_or_default(),
            human_label: String::new(),
        });
    }
    Ok(rows)
}

fn tsv_escape(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

fn fmt_score(value: Option<f64>) -> String {
    value.map(|v| format!("{v:.4}")).unwrap_or_default()
}

fn render_tsv(rows: &[LabelRow]) -> String {
    let mut out = String::from(TSV_HEADER);
    out.push('\n');
    for row in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            tsv_escape(&row.qid),
            tsv_escape(&row.subset),
            tsv_escape(&row.query),
            row.judge_label,
            fmt_score(row.correctness),
            fmt_score(row.faithfulness),
            tsv_escape(&row.reference_answer),
            tsv_escape(&row.model_answer),
            row.human_label,
        ));
    }
    out
}

fn cmd_export(args: &[String]) -> anyhow::Result<()> {
    let opts = parse_opts(args)?;
    let run_dir = opts
        .get("--run-dir")
        .ok_or_else(|| anyhow::anyhow!("export requires --run-dir <dir>"))?;
    let rows = collect_rows(Path::new(run_dir))?;
    let tsv = render_tsv(&rows);
    match opts.get("--out") {
        Some(out) => {
            std::fs::write(out, &tsv)?;
            println!(
                "wrote {} rows to {out} — fill human_label (0=wrong, 1=acceptable), then: kappa --labeled {out}",
                rows.len()
            );
        }
        None => print!("{tsv}"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// kappa
// ---------------------------------------------------------------------------

/// A filled-in row: judge side + human side.
#[derive(Debug, Clone, PartialEq)]
struct LabeledRow {
    judge_label: String,
    correctness: Option<f64>,
    human: bool,
}

/// Parse a filled labels.tsv. Rows with a blank `human_label` are skipped
/// (count returned); rows with any other value than 0/1/blank are an error.
/// A missing last column (editor stripped the trailing tab) counts as blank.
fn parse_labeled_tsv(content: &str) -> anyhow::Result<(Vec<LabeledRow>, usize)> {
    let mut rows = Vec::new();
    let mut skipped = 0usize;
    for (lineno, line) in content.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line == TSV_HEADER {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 8 {
            anyhow::bail!("line {}: expected 9 tab-separated columns", lineno + 1);
        }
        let human_raw = cols.get(8).map(|s| s.trim()).unwrap_or("");
        if human_raw.is_empty() {
            skipped += 1;
            continue;
        }
        let human = match human_raw {
            "0" => false,
            "1" => true,
            other => anyhow::bail!("line {}: human_label must be 0/1, got {other:?}", lineno + 1),
        };
        rows.push(LabeledRow {
            judge_label: cols[3].to_string(),
            correctness: cols[4].parse::<f64>().ok(),
            human,
        });
    }
    Ok((rows, skipped))
}

/// Judge→binary mapping (primary): PASS or PARTIAL → 1 (acceptable), else 0.
fn judge_binary_label(label: &str) -> bool {
    label == "PASS" || label == "PARTIAL"
}

/// Alternative mapping: answer_correctness ≥ 0.7 → 1 (missing score → 0).
fn judge_binary_correctness(correctness: Option<f64>) -> bool {
    correctness.is_some_and(|c| c >= 0.7)
}

fn print_kappa_report(title: &str, judge: &[bool], human: &[bool]) {
    let agreement = judge.iter().zip(human).filter(|(j, h)| j == h).count();
    let tp = judge.iter().zip(human).filter(|(j, h)| **j && **h).count();
    let fp = judge.iter().zip(human).filter(|(j, h)| **j && !**h).count();
    let fn_ = judge.iter().zip(human).filter(|(j, h)| !**j && **h).count();
    let tn = judge.iter().zip(human).filter(|(j, h)| !**j && !**h).count();
    println!("{title}");
    println!(
        "  agreement: {agreement}/{} (judge+ human+: {tp}, judge+ human-: {fp}, judge- human+: {fn_}, judge- human-: {tn})",
        judge.len()
    );
    match cohen_kappa_binary(human, judge) {
        Some(kappa) => {
            println!("  kappa: {kappa:.3}");
            println!(
                "  gate: {}",
                if kappa >= 0.60 {
                    "eligible (kappa >= 0.60, design §7.2 Phase 1)"
                } else {
                    "not eligible (kappa < 0.60, design §7.2 Phase 1)"
                }
            );
        }
        None => println!("  kappa: undefined (label distribution has zero variance)"),
    }
}

fn cmd_kappa(args: &[String]) -> anyhow::Result<()> {
    let opts = parse_opts(args)?;
    let labeled = opts
        .get("--labeled")
        .ok_or_else(|| anyhow::anyhow!("kappa requires --labeled <labels.tsv>"))?;
    let content = std::fs::read_to_string(labeled)?;
    let (rows, skipped) = parse_labeled_tsv(&content)?;
    println!("Judge calibration (eval v2, ADR-0012): {labeled}");
    println!("  labeled rows: {} (skipped blank human_label: {skipped})", rows.len());
    if rows.is_empty() {
        println!("  kappa: pending (no filled human_label rows)");
        return Ok(());
    }
    let human: Vec<bool> = rows.iter().map(|r| r.human).collect();
    let judge_label: Vec<bool> = rows.iter().map(|r| judge_binary_label(&r.judge_label)).collect();
    let judge_corr: Vec<bool> = rows.iter().map(|r| judge_binary_correctness(r.correctness)).collect();
    print_kappa_report("primary mapping (PASS|PARTIAL → 1):", &judge_label, &human);
    print_kappa_report("alternative mapping (correctness ≥ 0.7 → 1):", &judge_corr, &human);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(qid: &str, label: &str, correctness: Option<f64>) -> LabelRow {
        LabelRow {
            qid: qid.to_string(),
            subset: "thesis_factual".to_string(),
            query: "Y公司哪一年在大连建厂？".to_string(),
            judge_label: label.to_string(),
            correctness,
            faithfulness: Some(1.0),
            reference_answer: "Y公司2019年在大连建厂。".to_string(),
            model_answer: "2019 年建厂".to_string(),
            human_label: String::new(),
        }
    }

    #[test]
    fn tsv_roundtrip_parse() {
        let tsv = render_tsv(&[row("q001", "PASS", Some(0.95)), row("q002", "INCORRECT", Some(0.2))]);
        assert!(tsv.starts_with(TSV_HEADER));
        // Fill the human_label column (last field) and parse back.
        let filled: String = tsv
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    line.to_string()
                } else {
                    format!("{line}{}", if i == 1 { "1" } else { "0" })
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let (rows, skipped) = parse_labeled_tsv(&filled).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].human);
        assert!(!rows[1].human);
        assert_eq!(rows[0].judge_label, "PASS");
        assert_eq!(rows[1].correctness, Some(0.2));
    }

    #[test]
    fn blank_human_label_rows_are_skipped() {
        let tsv = render_tsv(&[row("q001", "PASS", Some(0.9)), row("q002", "PASS", Some(0.8))]);
        // q001 filled, q002 left blank.
        let filled: String = tsv
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 1 {
                    format!("{line}1")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let (rows, skipped) = parse_labeled_tsv(&filled).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn kappa_perfect_and_zero_agreement() {
        // Perfect agreement on the label mapping → κ = 1.
        let rows = vec![
            LabeledRow { judge_label: "PASS".into(), correctness: Some(0.9), human: true },
            LabeledRow { judge_label: "PARTIAL".into(), correctness: Some(0.5), human: true },
            LabeledRow { judge_label: "INCORRECT".into(), correctness: Some(0.1), human: false },
            LabeledRow { judge_label: "UNGROUNDED".into(), correctness: Some(0.8), human: false },
        ];
        let human: Vec<bool> = rows.iter().map(|r| r.human).collect();
        let judge: Vec<bool> = rows.iter().map(|r| judge_binary_label(&r.judge_label)).collect();
        assert_eq!(cohen_kappa_binary(&human, &judge), Some(1.0));
        // The alternative correctness≥0.7 mapping disagrees on rows 2/4 here → κ < 1.
        let judge_alt: Vec<bool> = rows.iter().map(|r| judge_binary_correctness(r.correctness)).collect();
        let kappa_alt = cohen_kappa_binary(&human, &judge_alt).unwrap();
        assert!(kappa_alt < 1.0 && kappa_alt > -1.0);
    }

    #[test]
    fn parse_rejects_bad_human_label() {
        let tsv = format!("{TSV_HEADER}\nq001\ts\tq\tPASS\t0.9\t1.0\tref\tans\tx\n");
        let err = parse_labeled_tsv(&tsv).unwrap_err();
        assert!(err.to_string().contains("human_label"));
    }
}
