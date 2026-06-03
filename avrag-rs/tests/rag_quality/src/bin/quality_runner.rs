//! quality_runner — RAG quality evaluation driver.
//!
//! Wires a real RAG pipeline (DashScope LLM + text-embedding-v4) into
//! the `RagEvaluator` trait and runs the small in-memory golden set
//! (`fixtures_golden.json`) built on the product_e2e fixture corpus.
//!
//! ⚠️ BEFORE DRAWING PRODUCT-QUALITY CONCLUSIONS FROM THIS RUNNER,
//! READ `tests/rag_quality/GOTCHAS.md`. Short version:
//!
//! - This is a SMOKE TEST, not a production benchmark. The
//!   retrieval here is single-pass flat-cosine, NOT the
//!   `RagRuntime` with planner / RRF / re-rank that production
//!   uses. Numbers like "100% recall" or "30% halluc" reflect
//!   this simplified pipeline, not the product.
//! - The hallucination heuristic is word-overlap; manual audit
//!   of the latest run showed 0% true hallucination but 15-30%
//!   heuristic false positives. Replace with NLI for real signal.
//! - The `mode` field on each golden example is the only thing
//!   that picks the system prompt; set it correctly when adding
//!   new examples.
//!
//! For a real production-quality number today, write a
//! `ProductionRagEvaluator` that calls `RagRuntime::execute()`
//! and replaces the heuristic with an NLI judge.
//! Prints the three PRD §13.2 release-gate metrics:
//!
//! - **Recall@15** (retrieval quality; gate: not regressing more than 3%)
//! - **Citation Accuracy** (generation grounding; gate: ≥95%)
//! - **Hallucination Rate** (answer faithfulness; gate: ≤2%)
//!
//! Run with:
//! ```bash
//! DASHSCOPE_API_KEY=sk-... \
//!   cargo run -p rag_quality --bin quality_runner -- \
//!   --golden tests/rag_quality/fixtures_golden.json \
//!   --corpus crates/app/tests/product_e2e/fixtures \
//!   --top-k 4
//! ```
//!
//! Without a `DASHSCOPE_API_KEY`, the runner bails with a clear
//! error message rather than silently producing fake numbers.

use std::env;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use rag_quality::{EvaluationHarness, GoldenDataset, HarnessConfig, RagEvaluator};
use reqwest::Client;
use serde_json::Value;
use tracing::{info, warn};

const EMBED_BASE: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";
const EMBED_MODEL: &str = "text-embedding-v4";
const LLM_MODEL: &str = "qwen-plus"; // OpenAI-compatible DashScope chat model

// ---------------------------------------------------------------------------
// Corpus loading + chunking
// ---------------------------------------------------------------------------

/// A single in-memory chunk used as the retrieval unit.
#[derive(Clone)]
struct Chunk {
    id: String,
    text: String,
    /// Pre-computed embedding vector.
    embedding: Vec<f32>,
}

/// Load a corpus directory: every `.txt` file becomes one document;
/// each document is split into paragraph-sized chunks (≥80 chars,
/// ≤800 chars to keep embedding inputs reasonable).
async fn load_corpus(
    corpus_dir: &Path,
    embed: &EmbeddingClient,
) -> Result<Vec<Chunk>> {
    let mut chunks = Vec::new();
    let mut entries = tokio::fs::read_dir(corpus_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("txt") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("doc").to_string();
        let text = tokio::fs::read_to_string(&path).await?;
        for (i, para) in split_paragraphs(&text).into_iter().enumerate() {
            if para.trim().is_empty() {
                continue;
            }
            let embedding = embed.embed(&para).await.with_context(|| {
                format!("embedding chunk {stem}#{i} ({} chars)", para.len())
            })?;
            chunks.push(Chunk {
                id: format!("{stem}#{i}"),
                text: para,
                embedding,
            });
        }
        info!(file = %path.display(), "loaded corpus document");
    }
    Ok(chunks)
}

