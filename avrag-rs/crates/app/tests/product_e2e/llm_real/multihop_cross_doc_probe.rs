//! Hard multi-hop cross-doc probe (prompts unchanged).
//!
//! Design goals (vector-unfriendly):
//! 1. **Vocabulary gap**: query is business (审批单); answer is facility site code (HS-S).
//! 2. **Bridge not in query**: CH-XPIPE-09 must be recovered from doc A.
//! 3. **No shared cue word**: asset doc avoids "Region"; query asks 物理站点码 not Region.
//! 4. **Noise doc**: policy text with Atlas-North decoy (customer service), no channels.
//!
//! Docs:
//! - multihop_orion_scheduler.txt  — AP-7741 → CH-XPIPE-09
//! - multihop_workerpool_ops.txt   — CH-XPIPE-09 → RACK-Δ9 → HS-S
//! - multihop_noise_policy.txt     — SLA noise + Atlas-North decoy
//!
//! ```bash
//! E2E_MODE=nightly RETRIEVAL_BACKEND=pgvector cargo test -p app --test product_e2e \
//!   multihop_ap7741_site_code_hard_pgvector --features product-e2e \
//!   -- --ignored --test-threads=1 --nocapture
//! ```

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, TestContext, llm_real::require_nightly_suite};

const DOC_A: &str = "multihop_orion_scheduler.txt";
const DOC_B: &str = "multihop_workerpool_ops.txt";
/// Dilute dense top-k: many channel→site rows + policy decoy (HS-S/AT-N appear without AP-7741).
const DOC_NOISE: &[&str] = &[
    "multihop_noise_policy.txt",
    "multihop_noise_channels_01.txt",
    "multihop_noise_channels_02.txt",
    "multihop_noise_channels_03.txt",
];

/// No channel id, no HS-S, no Helios, no "Region" — only approval id + 物理站点码.
const MULTIHOP_QUERY: &str =
    "审批单 AP-7741 对应作业最终落在哪个物理站点码？请给出站点码，并写出中间通道代号。";

/// Gold: facility site code (not marketing name alone).
const MUST_INCLUDE_SITE: &str = "HS-S";
const MUST_INCLUDE_BRIDGE: &str = "CH-XPIPE-09";
/// Decoy site for wrong channel / noise policy.
const DECOY_SITE: &str = "AT-N";

#[tokio::test]
#[ignore = "real LLM+embedding+triplet; hard multi-hop"]
async fn multihop_ap7741_site_code_hard_pgvector() {
    run_hard_multihop("pgvector").await;
}

#[tokio::test]
#[ignore = "real LLM+embedding+triplet; hard multi-hop"]
async fn multihop_ap7741_site_code_hard_milvus() {
    run_hard_multihop("milvus").await;
}

