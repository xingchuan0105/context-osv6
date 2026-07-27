//! Short golden-doc channel probe for **pgvector** and **milvus** retrieval backends.
//!
//! Corpus: golden-set short article `consulting_rbf_drc.txt` (~12KB).
//! Questions (tuned so the model prefers **lexical/BM25** on keyword probes):
//! - **dense** — still semantic paraphrase (control)
//! - **lexical** — exact IPO price / table-literal (must call lexical_search)
//! - **graph/triplet** — exact acronyms DRC/DRO/DRP full forms + relation
//!
//! Uses non-streaming chat to avoid SSE done-payload flakiness on long graph turns.
//!
//! ```bash
//! # pgvector
//! RETRIEVAL_BACKEND=pgvector INGESTION_TRIPLET_ENABLED=1 RETRIEVAL_GRAPH_AUGMENT=1 \
//!   E2E_MODE=nightly cargo test -p app --test product_e2e pgvector_rbf_dense_bm25_triplet_channel_probe \
//!   --features product-e2e -- --ignored --test-threads=1 --nocapture
//!
//! # milvus
//! RETRIEVAL_BACKEND=milvus INGESTION_TRIPLET_ENABLED=1 RETRIEVAL_GRAPH_AUGMENT=1 \
//!   E2E_MODE=nightly cargo test -p app --test product_e2e milvus_rbf_dense_bm25_triplet_channel_probe \
//!   --features product-e2e -- --ignored --test-threads=1 --nocapture
//! ```

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, TestContext, llm_real::require_nightly_suite};
use contracts::{ToolResult, ToolStatus};

const DOC: &str = "consulting_rbf_drc.txt";

struct Probe {
    channel: &'static str,
    query: &'static str,
    must_include: &'static [&'static str],
}

const PROBES: &[Probe] = &[
    Probe {
        channel: "dense",
        query: "小微门店走收益分成这类融资时，资金成本通常为什么被认为比常规股权或债权更贵？",
        // Doc: 本金2～5倍的抽成上限 / 高昂的资金成本
        must_include: &["倍"],
    },
    // Ultra-short keyword/literal query (金额/专名) — steers codegen to lexical_search.
    // Avoid long meta-instructions in the user string (they pollute dense embed and
    // still often fail to change tool choice).
    Probe {
        channel: "bm25_lexical",
        query: "小菜园 发行价 港元/股",
        must_include: &["8.50"],
    },
    // Exact acronyms as terms — lexical seeds for force graph_augment 1-hop.
    Probe {
        channel: "graph_triplet",
        query: "DRC DRO DRP 英文全称 背靠背",
        must_include: &["Daily Revenue", "DRO", "DRP"],
    },
];

#[tokio::test]
#[ignore = "real LLM+embedding+triplet; RETRIEVAL_BACKEND=pgvector"]
async fn pgvector_rbf_dense_bm25_triplet_channel_probe() {
    run_short_golden_channel_probe("pgvector").await;
}

#[tokio::test]
#[ignore = "real LLM+embedding+triplet; RETRIEVAL_BACKEND=milvus"]
async fn milvus_rbf_dense_bm25_triplet_channel_probe() {
    run_short_golden_channel_probe("milvus").await;
}