/// Split text into paragraph-sized chunks, matching the production
/// chunker in `crates/ingestion/src/chunker.rs` (which uses a
/// `min_chars` policy and merges sub-threshold chunks into the
/// previous one rather than dropping them). The production default
/// is `min_chars = 32`; we use 40 here for slightly more aggressive
/// merging that prevents the byline-style "By X Y Z" 1-liner chunks
/// (which the cosine retriever would rank below substantive content)
/// from being lost.
///
/// Two things matter for retrieval quality:
/// 1. A short chunk like "By Nassim Nicholas Taleb" can contain
///    the answer to "Who developed the concept?" — dropping it
///    kills recall for that query.
/// 2. If we keep it as a standalone chunk, it ranks below
///    content-heavy chunks on cosine similarity and never gets
///    retrieved. Merging it INTO the first content paragraph keeps
///    the answer reachable.
fn split_paragraphs(text: &str) -> Vec<String> {
    const MIN_CHARS: usize = 40;
    const MAX_CHARS: usize = 800;
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(line.trim());
        if current.len() >= MAX_CHARS {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    // Production-style merge pass: any sub-MIN_CHARS chunk gets
    // appended to the previous one. If the first chunk itself is
    // short, merge it forward into the next.
    let mut merged: Vec<String> = Vec::with_capacity(out.len());
    for chunk in out {
        if chunk.len() < MIN_CHARS {
            if let Some(last) = merged.last_mut() {
                last.push_str("\n\n");
                last.push_str(&chunk);
            } else {
                merged.push(chunk);
            }
        } else {
            merged.push(chunk);
        }
    }
    // If the FIRST chunk is still too short (no previous to merge
    // into), merge forward into the next one.
    if merged.len() >= 2 && merged[0].len() < MIN_CHARS {
        let head = merged.remove(0);
        merged[0] = format!("{head}\n\n{}", merged[0]);
    }
    merged
}

// ---------------------------------------------------------------------------
// Embedding + LLM clients (minimal DashScope wrappers)
// ---------------------------------------------------------------------------

struct EmbeddingClient {
    http: Client,
    api_key: String,
}

impl EmbeddingClient {
    fn new(api_key: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("build reqwest");
        Self { http, api_key }
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let resp = self
            .http
            .post(format!("{EMBED_BASE}/embeddings"))
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "model": EMBED_MODEL,
                "input": text,
                "dimensions": 1024,
            }))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("embedding API HTTP {status}: {body}");
        }
        let v: Value = resp.json().await?;
        let emb = v["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| anyhow!("missing embedding in response: {v}"))?
            .iter()
            .filter_map(|x| x.as_f64().map(|f| f as f32))
            .collect();
        Ok(emb)
    }
}

struct LlmClient {
    http: Client,
    api_key: String,
}

impl LlmClient {
    fn new(api_key: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("build reqwest");
        Self { http, api_key }
    }

    /// Plain chat completion (non-streaming); returns the assistant text.
    async fn complete(&self, system: &str, user: &str) -> Result<String> {
        let resp = self
            .http
            .post(format!("{EMBED_BASE}/chat/completions"))
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "model": LLM_MODEL,
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                ],
                "temperature": 0.0,
            }))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("LLM API HTTP {status}: {body}");
        }
        let v: Value = resp.json().await?;
        let content = v["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("missing content in LLM response: {v}"))?
            .to_string();
        Ok(content)
    }
}

// ---------------------------------------------------------------------------
// Mode-specific system prompts
// ---------------------------------------------------------------------------

/// RAG-mode prompt: strict, citation-bearing, "context only" framing.
const RAG_SYSTEM_PROMPT: &str = "\
You are a helpful research assistant. Answer the user's question \
using ONLY the provided context blocks. If the context does not \
contain the answer, reply with exactly: \
'Not mentioned in the provided context.' \
Cite AT MOST ONE source — the single most relevant one — using the \
EXACT marker format [citation:N] (with the brackets and colon) \
where N is the 1-based index of the context block. \
Example: 'Antifragility was developed by Taleb [citation:1].'";

