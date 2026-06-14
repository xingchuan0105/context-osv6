//! e2e-analyzer — CLI tool for analyzing E2E test artifacts.

mod attribution;
mod baseline;
mod cli;
mod coverage;
mod diff;
mod fingerprint;
mod loader;
mod llm_real_trends;
mod models;
mod report;
mod stability;

use clap::Parser;
use cli::{Cli, Commands, ReportFormat};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Diff {
            baseline,
            current,
            output,
            min_severity,
        } => {
            let baseline_dir = if baseline.is_dir() {
                baseline
            } else {
                let parent = baseline.parent().unwrap_or(Path::new("."));
                let run_id = baseline.file_name().unwrap_or_default().to_string_lossy();
                loader::find_run_dir(parent, &run_id)
                    .ok_or_else(|| anyhow::anyhow!("Baseline run not found: {}", run_id))?
            };

            let current_dir = if current.is_dir() {
                current
            } else {
                let parent = current.parent().unwrap_or(Path::new("."));
                let run_id = current.file_name().unwrap_or_default().to_string_lossy();
                loader::find_run_dir(parent, &run_id)
                    .ok_or_else(|| anyhow::anyhow!("Current run not found: {}", run_id))?
            };

            let baseline_results = loader::load_run_results(&baseline_dir);
            let current_results = loader::load_run_results(&current_dir);

            let baseline_map: HashMap<String, &models::TestResult> = baseline_results
                .iter()
                .map(|r| (r.test_name.clone(), r))
                .collect();
            let current_map: HashMap<String, &models::TestResult> = current_results
                .iter()
                .map(|r| (r.test_name.clone(), r))
                .collect();

            let mut all_diffs: Vec<(String, Vec<models::DiffEntry>)> = Vec::new();
            let min_sev = parse_min_severity(&min_severity);

            for test_name in current_map.keys() {
                if let (Some(b), Some(c)) =
                    (baseline_map.get(test_name), current_map.get(test_name))
                {
                    let diffs = diff::compare_results(test_name, b, c);
                    let filtered: Vec<_> = diffs
                        .into_iter()
                        .filter(|d| severity_ge(d.severity, min_sev))
                        .collect();
                    if !filtered.is_empty() {
                        all_diffs.push((test_name.clone(), filtered));
                    }
                }
            }

            // Print summary
            let summary = report::summarize_diffs(&all_diffs);
            println!(
                "Diff summary: {} critical, {} major, {} minor, {} info",
                summary.critical, summary.major, summary.minor, summary.info
            );
            println!("Tests with diffs: {}", all_diffs.len());

            for (test_name, entries) in &all_diffs {
                println!("\n  [{}]", test_name);
                for entry in entries {
                    println!(
                        "    - {:?} ({:?}): {}",
                        entry.dimension, entry.severity, entry.description
                    );
                }
            }

            let exit_code = report::exit_code(&summary);
            if let Some(out_path) = output {
                let md = report::generate_markdown_report(
                    &baseline_dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    &current_dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    &current_results,
                    &all_diffs,
                );
                fs::write(&out_path, md)?;
                println!("\nReport written to {}", out_path.display());
            }

            std::process::exit(exit_code);
        }

        Commands::Diagnose { run, output, test } => {
            let run_dir = if run.is_dir() {
                run
            } else {
                let parent = run.parent().unwrap_or(Path::new("."));
                let run_id = run.file_name().unwrap_or_default().to_string_lossy();
                loader::find_run_dir(parent, &run_id)
                    .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?
            };

            let results = loader::load_run_results(&run_dir);
            let baseline_dir = baseline::resolve_baseline(
                run_dir.parent().unwrap_or(Path::new(".")),
                None,
                &run_dir,
            );

            let baseline_results = baseline_dir
                .as_ref()
                .map(|d| loader::load_run_results(d))
                .unwrap_or_default();
            let baseline_map: HashMap<String, &models::TestResult> = baseline_results
                .iter()
                .map(|r| (r.test_name.clone(), r))
                .collect();
            let current_map: HashMap<String, &models::TestResult> =
                results.iter().map(|r| (r.test_name.clone(), r)).collect();

            let mut reports = Vec::new();

            for test_name in current_map.keys() {
                // If --test filter is provided, skip non-matching tests
                if let Some(ref filter) = test {
                    if test_name != filter {
                        continue;
                    }
                }

                let current = current_map[test_name];
                let baseline = baseline_map.get(test_name).copied().unwrap_or(current);

                let diffs = if let Some(b) = baseline_map.get(test_name) {
                    diff::compare_results(test_name, b, current)
                } else {
                    Vec::new()
                };

                if let Some(report) =
                    attribution::attribute_failures(test_name, &diffs, baseline, current)
                {
                    reports.push(report);
                }
            }

            // Print attribution reports
            if reports.is_empty() {
                println!("No failures to diagnose.");
            } else {
                println!("Diagnosis for {} test(s):\n", reports.len());
                for r in &reports {
                    println!("  [{}]", r.test_name);
                    println!("    Category: {:?}", r.failure_category);
                    println!("    Confidence: {:?}", r.confidence);
                    for layer in &r.suspected_layers {
                        println!("    Suspected layer: {}", layer.layer);
                        for ev in &layer.evidence {
                            println!("      Evidence: {}", ev);
                        }
                    }
                    if let Some(ref anomaly) = r.first_anomaly {
                        println!("    First anomaly: {}", anomaly.description);
                        if let Some(idx) = anomaly.tool_call_index {
                            println!("      Tool call index: {}", idx);
                        }
                    }
                    if !r.notes.is_empty() {
                        for note in &r.notes {
                            println!("    Note: {}", note);
                        }
                    }
                    println!();
                }
            }

            if let Some(out_path) = output {
                let json = serde_json::to_string_pretty(&reports)?;
                fs::write(&out_path, json)?;
                println!("Report written to {}", out_path.display());
            }
        }

        Commands::Report {
            run,
            output,
            format,
        } => {
            let run_dir = if run.is_dir() {
                run
            } else {
                let parent = run.parent().unwrap_or(Path::new("."));
                let run_id = run.file_name().unwrap_or_default().to_string_lossy();
                loader::find_run_dir(parent, &run_id)
                    .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?
            };

            let run_record = loader::load_run_record(&run_dir);
            let results = run_record
                .map(|record| record.results)
                .unwrap_or_else(|| loader::load_run_results(&run_dir));
            let baseline_dir = baseline::resolve_baseline(
                run_dir.parent().unwrap_or(Path::new(".")),
                None,
                &run_dir,
            );

            let baseline_results = baseline_dir
                .as_ref()
                .map(|d| loader::load_run_results(d))
                .unwrap_or_default();
            let baseline_map: HashMap<String, &models::TestResult> = baseline_results
                .iter()
                .map(|r| (r.test_name.clone(), r))
                .collect();
            let current_map: HashMap<String, &models::TestResult> =
                results.iter().map(|r| (r.test_name.clone(), r)).collect();

            let mut all_diffs: Vec<(String, Vec<models::DiffEntry>)> = Vec::new();

            for test_name in current_map.keys() {
                if let (Some(b), Some(c)) =
                    (baseline_map.get(test_name), current_map.get(test_name))
                {
                    let diffs = diff::compare_results(test_name, b, c);
                    if !diffs.is_empty() {
                        all_diffs.push((test_name.clone(), diffs));
                    }
                }
            }

            let baseline_run_id = baseline_dir
                .as_ref()
                .map(|d| {
                    d.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or_else(|| "none".to_string());
            let current_run_id = run_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let content = match format {
                ReportFormat::Markdown => report::generate_markdown_report(
                    &baseline_run_id,
                    &current_run_id,
                    &results,
                    &all_diffs,
                ),
                ReportFormat::Json => {
                    let summary = report::build_json_summary(
                        &baseline_run_id,
                        &current_run_id,
                        &results,
                        &all_diffs,
                    );
                    serde_json::to_string_pretty(&summary)?
                }
            };

            fs::write(&output, content)?;
            println!(
                "Report written to {} ({:?} format)",
                output.display(),
                format
            );
        }

        Commands::Baseline { run, name, store } => {
            let run_dir = if run.is_dir() {
                run
            } else {
                let parent = run.parent().unwrap_or(Path::new("."));
                let run_id = run.file_name().unwrap_or_default().to_string_lossy();
                loader::find_run_dir(parent, &run_id)
                    .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?
            };

            let run_id = run_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let output_dir: std::path::PathBuf = store
                .clone()
                .unwrap_or_else(|| run_dir.parent().unwrap_or(Path::new(".")).to_path_buf());

            // "promote" subcommand semantics: write the run_id as persistent baseline
            baseline::write_persistent_baseline(&output_dir, &run_id)?;
            println!("Baseline '{}' promoted to: {}", name, run_id);

            // Also show the current baseline
            if let Some(current) = baseline::read_persistent_baseline(&output_dir) {
                println!("Current persistent baseline: {}", current);
            }
        }

        Commands::Coverage { runs, output } => {
            let output_dir = if runs.is_empty() {
                std::path::PathBuf::from("crates/app/tests/e2e_output")
            } else {
                runs[0].parent().unwrap_or(Path::new(".")).to_path_buf()
            };
            let all_runs = loader::discover_all_runs(&output_dir);
            let recent: Vec<_> = all_runs
                .iter()
                .rev()
                .take(runs.len().max(30))
                .cloned()
                .collect();
            let results: Vec<_> = recent.iter().map(|d| loader::load_run_results(d)).collect();
            let matrix = coverage::build_coverage_matrix(&results);
            let gaps = matrix.gaps();
            let report = coverage::generate_coverage_report(&gaps);
            println!("{}", report);
            if let Some(out) = output {
                fs::write(&out, &report)?;
                println!("Report written to {}", out.display());
            }
        }

        Commands::Trends {
            history,
            output,
            limit,
        } => {
            let all_runs = loader::discover_all_runs(&history);
            let recent: Vec<_> = all_runs.iter().rev().take(limit).cloned().collect();
            let run_results: Vec<_> = recent
                .iter()
                .map(|d| {
                    let run_id = d.file_name().unwrap().to_string_lossy().to_string();
                    (run_id, loader::load_run_results(d))
                })
                .collect();

            // Find all unique test names across runs
            let mut test_names = std::collections::HashSet::new();
            for (_, results) in &run_results {
                for r in results {
                    test_names.insert(r.test_name.clone());
                }
            }

            let mut all_reports = Vec::new();
            for test_name in test_names {
                if let Some(record) = stability::analyze_stability(&test_name, &run_results) {
                    all_reports.push(stability::generate_stability_report(&record));
                }
            }

            if all_reports.is_empty() {
                let llm_root = history
                    .file_name()
                    .and_then(|n| n.to_str())
                    .filter(|name| *name == "llm_real")
                    .map(|_| history.parent().unwrap_or(&history).to_path_buf())
                    .unwrap_or_else(|| history.clone());
                let llm_series =
                    llm_real_trends::collect_llm_real_series(&llm_root, limit);
                if llm_series.is_empty() {
                    println!("Not enough data for stability analysis.");
                } else {
                    let report = llm_real_trends::generate_llm_real_trends_report(&llm_series);
                    println!("{report}");
                    if let Some(out) = output {
                        fs::write(&out, &report)?;
                        println!("Report written to {}", out.display());
                    }
                }
            } else {
                for report in &all_reports {
                    println!("{}", report);
                    println!("---\n");
                }
                if let Some(out) = output {
                    let combined = all_reports.join("\n---\n");
                    fs::write(&out, combined)?;
                    println!("Report written to {}", out.display());
                }
            }
        }

        Commands::LlmReal { command } => match command {
            cli::LlmRealCommands::List { output, limit } => {
                let runs = loader::discover_bucket_runs(&output, "llm_real");
                if runs.is_empty() {
                    println!("No llm_real runs under {}", output.join("llm_real").display());
                } else {
                    let start = runs.len().saturating_sub(limit);
                    for run_dir in &runs[start..] {
                        let artifacts = loader::load_llm_real_run(run_dir);
                        println!(
                            "{}  ({} tests)",
                            run_dir.display(),
                            artifacts.len()
                        );
                    }
                }
            }

            cli::LlmRealCommands::Summary { run, output } => {
                let run_dir = if run.is_dir() {
                    run
                } else {
                    let parent = run.parent().unwrap_or(Path::new("."));
                    let run_id = run.file_name().unwrap_or_default().to_string_lossy();
                    loader::find_run_dir(parent, &run_id)
                        .ok_or_else(|| anyhow::anyhow!("llm_real run not found: {}", run_id))?
                };

                let artifacts = loader::load_llm_real_run(&run_dir);
                if artifacts.is_empty() {
                    println!(
                        "No llm_real metadata.json files found under {}",
                        run_dir.display()
                    );
                } else {
                    let mut lines = vec![
                        "# llm_real observability summary".to_string(),
                        format!("Run: {}", run_dir.display()),
                        format!("Tests: {}", artifacts.len()),
                        String::new(),
                        "| test | agent | reasoning_deltas | trace_reasoning | prompt_snapshots | empty_warning | error+done |"
                            .to_string(),
                        "|------|-------|------------------|-----------------|------------------|---------------|------------|"
                            .to_string(),
                    ];
                    for artifact in &artifacts {
                        lines.push(format!(
                            "| {} | {} | {} | {} | {} | {} | {} |",
                            artifact.test_name,
                            artifact.agent_type.as_deref().unwrap_or("-"),
                            artifact
                                .reasoning_delta_count
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            artifact
                                .trace_reasoning_count
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            artifact
                                .prompt_snapshot_count
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            artifact
                                .reasoning_empty_warning
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            artifact
                                .stream_error_with_done
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                        ));
                    }
                    let report = lines.join("\n");
                    if let Some(out) = output {
                        fs::write(&out, &report)?;
                        println!("Report written to {}", out.display());
                    } else {
                        println!("{report}");
                    }
                }
            },

            cli::LlmRealCommands::Trends { output, limit, out } => {
                let series = llm_real_trends::collect_llm_real_series(&output, limit);
                let report = llm_real_trends::generate_llm_real_trends_report(&series);
                if let Some(path) = out {
                    fs::write(&path, &report)?;
                    println!("Report written to {}", path.display());
                } else {
                    println!("{report}");
                }
            }
        },
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_min_severity(s: &str) -> models::DiffSeverity {
    match s.to_lowercase().as_str() {
        "critical" => models::DiffSeverity::Critical,
        "major" => models::DiffSeverity::Major,
        "minor" => models::DiffSeverity::Minor,
        _ => models::DiffSeverity::Minor,
    }
}

fn severity_ge(a: models::DiffSeverity, b: models::DiffSeverity) -> bool {
    use models::DiffSeverity::*;
    let rank = |s| match s {
        Critical => 3,
        Major => 2,
        Minor => 1,
        Info => 0,
    };
    rank(a) >= rank(b)
}