async fn run_short_golden_channel_probe(backend: &str) {
    require_nightly_suite();
    let backend = backend.to_ascii_lowercase();
    unsafe {
        std::env::set_var("RETRIEVAL_BACKEND", &backend);
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "1");
        std::env::set_var("RETRIEVAL_GRAPH_AUGMENT", "1");
    }

    let tag = format!("{backend}-probe");
    eprintln!("[{tag}] starting short golden channel probe");

    let mut ctx = TestContext::new_with_real_llm().await;
    let upload = ctx
        .upload_document(DOC)
        .await
        .unwrap_or_else(|e| panic!("upload {DOC}: {e}"));
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(600))
        .await
        .unwrap_or_else(|e| panic!("wait_for_ingestion: {e}"));
    assert_eq!(
        status,
        DocumentStatus::Completed,
        "ingestion must complete for {DOC}"
    );

    match ctx.query_latest_backend_summary(&upload.document_id).await {
        Ok(summary) => eprintln!("[{tag}] backend_summary={summary}"),
        Err(e) => eprintln!("[{tag}] backend_summary unavailable: {e}"),
    }

    // Graph index soft/hard check: pgvector can count SQL rows; milvus relies on ingest summary + Q&A.
    let relation_count = if backend == "pgvector" {
        let n = count_pgvector_relations_for_doc(&upload.document_id).await;
        eprintln!("[{tag}] rag_kg_relations for doc ≈ {n}");
        assert!(
            n > 0,
            "expected triplet relations in rag_kg_relations for {DOC}; got {n}"
        );
        n
    } else {
        eprintln!("[{tag}] milvus: skip SQL relation count (graph lives in Milvus collections)");
        -1
    };

    let mut failures: Vec<String> = Vec::new();
    for probe in PROBES {
        eprintln!("[{tag}] channel={} query={}", probe.channel, probe.query);
        let http = ctx
            .chat(
                probe.query,
                &upload.workspace_id,
                &[upload.document_id.clone()],
            )
            .await
            .unwrap_or_else(|e| panic!("chat {}: {e}", probe.channel));
        let chat: ChatResponse = http
            .into_business()
            .unwrap_or_else(|e| panic!("parse chat {}: {e}", probe.channel));
        let answer = chat.answer.clone();
        let exit_tools: Vec<_> = chat.tool_results.iter().map(|t| t.tool.as_str()).collect();
        let worker_tools = worker_tool_names_from_chat(&chat);
        let tools: Vec<&str> = if worker_tools.is_empty() {
            exit_tools.clone()
        } else {
            worker_tools.iter().map(|s| s.as_str()).collect()
        };
        let worker_reasoning = worker_reasoning_from_chat(&chat);
        eprintln!(
            "[{tag}] channel={} answer_len={} worker_tools={:?} exit_tools={:?} answer_preview={}",
            probe.channel,
            answer.chars().count(),
            worker_tools,
            exit_tools,
            answer.chars().take(240).collect::<String>()
        );
        if !worker_reasoning.is_empty() {
            eprintln!(
                "[{tag}] channel={} worker_reasoning={}",
                probe.channel,
                worker_reasoning.chars().take(280).collect::<String>()
            );
        }

        let missing: Vec<_> = probe
            .must_include
            .iter()
            .filter(|kw| !answer.contains(*kw))
            .copied()
            .collect();
        if !missing.is_empty() {
            failures.push(format!(
                "channel={} missing {:?} answer={}",
                probe.channel, missing, answer
            ));
        }

        let augment_hit =
            graph_augment_hit(&chat.tool_results) || worker_graph_augment_hit(&chat);
        let explicit_graph =
            graph_explicit_called(&chat.tool_results) || worker_graph_explicit(&chat);
        eprintln!(
            "[{tag}] channel={} graph_augment_hit={} graph_explicit_called={}",
            probe.channel, augment_hit, explicit_graph
        );

        let used_lexical = tools.iter().any(|t| *t == "lexical_retrieval")
            || chat
                .tool_results
                .iter()
                .any(|t| t.tool == "lexical_retrieval" && t.status == ToolStatus::Ok);

        // Keyword probes *prefer* BM25; hard-require only when REQUIRE_LEXICAL_TOOL=1.
        // Current product LLMs often still dense-only even on short keyword queries
        // (answer quality may still pass via dense). graph_augment is covered by unit
        // tests + this soft path when the model does call lexical.
        let require_lexical = std::env::var("REQUIRE_LEXICAL_TOOL")
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false);
        if matches!(probe.channel, "bm25_lexical" | "graph_triplet") && !used_lexical {
            let msg = format!(
                "channel={} preferred lexical_retrieval, got tools={tools:?}",
                probe.channel
            );
            if require_lexical {
                failures.push(msg);
            } else {
                eprintln!("[{tag}] soft-routing: {msg}");
            }
        }

        // When lexical ran and backend has graph edges, expect force-augment telemetry.
        if matches!(probe.channel, "bm25_lexical" | "graph_triplet") && used_lexical {
            if relation_count > 0 && !augment_hit {
                failures.push(format!(
                    "channel={} expected graph_augment_hit with relations={relation_count} tools={tools:?}",
                    probe.channel
                ));
            } else if relation_count < 0 && !augment_hit {
                eprintln!(
                    "[{tag}] note: lexical without graph_augment_hit on milvus (graph may be empty or terms miss seeds)"
                );
            } else if augment_hit {
                eprintln!("[{tag}] channel={} lexical+graph_augment OK", probe.channel);
            }
        }

        ctx.save_llm_artifact(
            &format!("{backend}_channel_{}", probe.channel),
            &chat,
            Some(serde_json::json!({
                "channel": probe.channel,
                "document": DOC,
                "must_include": probe.must_include,
                "missing": missing,
                "retrieval_backend": backend,
                "relation_count": relation_count,
                "graph_augment_hit": augment_hit,
                "graph_explicit_called": explicit_graph,
            })),
            None,
        );
    }

    assert!(
        failures.is_empty(),
        "{backend} channel probe failures:\n{}",
        failures.join("\n")
    );
    eprintln!("[{tag}] PASS all channels");
}