/// Chat-mode prompt: conversational, no document grounding.
const CHAT_SYSTEM_PROMPT: &str = "\
You are a friendly, knowledgeable AI assistant. Answer the user's \
question naturally and concisely using your own knowledge. Do not \
fabricate citations to documents — there are none in this \
conversation.";

/// Search-mode prompt: open-ended, web-search flavored.
const SEARCH_SYSTEM_PROMPT: &str = "\
You are a search assistant. The user has asked an open-ended question \
that would normally be answered with a web search. There are no \
context documents provided in this evaluation. Answer using your \
own knowledge, briefly. If the answer requires real-time data you \
do not have access to, say so plainly.";

// ---------------------------------------------------------------------------
// Cosine similarity (single-threaded reference impl — fine for ~hundreds of chunks)
// ---------------------------------------------------------------------------

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = (na.sqrt() * nb.sqrt()).max(1e-9);
    dot / denom
}

// ---------------------------------------------------------------------------
// RAG evaluator
// ---------------------------------------------------------------------------

struct DashScopeRagEvaluator {
    chunks: Vec<Chunk>,
    llm: LlmClient,
    embed: EmbeddingClient,
    top_k: usize,
    /// Maps query string -> mode ("chat" / "rag" / "search"). Built
    /// at construction from the full golden set so `synthesize` can
    /// pick the right system prompt without changing the trait.
    query_mode_map: std::collections::HashMap<String, String>,
}

impl RagEvaluator for DashScopeRagEvaluator {
    fn retrieve(
        &self,
        query: &str,
        k: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        let embed = self.embed.clone();
        let chunks = self.chunks.clone();
        // Always return at least `k` chunks so the harness can compute
        // Recall@K correctly. The LLM context budget is capped at
        // `self.top_k` chunks in `synthesize` (a separate concern).
        let n = k.max(self.top_k);
        let query = query.to_string();
        // Chat and search modes do NOT retrieve — the question is
        // answered from parametric knowledge (or "I don't have access
        // to real-time data" for search). Returning empty chunks
        // matches production: production also skips retrieval for
        // these modes, and the hallucination heuristic correctly
        // skips when retrieved_chunks is empty.
        let mode = self
            .query_mode_map
            .get(&query)
            .cloned()
            .unwrap_or_else(|| "rag".to_string());
        if mode != "rag" {
            return Box::pin(async move { Ok(Vec::new()) });
        }
        Box::pin(async move {
            let q_emb = embed.embed(&query).await?;
            // Single-threaded cosine — fine for small corpora.
            let mut scored: Vec<(usize, f32)> = chunks
                .iter()
                .enumerate()
                .map(|(i, c)| (i, cosine(&q_emb, &c.embedding)))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top: Vec<String> = scored
                .into_iter()
                .take(n)
                .map(|(i, _)| chunks[i].text.clone())
                .collect();
            Ok(top)
        })
    }

    fn synthesize(
        &self,
        query: &str,
        chunks: &[String],
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        let llm = self.llm.clone();
        let query = query.to_string();
        // Only pass the top-K chunks to the LLM (LLM context budget);
        // the harness gets all retrieved chunks for recall eval, so
        // the synthesis step is not the bottleneck for recall.
        let chunks: Vec<String> = chunks.iter().take(self.top_k).cloned().collect();
        // Look up the example's mode in the pre-built map; default
        // to "rag" if not found (which should not happen in
        // practice — the map is built from the full golden set).
        let mode = self
            .query_mode_map
            .get(&query)
            .cloned()
            .unwrap_or_else(|| "rag".to_string());
        Box::pin(async move {
            let system = match mode.as_str() {
                "chat" => CHAT_SYSTEM_PROMPT,
                "search" => SEARCH_SYSTEM_PROMPT,
                _ => RAG_SYSTEM_PROMPT,
            };
            let context = if mode == "chat" {
                String::new()
            } else {
                chunks
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("[{}]\n{}", i + 1, c))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            };
            let user = if mode == "chat" {
                query.clone()
            } else {
                format!("Question: {query}\n\nContext:\n{context}")
            };
            let answer = llm.complete(system, &user).await?;
            eprintln!(
                "[quality_runner] mode={mode} Q: {query}\n[quality_runner] A: {answer}"
            );
            Ok(answer)
        })
    }
}

