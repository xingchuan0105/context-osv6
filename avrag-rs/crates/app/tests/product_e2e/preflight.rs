//! Preflight guards for product E2E smoke ingest safety.

/// Panic when non-test avrag-worker processes are running.
pub fn assert_no_external_workers() {
    let output = std::process::Command::new("pgrep")
        .args(["-af", "avrag-worker"])
        .output()
        .unwrap_or_else(|error| panic!("failed to run pgrep -af avrag-worker: {error}"));

    if !output.status.success() {
        // pgrep exits 1 when no process matched.
        return;
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let mut offenders = Vec::new();
    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.contains("product_e2e") {
            continue;
        }
        let pid = line.split_whitespace().next().unwrap_or("unknown");
        offenders.push(format!("{pid}: {line}"));
    }

    if offenders.is_empty() {
        return;
    }

    panic!(
        "[product_e2e preflight] found external avrag-worker process(es):\n{}\n\
         Stop them before smoke ingest to avoid queue races.\n\
         Suggested fix: pkill -f avrag-worker",
        offenders.join("\n")
    );
}

/// Validate that smoke ingest is not reusing the dev shared database by mistake.
pub fn assert_smoke_database_isolated(pg_url: &str) {
    let normalized = pg_url.trim_end_matches('/');
    let uses_dev_db = normalized
        .split_once('?')
        .map(|(base, _)| base)
        .unwrap_or(normalized)
        .ends_with("/avrag_rs");
    if !uses_dev_db || normalized.contains("e2e_smoke") {
        return;
    }
    if std::env::var("RAG_QUALITY_SMOKE_ALLOW_SHARED_DB").is_ok() {
        return;
    }

    let force_ingest = std::env::var("RAG_QUALITY_SMOKE_FORCE_INGEST")
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));

    if force_ingest {
        panic!(
            "[product_e2e preflight] RAG_QUALITY_SMOKE_FORCE_INGEST=1 is using shared dev DB: {pg_url}\n\
             Set RAG_QUALITY_SMOKE_DATABASE_URL to a dedicated smoke DB, e.g.\n\
             postgres://avrag:avrag@127.0.0.1:5432/avrag_rs_e2e_smoke\n\
             (or set RAG_QUALITY_SMOKE_ALLOW_SHARED_DB=1 to bypass intentionally)"
        );
    }

    eprintln!(
        "[product_e2e preflight] WARNING: smoke ingest is using shared dev DB: {pg_url}\n\
         Recommended: set RAG_QUALITY_SMOKE_DATABASE_URL=postgres://avrag:avrag@127.0.0.1:5432/avrag_rs_e2e_smoke\n\
         (set RAG_QUALITY_SMOKE_ALLOW_SHARED_DB=1 to silence this warning)"
    );
}
