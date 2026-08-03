use crate::ModelProviderConfig;
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

/// DashScope qwen3-vl-rerank hard limit: at most 100 text documents per
/// request (the provider 400s above that — "total documents should not be
/// larger than 100"). Batching is done client-side here so NO caller can
/// ever exceed the provider limit, regardless of pool size.
pub const DASHSCOPE_VL_RERANK_MAX_DOCS: usize = 100;

/// Conservative per-request cap for the SiliconFlow-compatible `/rerank`
/// endpoint (no documented higher limit; same 100 keeps one merge path).
const OPENAI_RERANK_MAX_DOCS: usize = 100;

/// Batch boundaries for `total` items split at `batch_size` — e.g. 250 items
/// at 100 → [(0,100), (100,200), (200,250)].
fn batch_ranges(total: usize, batch_size: usize) -> Vec<(usize, usize)> {
    (0..total)
        .step_by(batch_size)
        .map(|start| (start, (start + batch_size).min(total)))
        .collect()
}

/// Merge per-batch rankings into one global ranking: score descending, ties
/// broken by original input position (so pre-rerank order survives ties).
/// `batches` is (input offset, batch-local (index, score) results) per batch;
/// `top_n` is applied after the merge.
fn merge_ranked_batches(
    batches: Vec<(usize, Vec<(usize, f32)>)>,
    top_n: usize,
) -> Vec<(usize, f32)> {
    let mut all: Vec<(usize, f32)> = batches
        .into_iter()
        .flat_map(|(offset, results)| {
            results
                .into_iter()
                .map(move |(index, score)| (index + offset, score))
        })
        .collect();
    all.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    all.truncate(top_n);
    all
}

