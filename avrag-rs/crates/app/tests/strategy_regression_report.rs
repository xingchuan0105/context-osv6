//! Regression report generator.
//!
//! Run after e2e_format_output and e2e_ingestion_answer to produce report.md.
//!
//! Run with: cargo test --ignored -p app --test strategy_regression_report

#[path = "strategy_e2e/playwright_helper.rs"]
mod playwright_helper;
#[path = "strategy_e2e/recording_llm.rs"]
mod recording_llm;
#[path = "strategy_e2e/result_serializer.rs"]
mod result_serializer;

#[tokio::test]
#[ignore = "requires prior E2E runs to aggregate"]
async fn generate_regression_report() {
    use result_serializer::*;

    let output_base = std::path::PathBuf::from("tests/e2e_output");

    // Find latest run directory
    let mut runs: Vec<_> = match std::fs::read_dir(&output_base) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("No E2E runs found in tests/e2e_output/");
            return;
        }
    }
    .filter_map(|e| e.ok())
    .filter(|e| e.file_name().to_string_lossy().starts_with("e2e_"))
    .collect();

    runs.sort_by_key(|e| {
        e.metadata()
            .unwrap()
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    let latest_run = match runs.last() {
        Some(r) => r.path(),
        None => {
            eprintln!("No E2E runs found in tests/e2e_output/");
            return;
        }
    };

    let results = load_run_results(&latest_run);
    let report = generate_markdown_report(&latest_run, &results).unwrap();

    let report_path = latest_run.join("report.md");
    std::fs::write(&report_path, report).unwrap();

    println!("Report written to: {}", report_path.display());
}