async fn run_hard_multihop(backend: &str) {
    require_nightly_suite();
    let backend = backend.to_ascii_lowercase();
    unsafe {
        std::env::set_var("RETRIEVAL_BACKEND", &backend);
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "1");
        std::env::set_var("RETRIEVAL_GRAPH_AUGMENT", "1");
    }

    let tag = format!("multihop-v2-{backend}");
    eprintln!("[{tag}] hard multi-hop: AP-7741 -> CH-XPIPE-09 -> HS-S (+ noise)");

    let mut ctx = TestContext::new_with_real_llm().await;

    let upload_a = ingest(&mut ctx, DOC_A, None).await;
    let upload_b = ingest(&mut ctx, DOC_B, Some(&upload_a.workspace_id)).await;
    let mut doc_scope = vec![
        upload_a.document_id.clone(),
        upload_b.document_id.clone(),
    ];
    for noise in DOC_NOISE {
        let u = ingest(&mut ctx, noise, Some(&upload_a.workspace_id)).await;
        doc_scope.push(u.document_id);
    }
    eprintln!(
        "[{tag}] workspace={} docs={} (A/B + {} noise)",
        upload_a.workspace_id,
        doc_scope.len(),
        DOC_NOISE.len()
    );

    if backend == "pgvector" {
        let n_a = count_pgvector_relations(&upload_a.document_id).await;
        let n_b = count_pgvector_relations(&upload_b.document_id).await;
        eprintln!("[{tag}] relations A={n_a} B={n_b}");
    }

    // --- Optional dense-only ablation (same stack, one dense call via normal chat). ---
    // We cannot force tool choice without prompt changes; log tool mix for diagnosis.
    eprintln!("[{tag}] query={MULTIHOP_QUERY}");

    // Up to 3 attempts: hard fixture + noise makes retrieval non-deterministic;
    // also guards pathological off-topic answers (calculator / progress math).
    const MAX_ATTEMPTS: usize = 3;
    let mut last_answer = String::new();
    let mut last_tools: Vec<String> = Vec::new();
    let mut last_chat: Option<ChatResponse> = None;
    let mut pass_meta = serde_json::json!({});

    for attempt in 1..=MAX_ATTEMPTS {
        let http = ctx
            .chat(MULTIHOP_QUERY, &upload_a.workspace_id, &doc_scope)
            .await
            .unwrap_or_else(|e| panic!("chat attempt {attempt}: {e}"));
        let chat: ChatResponse = http
            .into_business()
            .unwrap_or_else(|e| panic!("parse attempt {attempt}: {e}"));

        let answer = chat.answer.clone();
        // Prefer sub-agent tools from mode_debug.workers (real names); fall back
        // to exit tool_results (store bridge often collapses to dense_retrieval).
        let worker_tools = worker_tool_names(&chat);
        let exit_tools: Vec<String> = chat
            .tool_results
            .iter()
            .map(|t| t.tool.clone())
            .collect();
        let tools = if worker_tools.is_empty() {
            exit_tools.clone()
        } else {
            worker_tools.clone()
        };
        let used_lexical = tools.iter().any(|t| t == "lexical_retrieval");
        let used_dense = tools.iter().any(|t| t == "dense_retrieval");
        let used_graph = tools.iter().any(|t| t == "graph_retrieval");
        let multi_tool = tools
            .iter()
            .filter(|t| {
                matches!(
                    t.as_str(),
                    "dense_retrieval" | "lexical_retrieval" | "graph_retrieval"
                )
            })
            .count()
            >= 2;
        let worker_reasoning = worker_reasoning_preview(&chat);
        let thinking_kinds = worker_thinking_kinds(&chat);

        eprintln!(
            "[{tag}] attempt={attempt}/{MAX_ATTEMPTS} worker_tools={worker_tools:?} exit_tools={exit_tools:?} dense={used_dense} lexical={used_lexical} graph={used_graph} multi_tool={multi_tool}"
        );
        if !worker_reasoning.is_empty() || !thinking_kinds.is_empty() {
            eprintln!(
                "[{tag}] attempt={attempt} thinking_kinds={thinking_kinds:?} reasoning_preview={}",
                worker_reasoning.chars().take(280).collect::<String>()
            );
        }
        eprintln!(
            "[{tag}] attempt={attempt} answer_len={} answer=\n{answer}",
            answer.chars().count()
        );

        let has_site = answer.contains(MUST_INCLUDE_SITE);
        let has_bridge = answer.contains(MUST_INCLUDE_BRIDGE);
        let decoy_only = answer.contains(DECOY_SITE) && !has_site;
        let helios_without_code =
            (answer.contains("Helios") || answer.contains("helios")) && !has_site;
        let off_topic = !has_site
            && (tools.iter().any(|t| t == "calculator")
                || answer.contains('%')
                || answer.contains("进度"));

        last_answer = answer.clone();
        last_tools = tools.clone();
        last_chat = Some(chat.clone());
        pass_meta = serde_json::json!({
            "backend": backend,
            "docs": [DOC_A, DOC_B],
            "noise_docs": DOC_NOISE,
            "query": MULTIHOP_QUERY,
            "attempt": attempt,
            "must_site": MUST_INCLUDE_SITE,
            "must_bridge": MUST_INCLUDE_BRIDGE,
            "has_site": has_site,
            "has_bridge": has_bridge,
            "decoy_only": decoy_only,
            "helios_without_code": helios_without_code,
            "off_topic": off_topic,
            "tools": tools,
            "worker_tools": worker_tools,
            "exit_tools": exit_tools,
            "used_lexical": used_lexical,
            "used_dense": used_dense,
            "used_graph": used_graph,
            "multi_tool": multi_tool,
            "thinking_kinds": thinking_kinds,
            "worker_reasoning_chars": worker_reasoning.chars().count(),
            "design": "vocab-gap + site-code gold + 4 noise docs dilute dense top-k",
        });

        if has_site && !decoy_only && !helios_without_code {
            if !has_bridge {
                eprintln!(
                    "[{tag}] soft: missing bridge {MUST_INCLUDE_BRIDGE} (site ok, chain incomplete)"
                );
            }
            if used_dense && !used_lexical && !used_graph {
                eprintln!("[{tag}] soft: dense-only tools under hard fixture+noise");
            }
            if multi_tool || used_lexical {
                eprintln!("[{tag}] note: multi-channel or lexical used");
            }
            ctx.save_llm_artifact(
                &format!("multihop_v2_hard_{backend}"),
                last_chat.as_ref().unwrap(),
                Some(pass_meta),
                None,
            );
            eprintln!("[{tag}] PASS hard multi-hop on attempt {attempt}");
            return;
        }

        eprintln!(
            "[{tag}] attempt={attempt} not yet: has_site={has_site} decoy_only={decoy_only} off_topic={off_topic}"
        );
    }

    if let Some(chat) = last_chat.as_ref() {
        ctx.save_llm_artifact(
            &format!("multihop_v2_hard_{backend}_fail"),
            chat,
            Some(pass_meta),
            None,
        );
    }
    panic!(
        "[{tag}] failed after {MAX_ATTEMPTS} attempts; need site code {MUST_INCLUDE_SITE}\nlast_tools={last_tools:?}\nlast_answer={last_answer}"
    );
}