pub struct RerankerClient {
    config: ModelProviderConfig,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub enum MultiModalRerankDocument {
    Text(String),
    Image(String),
    Video(String),
}

#[derive(Debug, Clone)]
pub struct MultiModalRerankResult {
    pub index: usize,
    pub score: f32,
}

impl RerankerClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        Self { config, client }
    }

    pub async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> anyhow::Result<Vec<RerankResult>> {
        if !self.config.is_configured() {
            anyhow::bail!("Reranker not configured");
        }

        if self.uses_dashscope_vl_rerank() {
            let mm_documents = documents
                .iter()
                .cloned()
                .map(MultiModalRerankDocument::Text)
                .collect::<Vec<_>>();
            let ranked = self
                .rerank_multimodal_text_query(query, &mm_documents, documents.len())
                .await?;
            return Ok(ranked
                .into_iter()
                .map(|r| RerankResult {
                    index: r.index,
                    document: documents.get(r.index).cloned().unwrap_or_default(),
                    score: r.score,
                })
                .collect());
        }

        // OpenAI-style (/rerank): batch at OPENAI_RERANK_MAX_DOCS so no
        // caller can exceed the provider's per-request document limit, then
        // merge the batch rankings by score.
        let mut batches = Vec::new();
        for (start, end) in batch_ranges(documents.len(), OPENAI_RERANK_MAX_DOCS) {
            let results = self
                .openai_rerank_once(query, &documents[start..end])
                .await?;
            batches.push((
                start,
                results.into_iter().map(|r| (r.index, r.score)).collect(),
            ));
        }
        let merged = merge_ranked_batches(batches, documents.len());
        Ok(merged
            .into_iter()
            .map(|(index, score)| RerankResult {
                index,
                document: documents.get(index).cloned().unwrap_or_default(),
                score,
            })
            .collect())
    }

    /// One OpenAI-style `/rerank` request for at most
    /// `OPENAI_RERANK_MAX_DOCS` documents.
    async fn openai_rerank_once(
        &self,
        query: &str,
        documents: &[String],
    ) -> anyhow::Result<Vec<RerankResult>> {
        let request_body = json!({
            "model": self.config.model,
            "query": query,
            "documents": documents,
        });

        let response = self
            .client
            .post(format!(
                "{}/rerank",
                self.config.base_url.trim_end_matches('/')
            ))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send rerank request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Rerank API error {}: {}", status, body);
        }

        #[derive(Deserialize)]
        struct RerankResponse {
            results: Vec<RerankResultItem>,
        }

        #[derive(Deserialize)]
        struct RerankResultItem {
            index: usize,
            relevance_score: f32,
        }

        let resp: RerankResponse = response
            .json()
            .await
            .context("Failed to parse rerank response")?;

        Ok(resp
            .results
            .into_iter()
            .map(|r| RerankResult {
                index: r.index,
                document: documents.get(r.index).cloned().unwrap_or_default(),
                score: r.relevance_score,
            })
            .collect())
    }

    pub async fn rerank_multimodal_text_query(
        &self,
        query: &str,
        documents: &[MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<MultiModalRerankResult>> {
        if !self.config.is_configured() {
            anyhow::bail!("Reranker not configured");
        }
        if !self.uses_dashscope_vl_rerank() && !self.uses_openai_vl_rerank() {
            anyhow::bail!(
                "rerank_multimodal_text_query requires a qwen3-vl-rerank or openai_vl_rerank config"
            );
        }

        // Batch at the provider's 100-documents-per-request limit and merge
        // the batch rankings by score (scores are model-scale comparable
        // across batches). Each batch asks for ALL its documents back so the
        // global merge sees every candidate before applying top_n.
        let mut batches = Vec::new();
        for (start, end) in batch_ranges(documents.len(), DASHSCOPE_VL_RERANK_MAX_DOCS) {
            let batch = &documents[start..end];
            let results = if self.uses_openai_vl_rerank() {
                self.openai_vl_rerank_once(query, batch, batch.len())
                    .await?
            } else {
                self.dashscope_vl_rerank_once(query, batch, batch.len())
                    .await?
            };
            batches.push((
                start,
                results.into_iter().map(|r| (r.index, r.score)).collect(),
            ));
        }
        let merged = merge_ranked_batches(batches, top_n.min(documents.len()));
        Ok(merged
            .into_iter()
            .map(|(index, score)| MultiModalRerankResult { index, score })
            .collect())
    }

    /// One DashScope qwen3-vl-rerank request for at most
    /// `DASHSCOPE_VL_RERANK_MAX_DOCS` documents.
    async fn dashscope_vl_rerank_once(
        &self,
        query: &str,
        documents: &[MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<MultiModalRerankResult>> {
        let request_body = json!({
            "model": self.config.model,
            "input": {
                "query": { "text": query },
                "documents": documents.iter().map(multimodal_document_to_json).collect::<Vec<_>>()
            },
            "parameters": {
                "return_documents": false,
                "top_n": top_n,
                "instruct": "Given a web search query, retrieve relevant passages that answer the query."
            }
        });

        let response = self
            .client
            .post(&self.config.base_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send multimodal rerank request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Multimodal rerank API error {}: {}", status, body);
        }

        #[derive(Deserialize)]
        struct DashScopeRerankResponse {
            output: DashScopeRerankOutput,
        }

        #[derive(Deserialize)]
        struct DashScopeRerankOutput {
            results: Vec<DashScopeRerankItem>,
        }

        #[derive(Deserialize)]
        struct DashScopeRerankItem {
            index: usize,
            relevance_score: f32,
        }

        let resp: DashScopeRerankResponse = response
            .json()
            .await
            .context("Failed to parse multimodal rerank response")?;

        Ok(resp
            .output
            .results
            .into_iter()
            .map(|result| MultiModalRerankResult {
                index: result.index,
                score: result.relevance_score,
            })
            .collect())
    }

    /// One SiliconFlow Qwen3-VL-Reranker-8B request: OpenAI-shaped `POST
    /// {base}/rerank` with a bare-string `query` and multimodal `documents`
    /// object array (`[{text}, {image}]`). Response reuses the OpenAI rerank
    /// `results[index, relevance_score]` shape.
    async fn openai_vl_rerank_once(
        &self,
        query: &str,
        documents: &[MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<MultiModalRerankResult>> {
        let request_body = json!({
            "model": self.config.model,
            "query": query,
            "documents": documents.iter().map(multimodal_document_to_json).collect::<Vec<_>>(),
            "top_n": top_n,
        });

        let response = self
            .client
            .post(format!(
                "{}/rerank",
                self.config.base_url.trim_end_matches('/')
            ))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send OpenAI-VL multimodal rerank request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI-VL multimodal rerank API error {}: {}", status, body);
        }

        #[derive(Deserialize)]
        struct OpenAiRerankResponse {
            results: Vec<OpenAiRerankItem>,
        }
        #[derive(Deserialize)]
        struct OpenAiRerankItem {
            index: usize,
            relevance_score: f32,
        }

        let resp: OpenAiRerankResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI-VL multimodal rerank response")?;

        Ok(resp
            .results
            .into_iter()
            .map(|result| MultiModalRerankResult {
                index: result.index,
                score: result.relevance_score,
            })
            .collect())
    }

    fn uses_dashscope_vl_rerank(&self) -> bool {
        matches!(
            self.config.api_style,
            Some(crate::ApiStyle::DashScopeVlRerank)
        ) || self.config.model == "qwen3-vl-rerank"
    }

    fn uses_openai_vl_rerank(&self) -> bool {
        self.config.api_style == Some(crate::ApiStyle::OpenAiVlRerank)
    }
}

fn multimodal_document_to_json(document: &MultiModalRerankDocument) -> serde_json::Value {
    match document {
        MultiModalRerankDocument::Text(text) => json!({ "text": text }),
        MultiModalRerankDocument::Image(image) => json!({ "image": image }),
        MultiModalRerankDocument::Video(video) => json!({ "video": video }),
    }
}

#[derive(Debug, Clone)]
pub struct RerankResult {
    pub index: usize,
    pub document: String,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OpenAiVlRerank (SiliconFlow Qwen3-VL-Reranker-8B) branch: body sends a
    /// bare-string `query` + multimodal `documents` object array + `top_n`;
    /// response is OpenAI `results[index, relevance_score]`; merged ranking
    /// honors input order ties.
    #[tokio::test]
    async fn openai_vl_rerank_sends_object_documents_and_merges() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let calls = Arc::new(AtomicUsize::new(0));
        let call_counter = calls.clone();
        let app = Router::new().route(
            "/rerank",
            post(move |Json(req): Json<serde_json::Value>| {
                call_counter.fetch_add(1, Ordering::SeqCst);
                let query = req["query"].as_str().unwrap_or("").to_string();
                let docs = req["documents"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let has_object_text = docs.iter().any(|v| v.get("text").is_some());
                let has_object_image = docs.iter().any(|v| v.get("image").is_some());
                let dim = docs.len();
                let results: Vec<serde_json::Value> = (0..dim)
                    .map(|i| json!({"index": dim - 1 - i, "relevance_score": (i as f32 + 1.0) / 10.0}))
                    .collect();
                async move {
                    assert!(!query.is_empty(), "query must be a bare string");
                    assert!(has_object_text, "documents must include text object");
                    assert!(has_object_image, "documents must include image object");
                    Json(json!({ "results": results }))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock openai-vl rerank listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = RerankerClient::new(ModelProviderConfig {
            base_url,
            api_key: "sk-test".to_string(),
            model: "Qwen/Qwen3-VL-Reranker-8B".to_string(),
            timeout_ms: 5_000,
            api_style: Some(crate::ApiStyle::OpenAiVlRerank),
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });

        let docs = vec![
            MultiModalRerankDocument::Text("速冻设备".to_string()),
            MultiModalRerankDocument::Image("http://example.com/img.png".to_string()),
        ];
        let ranked = client
            .rerank_multimodal_text_query("速冻机", &docs, 2)
            .await
            .expect("openai-vl rerank");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(ranked.len(), 2);
        // Mock scores: index0=0.2, index1=0.1 → merged desc by score → [0, 1].
        assert_eq!(ranked[0].index, 0);
        assert_eq!(ranked[1].index, 1);
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn batch_ranges_split_250_into_3_batches() {
        assert_eq!(
            batch_ranges(250, DASHSCOPE_VL_RERANK_MAX_DOCS),
            vec![(0, 100), (100, 200), (200, 250)]
        );
        // Pools at or under the cap stay a single request (no behavior change).
        assert_eq!(
            batch_ranges(100, DASHSCOPE_VL_RERANK_MAX_DOCS),
            vec![(0, 100)]
        );
        assert_eq!(
            batch_ranges(50, DASHSCOPE_VL_RERANK_MAX_DOCS),
            vec![(0, 50)]
        );
        assert!(batch_ranges(0, DASHSCOPE_VL_RERANK_MAX_DOCS).is_empty());
    }

    #[test]
    fn merge_ranked_batches_orders_score_desc_with_input_position_tiebreak() {
        // Three batches (offsets 0/100/200); scores deliberately scrambled.
        // The doc at input rank 150 (batch 1, local 50) has the highest score
        // and must come out first — the case the old single-request code lost
        // when the provider 400'd an over-100 pool.
        let batches = vec![
            (0, vec![(0, 0.5), (1, 0.9), (2, 0.1)]),
            (100, vec![(0, 0.7), (50, 0.99), (51, 0.7)]),
            (200, vec![(0, 0.9), (1, 0.3)]),
        ];
        let merged = merge_ranked_batches(batches, 6);
        let indices: Vec<usize> = merged.iter().map(|(i, _)| *i).collect();
        // 150 first (0.99); then the two 0.9s in input order (1 before 200);
        // then the two 0.7s in input order (100 before 151); then 0.5.
        assert_eq!(indices, vec![150, 1, 200, 100, 151, 0]);
    }

    #[test]
    fn merge_ranked_batches_applies_top_n_after_merge() {
        let batches = vec![
            (0, vec![(0, 0.4), (1, 0.6)]),
            (100, vec![(0, 0.9), (1, 0.8)]),
        ];
        let merged = merge_ranked_batches(batches, 2);
        assert_eq!(merged, vec![(100, 0.9), (101, 0.8)]);
    }

    #[test]
    fn merge_ranked_batches_single_batch_matches_legacy_order() {
        // One batch (pool ≤ cap): merge must reproduce the plain score-desc
        // ranking the old single-request path produced.
        let batches = vec![(0, vec![(2, 0.1), (0, 0.9), (1, 0.5)])];
        let merged = merge_ranked_batches(batches, 3);
        assert_eq!(merged, vec![(0, 0.9), (1, 0.5), (2, 0.1)]);
    }

    /// Live provider proof for the ≤100-doc batching: 150 documents through
    /// the public `rerank` entry (same code path dense.rs reaches via the
    /// multimodal stage) must not 400 against the real DashScope endpoint.
    /// Run: `set -a; source .env; set +a; cargo test -p avrag-llm \
    ///   live_dashscope_vl_rerank_over_100_docs -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "live DashScope call; needs MM_RERANK_* in env"]
    async fn live_dashscope_vl_rerank_over_100_docs() {
        let (Ok(base_url), Ok(api_key), Ok(model)) = (
            std::env::var("MM_RERANK_BASE_URL"),
            std::env::var("MM_RERANK_API_KEY"),
            std::env::var("MM_RERANK_MODEL"),
        ) else {
            eprintln!("MM_RERANK_* not set — skipping live rerank test");
            return;
        };
        // Mirror production construction (app-bootstrap make_reranker).
        let client = RerankerClient::new(ModelProviderConfig {
            base_url,
            api_key,
            model,
            timeout_ms: 60_000,
            api_style: Some(crate::ApiStyle::DashScopeVlRerank),
            dimensions: None,
            enable_thinking: Some(false),
            enable_cache: Some(false),
            rpm_limit: None,
            tpm_limit: None,
        });

        // 150 documents: one planted clearly-relevant answer at the END (it
        // lands in the second batch), the rest plausible-but-generic.
        let mut documents: Vec<String> = (0..149)
            .map(|i| {
                format!(
                    "速冻机 型号 FZ-{i:03} 采用通用风冷结构，适用于果蔬预冷与常规冷冻工艺，\
                     产能参数以厂家标定为准，交货周期 30 天。"
                )
            })
            .collect();
        documents.push(
            "2T 隧道式速冻机日产能在标准工况下约为 2 吨/日，采用连续网带输送与强制冷风循环，\
             适用于小规模速冻食品加工。"
                .to_string(),
        );
        assert_eq!(documents.len(), 150);

        let ranked = client
            .rerank("2T 速冻机日产能", &documents)
            .await
            .expect("rerank of 150 docs must not 400 after batching");
        assert!(!ranked.is_empty(), "empty ranking from live rerank");
        eprintln!(
            "live rerank ok: {} docs in → {} ranked out; top index={} score={:.4}",
            documents.len(),
            ranked.len(),
            ranked[0].index,
            ranked[0].score
        );
        // The planted doc (index 149, second batch) surfacing near the top
        // proves the cross-batch merge works against the real provider.
        let top10: Vec<usize> = ranked.iter().take(10).map(|r| r.index).collect();
        assert!(
            top10.contains(&149),
            "planted relevant doc 149 missing from top10: {top10:?}"
        );
    }
}

#[async_trait::async_trait]
impl avrag_rag_core_ports::RerankPort for RerankerClient {
    async fn rerank(
        &self,
        query: &str,
        documents: &[&str],
    ) -> anyhow::Result<Vec<avrag_rag_core_ports::RerankResult>> {
        let owned: Vec<String> = documents.iter().map(|s| (*s).to_string()).collect();
        let ranked = RerankerClient::rerank(self, query, &owned).await?;
        Ok(ranked
            .into_iter()
            .map(|r| avrag_rag_core_ports::RerankResult {
                index: r.index,
                score: r.score,
            })
            .collect())
    }

    async fn rerank_multimodal_text_query(
        &self,
        query: &str,
        documents: &[avrag_rag_core_ports::MultiModalRerankDocument],
        top_n: usize,
    ) -> anyhow::Result<Vec<avrag_rag_core_ports::RerankResult>> {
        let mapped: Vec<MultiModalRerankDocument> = documents
            .iter()
            .map(|d| match d {
                avrag_rag_core_ports::MultiModalRerankDocument::Text(t) => {
                    MultiModalRerankDocument::Text(t.clone())
                }
                avrag_rag_core_ports::MultiModalRerankDocument::Image(i) => {
                    MultiModalRerankDocument::Image(i.clone())
                }
                avrag_rag_core_ports::MultiModalRerankDocument::Video(v) => {
                    MultiModalRerankDocument::Video(v.clone())
                }
            })
            .collect();
        let ranked = RerankerClient::rerank_multimodal_text_query(
            self,
            query,
            &mapped,
            top_n.max(1).min(mapped.len().max(1)),
        )
        .await?;
        Ok(ranked
            .into_iter()
            .map(|r| avrag_rag_core_ports::RerankResult {
                index: r.index,
                score: r.score,
            })
            .collect())
    }
}
