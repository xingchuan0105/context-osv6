//! Real-LLM E2E regression suite.
//!
//! These tests use production LLM providers instead of the mock LLM server.
//! They are marked `#[ignore]` by default because they:
//! - incur API cost (DeepSeek + DashScope),
//! - are non-deterministic,
//! - are slower than mock tests,
//! - may hit provider rate limits under parallel execution.
//!
//! Run serially with:
//!   cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture
//!
//! Required environment (loaded from the repository `.env` if not already set):
//!   AGENT_LLM_BASE_URL, AGENT_LLM_API_KEY, AGENT_LLM_MODEL
//!   MEMORY_LLM_BASE_URL, MEMORY_LLM_API_KEY, MEMORY_LLM_MODEL
//!   INGESTION_LLM_BASE_URL, INGESTION_LLM_API_KEY, INGESTION_LLM_MODEL
//!   EMBEDDING_BASE_URL, EMBEDDING_API_KEY, EMBEDDING_MODEL
//!   SEARCH_PROVIDER, SEARCH_BASE_URL, SEARCH_API_KEY (search tests only)

use crate::product_e2e::TestContext;

/// Load key/value pairs from the repository `.env` file into the process
/// environment.  This lets real-LLM tests discover credentials without
/// requiring the caller to `source .env` first.
///
/// Only sets variables that are **not** already present in the environment,
/// so explicit exports take priority.
pub(crate) fn load_env_from_repo_dotenv() {
    // The worktree usually does not have its own `.env`.
    // Try the worktree location first, then fall back to the main repo copy.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/app -> crates -> avrag-rs -> e2e-analyzer -> worktrees -> .claude -> context-osv6 -> avrag-rs/.env
    let main_repo_dotenv = manifest
        .join("../../../../../../avrag-rs/.env")
        .canonicalize()
        .ok();
    let worktree_dotenv = manifest.join("../../.env").canonicalize().ok();
    let path = worktree_dotenv
        .or(main_repo_dotenv)
        .expect("repository .env file must exist for real-LLM tests");

    let content = std::fs::read_to_string(path).expect("read .env");
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();
        // Strip surrounding quotes if present.
        let value = if raw_value.len() >= 2 {
            let first = raw_value.chars().next().unwrap();
            let last = raw_value.chars().last().unwrap();
            if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
                &raw_value[1..raw_value.len() - 1]
            } else {
                raw_value
            }
        } else {
            raw_value
        };
        let should_set = match std::env::var(key) {
            Err(_) => true,
            // Allow .env to replace placeholder search credentials left in the shell.
            Ok(existing)
                if (key == "SEARCH_API_KEY" && (existing.is_empty() || existing == "mock"))
                    || (key == "SEARCH_BASE_URL" && existing.starts_with("http://127.0.0.1")) =>
            {
                true
            }
            Ok(_) => false,
        };
        if should_set {
            unsafe { std::env::set_var(key, value) };
        }
    }
}

/// True when SEARCH_API_KEY (or E2E_BRAVE_API_KEY) is present and not the mock placeholder.
pub fn has_real_search_credentials() -> bool {
    fn valid_key(key: Result<String, std::env::VarError>) -> bool {
        key.map(|k| !k.is_empty() && k != "mock").unwrap_or(false)
    }
    valid_key(std::env::var("SEARCH_API_KEY")) || valid_key(std::env::var("E2E_BRAVE_API_KEY"))
}

/// Ensure SEARCH_PROVIDER / SEARCH_BASE_URL defaults when using real Brave.
pub fn ensure_search_defaults() {
    if !std::env::var("SEARCH_API_KEY")
        .map(|k| !k.is_empty() && k != "mock")
        .unwrap_or(false)
    {
        if let Ok(key) = std::env::var("E2E_BRAVE_API_KEY") {
            if !key.is_empty() {
                unsafe { std::env::set_var("SEARCH_API_KEY", key) };
            }
        }
    }
    if !std::env::var("SEARCH_PROVIDER")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        unsafe { std::env::set_var("SEARCH_PROVIDER", "brave_llm_context") };
    }
    if !std::env::var("SEARCH_BASE_URL")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        unsafe { std::env::set_var("SEARCH_BASE_URL", "https://api.search.brave.com") };
    }
}