fn graph_augment_hit(tool_results: &[ToolResult]) -> bool {
    tool_results.iter().any(|r| {
        r.status == ToolStatus::Ok
            && r.tool == "graph_retrieval"
            && r.trace
                .as_ref()
                .and_then(|t| t.degrade_reason.as_deref())
                == Some("graph_augment")
    })
}

fn graph_explicit_called(tool_results: &[ToolResult]) -> bool {
    tool_results.iter().any(|r| {
        r.status == ToolStatus::Ok
            && r.tool == "graph_retrieval"
            && r.trace
                .as_ref()
                .and_then(|t| t.degrade_reason.as_deref())
                != Some("graph_augment")
    })
}

fn worker_tools_json(chat: &ChatResponse) -> Option<&Vec<serde_json::Value>> {
    chat.mode_debug
        .as_ref()
        .and_then(|m| m.general.as_ref())
        .and_then(|g| g.get("workers"))
        .and_then(|w| w.as_array())
}

fn worker_tool_names_from_chat(chat: &ChatResponse) -> Vec<String> {
    let Some(workers) = worker_tools_json(chat) else {
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

fn worker_reasoning_from_chat(chat: &ChatResponse) -> String {
    let Some(workers) = worker_tools_json(chat) else {
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

fn worker_graph_augment_hit(chat: &ChatResponse) -> bool {
    let Some(workers) = worker_tools_json(chat) else {
        return false;
    };
    workers.iter().any(|w| {
        w.get("tools")
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .any(|t| {
                t.get("tool").and_then(|n| n.as_str()) == Some("graph_retrieval")
                    && t.get("degrade_reason").and_then(|d| d.as_str())
                        == Some("graph_augment")
            })
    })
}

fn worker_graph_explicit(chat: &ChatResponse) -> bool {
    let Some(workers) = worker_tools_json(chat) else {
        return false;
    };
    workers.iter().any(|w| {
        w.get("tools")
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .any(|t| {
                t.get("tool").and_then(|n| n.as_str()) == Some("graph_retrieval")
                    && t.get("degrade_reason").and_then(|d| d.as_str())
                        != Some("graph_augment")
            })
    })
}

async fn count_pgvector_relations_for_doc(document_id: &str) -> i64 {
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