/// Real sub-agent tool names from `mode_debug.general.workers` (not store bridge).
fn worker_tool_names(chat: &ChatResponse) -> Vec<String> {
    let Some(workers) = chat
        .mode_debug
        .as_ref()
        .and_then(|m| m.general.as_ref())
        .and_then(|g| g.get("workers"))
        .and_then(|w| w.as_array())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for w in workers {
        if let Some(tools) = w.get("tools").and_then(|t| t.as_array()) {
            for t in tools {
                if let Some(name) = t.get("tool").and_then(|n| n.as_str()) {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

fn worker_reasoning_preview(chat: &ChatResponse) -> String {
    let Some(workers) = chat
        .mode_debug
        .as_ref()
        .and_then(|m| m.general.as_ref())
        .and_then(|g| g.get("workers"))
        .and_then(|w| w.as_array())
    else {
        return String::new();
    };
    workers
        .iter()
        .filter_map(|w| {
            w.get("reasoning_summary")
                .and_then(|s| s.as_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn worker_thinking_kinds(chat: &ChatResponse) -> Vec<String> {
    let Some(workers) = chat
        .mode_debug
        .as_ref()
        .and_then(|m| m.general.as_ref())
        .and_then(|g| g.get("workers"))
        .and_then(|w| w.as_array())
    else {
        return Vec::new();
    };
    let mut kinds = Vec::new();
    for w in workers {
        if let Some(steps) = w.get("thinking").and_then(|t| t.as_array()) {
            for s in steps {
                if let Some(k) = s.get("kind").and_then(|v| v.as_str()) {
                    kinds.push(k.to_string());
                }
            }
        }
    }
    kinds
}

async fn ingest(
    ctx: &mut TestContext,
    fixture: &str,
    workspace: Option<&str>,
) -> crate::product_e2e::UploadResponse {
    let upload = if let Some(ws) = workspace {
        ctx.upload_document_to_notebook(fixture, ws)
            .await
            .unwrap_or_else(|e| panic!("upload {fixture}: {e}"))
    } else {
        ctx.upload_document(fixture)
            .await
            .unwrap_or_else(|e| panic!("upload {fixture}: {e}"))
    };
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(600))
        .await
        .unwrap_or_else(|e| panic!("ingest {fixture}: {e}"));
    assert_eq!(
        status,
        DocumentStatus::Completed,
        "{fixture} must complete"
    );
    upload
}

async fn count_pgvector_relations(document_id: &str) -> i64 {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return -1;
    };
    let Ok(pool) = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
    else {
        return -1;
    };
    let row: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM rag_kg_relations WHERE doc_id = $1::uuid",
    )
    .bind(document_id)
    .fetch_one(&pool)
    .await;
    row.map(|r| r.0).unwrap_or(-1)
}
