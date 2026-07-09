//! Cost report aggregation over llm_real artifacts.
use super::*;



// ---------------------------------------------------------------------------
// Cost report
// ---------------------------------------------------------------------------

/// Scan all `metadata.json` files under `tests/e2e_output/llm_real/` and
/// print a cost summary.  Fails (with a warning) if the estimated monthly
/// spend exceeds the threshold.
#[tokio::test]
#[ignore = "utility — run manually to inspect costs"]
async fn cost_report_from_artifacts() {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("e2e_output")
        .join("llm_real");

    if !base.exists() {
        eprintln!(
            "No artifact directory found at {}; no real-LLM tests have been run.",
            base.display()
        );
        return;
    }

    fn collect_metadata_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new("metadata.json"))
                {
                    out.push(path);
                } else if path.is_dir() {
                    collect_metadata_files(&path, out);
                }
            }
        }
    }
    let mut files = Vec::new();
    collect_metadata_files(&base, &mut files);

    let mut test_count = 0usize;
    // Token counts are available in artifact metadata via ChatResponse.usage.
    let mut total_prompt_tokens = 0u64;
    let mut total_completion_tokens = 0u64;

    for path in &files {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let meta: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        if let Some(usage) = meta.get("usage").and_then(|u| u.as_object()) {
            total_prompt_tokens += usage
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            total_completion_tokens += usage
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
        }
        test_count += 1;
    }

    // Approximate cost when usage is missing from older artifacts.
    //   LLM: ~3K tokens × ¥0.001/1K = ¥0.003
    //   Embedding: ~1.5K tokens × ¥0.0005/1K = ¥0.00075
    //   ≈ ¥0.004 per test
    let approx_cost_per_test = 0.004_f64;
    let total_cost_cny = test_count as f64 * approx_cost_per_test;

    println!("\n=== Real-LLM E2E Cost Report ===");
    println!("  Artifact files:     {}", files.len());
    println!("  Tests run:          {}", test_count);
    println!("  Total prompt tok:   {total_prompt_tokens}");
    println!("  Total completion:   {total_completion_tokens}");
    println!("  Est. cost/test:     ¥{:.4}", approx_cost_per_test);
    println!(
        "  Est. total cost:    ¥{:.4} ({:.4} USD @ 7.2)",
        total_cost_cny,
        total_cost_cny / 7.2
    );

    // Monthly budget threshold: ¥10 CNY (~$1.40 USD)
    const MONTHLY_BUDGET_CNY: f64 = 10.0;
    if total_cost_cny > MONTHLY_BUDGET_CNY {
        eprintln!(
            "\n⚠️ WARNING: estimated cost ¥{:.2} exceeds monthly budget ¥{:.2}!",
            total_cost_cny, MONTHLY_BUDGET_CNY
        );
    }
}