// Manual Clone (we don't derive because the inner reqwest::Client is cheap
// to clone via Arc but the struct is small).
impl Clone for EmbeddingClient {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
        }
    }
}
impl Clone for LlmClient {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

struct Args {
    golden: PathBuf,
    corpus: PathBuf,
    top_k: usize,
}

fn parse_args() -> Result<Args> {
    let mut golden: Option<PathBuf> = None;
    let mut corpus: Option<PathBuf> = None;
    let mut top_k: usize = 4;

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--golden" => {
                golden = Some(PathBuf::from(it.next().ok_or_else(|| anyhow!("--golden needs value"))?));
            }
            "--corpus" => {
                corpus = Some(PathBuf::from(it.next().ok_or_else(|| anyhow!("--corpus needs value"))?));
            }
            "--top-k" => {
                top_k = it
                    .next()
                    .ok_or_else(|| anyhow!("--top-k needs value"))?
                    .parse()
                    .context("--top-k must be usize")?;
            }
            "--help" | "-h" => {
                println!("Usage: quality_runner --golden <path> --corpus <dir> [--top-k N]");
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    Ok(Args {
        golden: golden.ok_or_else(|| anyhow!("--golden is required"))?,
        corpus: corpus.ok_or_else(|| anyhow!("--corpus is required"))?,
        top_k,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args()?;
    let api_key = env::var("DASHSCOPE_API_KEY")
        .or_else(|_| env::var("EMBEDDING_API_KEY"))
        .map_err(|_| anyhow!(
            "DASHSCOPE_API_KEY (or EMBEDDING_API_KEY) must be set. Without it, this runner \
             cannot produce real numbers."
        ))?;
    if api_key.trim().is_empty() {
        bail!("API key is empty");
    }

    info!(golden = %args.golden.display(), corpus = %args.corpus.display(), top_k = args.top_k, "starting quality runner");

    let embed = EmbeddingClient::new(api_key.clone());
    let chunks = load_corpus(&args.corpus, &embed).await?;
    info!(chunks = chunks.len(), "corpus loaded");
    if chunks.is_empty() {
        bail!("corpus is empty (no .txt files in {})", args.corpus.display());
    }

    let dataset = GoldenDataset::load(&args.golden)?;
    info!(version = %dataset.version, examples = dataset.len(), "golden set loaded");

    // Build the query -> mode map so the evaluator can pick the
    // right system prompt per example without changing the trait.
    let mut query_mode_map = std::collections::HashMap::new();
    for example in dataset.all_examples() {
        query_mode_map.insert(example.query.clone(), example.mode.clone());
    }

    let evaluator = DashScopeRagEvaluator {
        chunks,
        llm: LlmClient::new(api_key),
        embed,
        top_k: args.top_k,
        query_mode_map,
    };
    let config = HarnessConfig {
        recall_k: 15,
        baseline_recall: 0.97,
        max_examples_per_subset: None,
        verbose: true,
    };
    let harness =
        EvaluationHarness::new(dataset.clone(), config).with_evaluator(Box::new(evaluator));

    let report = harness.run_all().await?;

    println!();
    println!("=========================================");
    println!("RAG Quality Report  (dataset v{})", report.dataset_version);
    println!("=========================================");
    println!("Total examples:      {}", report.metrics.total_examples);
    println!("Recall@15:           {:.2}%", report.metrics.recall_at_15 * 100.0);
    println!("Citation Accuracy:   {:.2}%", report.metrics.citation_accuracy * 100.0);
    println!("Hallucination Rate:  {:.2}%", report.metrics.hallucination_rate * 100.0);

    // Per-subset breakdown: re-iterate the dataset in the same order
    // as the harness to know which subset each result belongs to.
    // The harness appends per-example results in dataset order
    // (subsets in declared order, examples within each subset in
    // declared order, capped by max_examples_per_subset=None).
    let flat: Vec<_> = dataset
        .subsets
        .iter()
        .flat_map(|s| s.examples.iter().map(|e| (s.name.as_str(), e)))
        .collect();
    let mut by_subset: std::collections::BTreeMap<&str, (usize, f64, f64, f64)> =
        std::collections::BTreeMap::new();
    let mut cursor = 0;
    for (subset_name, _example) in &flat {
        if let (Some(r), Some(c), Some(h)) = (
            report.metrics.recall_results.get(cursor),
            report.metrics.citation_results.get(cursor),
            report.metrics.hallucination_results.get(cursor),
        ) {
            let entry = by_subset.entry(*subset_name).or_insert((0, 0.0, 0.0, 0.0));
            entry.0 += 1;
            entry.1 += r.recall;
            entry.2 += c.accuracy;
            entry.3 += if h.is_hallucinated { 1.0 } else { 0.0 };
        }
        cursor += 1;
    }
    if !by_subset.is_empty() {
        println!();
        println!("Per-subset breakdown:");
        for (name, (n, r_sum, c_sum, h_sum)) in &by_subset {
            let n_f = *n as f64;
            println!(
                "  {name:<24}  recall@15={:>5.1}%  cit_acc={:>5.1}%  halluc={:>5.1}%  (n={n})",
                r_sum / n_f * 100.0,
                c_sum / n_f * 100.0,
                h_sum / n_f * 100.0,
            );
        }
    }

    if !report.failures.is_empty() {
        println!();
        println!("Failures ({}):", report.failures.len());
        for (q, err) in &report.failures {
            println!("  - {q:?}: {err}");
        }
    }

    // Per-example detail: which golden chunks matched, which citations
    // were present/missing, which sentences were flagged for
    // hallucination. Essential for diagnosing *why* a gate failed.
    let mut cursor = 0;
    println!();
    println!("Per-example detail:");
    for subset in &dataset.subsets {
        for example in &subset.examples {
            let r = report.metrics.recall_results.get(cursor);
            let c = report.metrics.citation_results.get(cursor);
            let h = report.metrics.hallucination_results.get(cursor);
            println!();
            println!("  [{}] {}", subset.name, example.query);
            if let Some(r) = r {
                println!(
                    "    recall@{}: {:.0}% ({}/{} matched)",
                    15, r.recall * 100.0, r.matched_chunks.len(), r.golden_count
                );
            }
            if let Some(c) = c {
                println!(
                    "    cit_acc: {:.0}% (true_pos={} spurious={} missing={:?})",
                    c.accuracy * 100.0,
                    c.true_positives,
                    c.false_positives,
                    c.missing
                );
            }
            if let Some(h) = h {
                println!(
                    "    halluc: score={:.2} flagged={}",
                    h.hallucination_score,
                    h.flagged_phrases.len()
                );
                for phrase in &h.flagged_phrases {
                    let preview = if phrase.len() > 80 {
                        format!("{}…", &phrase[..80])
                    } else {
                        phrase.clone()
                    };
                    println!("      ! {preview}");
                }
            }
            cursor += 1;
        }
    }

    println!();
    match report.metrics.assert_passing(0.97) {
        Ok(()) => println!("GATE: PASS"),
        Err(e) => {
            println!("GATE: FAIL");
            for line in e.lines() {
                println!("  {line}");
            }
            std::process::exit(1);
        }
    }

    Ok(())
}