/// Max attempts for non-deterministic real-LLM chat/search calls.
pub const REAL_LLM_MAX_ATTEMPTS: usize = 2;

const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

/// Retry chat until HTTP 200 with a non-empty answer, or exhaust attempts.
pub async fn chat_with_retry(
    ctx: &TestContext,
    query: &str,
    notebook_id: &str,
    doc_scope: &[String],
) -> (crate::product_e2e::HttpResponse, crate::product_e2e::ChatResponse) {
    use crate::product_e2e::assertions::assert_http_ok;

    let mut last_http = None;
    for attempt in 1..=REAL_LLM_MAX_ATTEMPTS {
        let http_resp = ctx
            .chat(query, notebook_id, doc_scope)
            .await
            .expect("chat request");
        last_http = Some(http_resp.clone());

        if http_resp.status != 200 {
            eprintln!(
                "[llm_real] chat attempt {attempt}/{REAL_LLM_MAX_ATTEMPTS} HTTP {}; body={}",
                http_resp.status, http_resp.body_json
            );
            if attempt < REAL_LLM_MAX_ATTEMPTS {
                tokio::time::sleep(RETRY_DELAY).await;
                continue;
            }
            assert_http_ok(&http_resp);
        }

        let resp: crate::product_e2e::ChatResponse = match http_resp.clone().into_business() {
            Ok(resp) => resp,
            Err(err) => {
                eprintln!(
                    "[llm_real] chat attempt {attempt}/{REAL_LLM_MAX_ATTEMPTS} invalid body: {err}; body={}",
                    http_resp.body_json
                );
                if attempt < REAL_LLM_MAX_ATTEMPTS {
                    tokio::time::sleep(RETRY_DELAY).await;
                    continue;
                }
                panic!("valid ChatResponse schema: {err}");
            }
        };

        if !resp.answer.is_empty() {
            return (http_resp, resp);
        }

        eprintln!(
            "[llm_real] chat attempt {attempt}/{REAL_LLM_MAX_ATTEMPTS} empty answer; retrying after {}s",
            RETRY_DELAY.as_secs()
        );
        if attempt < REAL_LLM_MAX_ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        } else {
            return (http_resp, resp);
        }
    }

    let http = last_http.expect("chat produced no response");
    assert_http_ok(&http);
    let resp = http.clone().into_business().expect("valid ChatResponse schema");
    (http, resp)
}

/// Retry search until HTTP 200 with a non-empty answer, or exhaust attempts.
pub async fn search_with_retry(
    ctx: &TestContext,
    query: &str,
    notebook_id: &str,
) -> (crate::product_e2e::HttpResponse, crate::product_e2e::ChatResponse) {
    use crate::product_e2e::assertions::assert_http_ok;

    let mut last_http = None;
    for attempt in 1..=REAL_LLM_MAX_ATTEMPTS {
        let http_resp = ctx.search(query, notebook_id).await.expect("search request");
        last_http = Some(http_resp.clone());

        if http_resp.status != 200 {
            eprintln!(
                "[llm_real] search attempt {attempt}/{REAL_LLM_MAX_ATTEMPTS} HTTP {}; body={}",
                http_resp.status, http_resp.body_json
            );
            if attempt < REAL_LLM_MAX_ATTEMPTS {
                tokio::time::sleep(RETRY_DELAY).await;
                continue;
            }
            assert_http_ok(&http_resp);
        }

        let resp: crate::product_e2e::ChatResponse = match http_resp.clone().into_business() {
            Ok(resp) => resp,
            Err(err) => {
                eprintln!(
                    "[llm_real] search attempt {attempt}/{REAL_LLM_MAX_ATTEMPTS} invalid body: {err}; body={}",
                    http_resp.body_json
                );
                if attempt < REAL_LLM_MAX_ATTEMPTS {
                    tokio::time::sleep(RETRY_DELAY).await;
                    continue;
                }
                panic!("valid ChatResponse schema: {err}");
            }
        };

        if !resp.answer.is_empty() && resp.degrade_trace.is_empty() {
            return (http_resp, resp);
        }

        eprintln!(
            "[llm_real] search attempt {attempt}/{REAL_LLM_MAX_ATTEMPTS} not ready \
             (answer_len={}, degrade={:?}); retrying after {}s",
            resp.answer.len(),
            resp.degrade_trace,
            RETRY_DELAY.as_secs()
        );
        if attempt < REAL_LLM_MAX_ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        } else {
            return (http_resp, resp);
        }
    }

    let http = last_http.expect("search produced no response");
    assert_http_ok(&http);
    let resp = http.clone().into_business().expect("valid ChatResponse schema");
    (http, resp)
}

