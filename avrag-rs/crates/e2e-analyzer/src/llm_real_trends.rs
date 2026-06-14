//! Trend analysis for sparse llm_real runs (one test per run directory).

use crate::loader;
use crate::models::LlmRealTestArtifact;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LlmRealTrendPoint {
    pub run_id: String,
    pub artifact: LlmRealTestArtifact,
    pub total_tokens: Option<u64>,
    pub citation_count: Option<u64>,
}

/// Collect llm_real artifacts grouped by `test_name` across recent runs.
pub fn collect_llm_real_series(
    output_dir: &Path,
    limit: usize,
) -> BTreeMap<String, Vec<LlmRealTrendPoint>> {
    let runs = loader::discover_bucket_runs(output_dir, "llm_real");
    let start = runs.len().saturating_sub(limit);
    let mut by_test: BTreeMap<String, Vec<LlmRealTrendPoint>> = BTreeMap::new();

    for run_dir in &runs[start..] {
        let run_id = run_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        for artifact in loader::load_llm_real_run(run_dir) {
            let total_tokens = artifact.usage.as_ref().and_then(|u| {
                u.get("total_tokens")
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
                        let prompt = u.get("prompt_tokens")?.as_u64()?;
                        let completion = u.get("completion_tokens")?.as_u64()?;
                        Some(prompt + completion)
                    })
            });
            let citation_count = artifact
                .extra
                .as_ref()
                .and_then(|e| e.get("citation_count"))
                .and_then(|v| v.as_u64())
                .or_else(|| {
                    artifact
                        .extra
                        .as_ref()
                        .and_then(|e| e.get("citation_count"))
                        .and_then(|v| v.as_i64())
                        .map(|v| v as u64)
                });
            // metadata.json top-level citation_count is parsed into extra by loader sometimes;
            // also read from raw fields via reasoning stats only — citation in extra from tests.
            let point = LlmRealTrendPoint {
                run_id: run_id.clone(),
                total_tokens,
                citation_count,
                artifact,
            };
            by_test
                .entry(point.artifact.test_name.clone())
                .or_default()
                .push(point);
        }
    }

    by_test
}

pub fn generate_llm_real_trends_report(
    series: &BTreeMap<String, Vec<LlmRealTrendPoint>>,
) -> String {
    if series.is_empty() {
        return "No llm_real runs found.".to_string();
    }

    let mut out = String::from("# llm_real Trends\n\n");
    out.push_str("| test | runs | empty_warn_rate | error+done_rate | avg_tokens | citation_ok_rate |\n");
    out.push_str("|------|------|-----------------|-----------------|------------|----------------|\n");

    for (test_name, points) in series {
        let n = points.len().max(1) as f64;
        let empty_warns = points
            .iter()
            .filter(|p| p.artifact.reasoning_empty_warning == Some(true))
            .count();
        let error_done = points
            .iter()
            .filter(|p| p.artifact.stream_error_with_done == Some(true))
            .count();
        let token_vals: Vec<u64> = points.iter().filter_map(|p| p.total_tokens).collect();
        let avg_tokens = if token_vals.is_empty() {
            "-".to_string()
        } else {
            format!(
                "{:.0}",
                token_vals.iter().sum::<u64>() as f64 / token_vals.len() as f64
            )
        };
        let citation_ok = points
            .iter()
            .filter(|p| p.citation_count.unwrap_or(0) > 0)
            .count();
        let citation_rate = if points.iter().any(|p| p.citation_count.is_some()) {
            format!("{:.0}%", (citation_ok as f64 / n) * 100.0)
        } else {
            "-".to_string()
        };

        out.push_str(&format!(
            "| {} | {} | {:.0}% | {:.0}% | {} | {} |\n",
            test_name,
            points.len(),
            (empty_warns as f64 / n) * 100.0,
            (error_done as f64 / n) * 100.0,
            avg_tokens,
            citation_rate,
        ));
    }

    out.push_str("\n## Run history (by test)\n\n");
    for (test_name, points) in series {
        out.push_str(&format!("### {}\n\n", test_name));
        out.push_str("| run_id | tokens | reasoning_deltas | trace | empty_warn | error+done |\n");
        out.push_str("|--------|--------|------------------|-------|------------|------------|\n");
        for p in points {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                p.run_id,
                p.total_tokens
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                p.artifact
                    .reasoning_delta_count
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                p.artifact
                    .trace_reasoning_count
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                p.artifact
                    .reasoning_empty_warning
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                p.artifact
                    .stream_error_with_done
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ));
        }
        out.push('\n');
    }

    out
}