/// Guard that fails fast if a required real-LLM credential is missing.
fn require_real_llm_config() {
    let required = [
        "AGENT_LLM_BASE_URL",
        "AGENT_LLM_API_KEY",
        "AGENT_LLM_MODEL",
        "EMBEDDING_BASE_URL",
        "EMBEDDING_API_KEY",
        "EMBEDDING_MODEL",
    ];
    for key in &required {
        assert!(
            std::env::var(key).is_ok(),
            "real-LLM test missing required env var: {key}"
        );
    }
}

pub mod rag_real;
pub mod search_real;

impl TestContext {
    /// Create a TestContext that uses the **real** production LLM and embedding
    /// providers.  Uses real Brave Search when SEARCH_API_KEY is configured
    /// and reachable; otherwise falls back to the local mock search server.
    pub async fn new_with_real_llm() -> Self {
        load_env_from_repo_dotenv();
        require_real_llm_config();
        if has_real_search_credentials() {
            ensure_search_defaults();
        }

        // build_smoke with use_real_llm=true:
        // - does not override AGENT_LLM_* / EMBEDDING_* with mock values
        // - does not start mock LLM/Embedding servers
        // - uses real Brave when credentials + connectivity allow, else mock search
        Self::build_smoke(true, 300, None, true).await
    }
}

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

    let mut test_count = 0usize;

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

    for path in &files {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let _meta: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        test_count += 1;
    }

    // NOTE: ChatResponse currently does not expose token counts, so
    // precise cost calculation requires adding a `usage` field to the
    // production response schema.  For now we report test count only.
    // Approximate cost per test (RAG):
    //   LLM: ~3K tokens × ¥0.001/1K = ¥0.003
    //   Embedding: ~1.5K tokens × ¥0.0005/1K = ¥0.00075
    //   ≈ ¥0.004 per test
    let approx_cost_per_test = 0.004_f64;
    let total_cost_cny = test_count as f64 * approx_cost_per_test;

    println!("\n=== Real-LLM E2E Cost Report ===");
    println!("  Artifact files:     {}", files.len());
    println!("  Tests run:          {}", test_count);
    println!("  Est. cost/test:     ¥{:.4}", approx_cost_per_test);
    println!(
        "  Est. total cost:    ¥{:.4} ({:.4} USD @ 7.2)",
        total_cost_cny,
        total_cost_cny / 7.2
    );
    println!("  NOTE: precise token counts not yet available in ChatResponse schema.");

    // Monthly budget threshold: ¥10 CNY (~$1.40 USD)
    const MONTHLY_BUDGET_CNY: f64 = 10.0;
    if total_cost_cny > MONTHLY_BUDGET_CNY {
        eprintln!(
            "\n⚠️ WARNING: estimated cost ¥{:.2} exceeds monthly budget ¥{:.2}!",
            total_cost_cny, MONTHLY_BUDGET_CNY
        );
    }
}
