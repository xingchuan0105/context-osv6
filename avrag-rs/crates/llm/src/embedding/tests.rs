//! Embedding client tests (moved out of the client module for the L1
//! file-size gate; no coverage change).
use super::*;

    use super::*;

    #[test]
    fn fuse_part_vectors_single_passthrough() {
        let v = fuse_part_vectors(&[vec![3.0, 4.0, 0.0]]).unwrap();
        assert_eq!(v, vec![3.0, 4.0, 0.0]);
    }

    #[test]
    fn fuse_part_vectors_mean_l2() {
        let v = fuse_part_vectors(&[vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]]).unwrap();
        let inv = 1.0f32 / 2.0f32.sqrt();
        assert!((v[0] - inv).abs() < 1e-5);
        assert!((v[1] - inv).abs() < 1e-5);
        assert!(v[2].abs() < 1e-5);
    }

    #[test]
    fn test_model_provider_config_is_configured() {
        let empty = ModelProviderConfig {
            base_url: "".to_string(),
            api_key: "".to_string(),
            model: "test".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        assert!(!empty.is_configured());

        let configured = ModelProviderConfig {
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-test".to_string(),
            model: "test".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        assert!(configured.is_configured());
    }

    #[test]
    fn test_multimodal_input_counts_modalities() {
        let input = MultiModalEmbeddingInput::text_image("diagram", "https://example.com/a.png");
        assert_eq!(super::build_dashscope_multimodal_contents(&input).len(), 2);
    }

    #[test]
    fn test_multimodal_token_estimate_uses_image_count() {
        let single = MultiModalEmbeddingInput::text_image("cap", "data:image/jpeg;base64,abc");
        let multi = MultiModalEmbeddingInput::text_images(
            "pages 1-4",
            vec![
                "data:image/jpeg;base64,a".to_string(),
                "data:image/jpeg;base64,b".to_string(),
                "data:image/jpeg;base64,c".to_string(),
                "data:image/jpeg;base64,d".to_string(),
            ],
        );
        assert!(estimate_mm_tokens(&multi) > estimate_mm_tokens(&single));
        assert!(estimate_mm_tokens(&multi) >= 4 * default_image_token_estimate());
    }

    #[test]
    fn test_pure_image_estimate_not_100_tokens() {
        let input = MultiModalEmbeddingInput {
            text: None,
            image: Some("data:image/jpeg;base64,abc".to_string()),
            images: Vec::new(),
            video: None,
        };
        assert!(estimate_mm_tokens(&input) >= default_image_token_estimate());
    }

    #[test]
    fn test_build_dashscope_contents_uses_separate_image_entries() {
        let input = MultiModalEmbeddingInput::text_images(
            "pages 1-4",
            vec!["img-a".to_string(), "img-b".to_string()],
        );
        let contents = super::build_dashscope_multimodal_contents(&input);
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["text"], "pages 1-4");
        assert_eq!(contents[1]["image"], "img-a");
        assert_eq!(contents[2]["image"], "img-b");
    }

    #[test]
    fn test_dashscope_multimodal_detection_by_model() {
        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url: "https://dashscope.aliyuncs.com/api/v1/services/embeddings/multimodal-embedding/multimodal-embedding".to_string(),
            api_key: "sk-test".to_string(),
            model: "qwen3-vl-embedding".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });
        assert!(client.uses_dashscope_multimodal_embedding());
    }

    /// OpenAiVlEmbedding (SiliconFlow Qwen3-VL-Embedding-8B) branch: body has
    /// multimodal `input` object array + `dimensions`, response is OpenAI-shaped
    /// `data[].embedding`, and the returned vector dim matches.
    #[tokio::test]
    async fn openai_vl_embedding_sends_object_array_and_dimensions() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let calls = Arc::new(AtomicUsize::new(0));
        let call_counter = calls.clone();
        let app = Router::new().route(
            "/embeddings",
            post(move |Json(req): Json<serde_json::Value>| {
                call_counter.fetch_add(1, Ordering::SeqCst);
                // body: {model, input:[{image},{text}], dimensions}
                let input = req["input"].as_array().cloned().unwrap_or_default();
                let has_image = input.iter().any(|v| v.get("image").is_some());
                let has_text = input.iter().any(|v| v.get("text").is_some());
                let dim = req["dimensions"].as_u64().unwrap_or(1024) as usize;
                let vector: Vec<f32> = (0..dim).map(|i| 0.05 + i as f32 * 0.001).collect();
                let data = vec![json!({ "embedding": vector })];
                async move {
                    assert!(has_image, "multimodal input must include image object");
                    assert!(has_text, "multimodal input must include text object");
                    Json(json!({ "data": data }))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock openai-vl listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url,
            api_key: "sk-test".to_string(),
            model: "Qwen/Qwen3-VL-Embedding-8B".to_string(),
            timeout_ms: 5_000,
            api_style: Some(crate::ApiStyle::OpenAiVlEmbedding),
            dimensions: Some(1024),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });

        let input = MultiModalEmbeddingInput {
            text: Some("速冻设备".to_string()),
            images: vec!["http://example.com/img.png".to_string()],
            ..Default::default()
        };
        let vector = client
            .embed_multimodal_fused(&input, Some(1024))
            .await
            .expect("openai-vl multimodal embed");
        assert_eq!(vector.len(), 1024, "vector dim must match requested dimensions");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// SF returns per-item vectors for mixed input (no server-side fusion):
    /// the client must fuse so the caption contributes — mean of L2-normalized
    /// parts, renormalized. Mock returns data[[1,0,0],[0,1,0]] → [1/√2,1/√2,0].
    #[tokio::test]
    async fn openai_vl_embedding_fuses_per_item_vectors() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;

        let app = Router::new().route(
            "/embeddings",
            post(|_req: Json<serde_json::Value>| async move {
                Json(json!({
                    "data": [
                        { "embedding": [1.0, 0.0, 0.0] },
                        { "embedding": [0.0, 1.0, 0.0] }
                    ]
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock openai-vl fusion listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url,
            api_key: "sk-test".to_string(),
            model: "Qwen/Qwen3-VL-Embedding-8B".to_string(),
            timeout_ms: 5_000,
            api_style: Some(crate::ApiStyle::OpenAiVlEmbedding),
            dimensions: Some(3),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });

        let input = MultiModalEmbeddingInput {
            text: Some("caption".to_string()),
            images: vec!["http://example.com/img.png".to_string()],
            ..Default::default()
        };
        let vector = client
            .embed_multimodal_fused(&input, Some(3))
            .await
            .expect("openai-vl fused embed");
        let inv_sqrt2 = 1.0f32 / 2.0f32.sqrt();
        assert_eq!(vector.len(), 3);
        assert!((vector[0] - inv_sqrt2).abs() < 1e-5, "x={}", vector[0]);
        assert!((vector[1] - inv_sqrt2).abs() < 1e-5, "y={}", vector[1]);
        assert!(vector[2].abs() < 1e-5, "z={}", vector[2]);
    }

    /// Single-item response passes through untouched (DashScope parity — no
    /// client-side normalization when there is nothing to fuse).
    #[tokio::test]
    async fn openai_vl_embedding_single_item_passes_through() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;

        let app = Router::new().route(
            "/embeddings",
            post(|_req: Json<serde_json::Value>| async move {
                Json(json!({ "data": [{ "embedding": [3.0, 4.0, 0.0] }] }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock openai-vl single listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url,
            api_key: "sk-test".to_string(),
            model: "Qwen/Qwen3-VL-Embedding-8B".to_string(),
            timeout_ms: 5_000,
            api_style: Some(crate::ApiStyle::OpenAiVlEmbedding),
            dimensions: Some(3),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });

        let vector = client
            .embed_multimodal_fused(&MultiModalEmbeddingInput::text("仅文本"), Some(3))
            .await
            .expect("openai-vl single embed");
        assert_eq!(vector, vec![3.0, 4.0, 0.0]);
    }

    /// OpenAiVlRerank style must NOT route through the multimodal embedding path.
    #[tokio::test]
    async fn openai_vl_rerank_style_is_not_multimodal_embedding() {
        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url: "http://127.0.0.1:1".to_string(),
            api_key: "sk-test".to_string(),
            model: "Qwen/Qwen3-VL-Reranker-8B".to_string(),
            timeout_ms: 1000,
            api_style: Some(crate::ApiStyle::OpenAiVlRerank),
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });
        assert!(!client.uses_dashscope_multimodal_embedding());
        assert!(!client.uses_openai_vl_embedding());
        assert!(client.embed_multimodal_fused(&MultiModalEmbeddingInput::default(), None).await.is_err());
    }

    /// Same input text → Redis cache hit → mock embedding HTTP called once.
    ///
    /// Skips when Redis is unavailable (no `TEST_REDIS_URL` / local docker).
    #[tokio::test]
    async fn embed_openai_compatible_text_caches_in_redis() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let redis_url = std::env::var("TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let cache: Arc<dyn avrag_rag_core_ports::CachePort> =
            match avrag_cache_redis::CacheStore::new(&redis_url) {
                Ok(cache) => Arc::new(cache),
                Err(error) => {
                    eprintln!(
                        "skip embed_openai_compatible_text_caches_in_redis: redis unavailable: {error}"
                    );
                    return;
                }
            };

        let http_calls = Arc::new(AtomicUsize::new(0));
        let call_counter = http_calls.clone();
        let app = Router::new().route(
            "/embeddings",
            post(move |Json(req): Json<serde_json::Value>| {
                call_counter.fetch_add(1, Ordering::SeqCst);
                let texts = req["input"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                let dim = req["dimensions"].as_u64().unwrap_or(8) as usize;
                let vector: Vec<f32> = (0..dim).map(|i| 0.1 + i as f32 * 0.01).collect();
                let data: Vec<serde_json::Value> =
                    texts.iter().map(|_| json!({"embedding": vector})).collect();
                async move { Json(json!({ "data": data })) }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock embedding listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Unique model key avoids collisions with prior Redis runs of this test.
        let model = format!("mock-embedding-{}", uuid::Uuid::new_v4());
        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url: base_url.clone(),
            api_key: "sk-test".to_string(),
            model,
            timeout_ms: 5_000,
            api_style: None,
            dimensions: Some(8),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        })
        .with_cache(cache);

        let text = "cache-me-once";
        let first = client.embed(&[text]).await.expect("first embed");
        assert_eq!(
            http_calls.load(Ordering::SeqCst),
            1,
            "first embed should call the mock provider"
        );
        let second = client.embed(&[text]).await.expect("second embed");
        assert_eq!(first, second);
        assert_eq!(
            http_calls.load(Ordering::SeqCst),
            1,
            "second identical embed should hit Redis, not call HTTP again"
        );
    }

    /// Regression (2026-08-03 SiliconFlow migration): when `dimensions` is not
    /// configured (bge-m3 rejects the field with 400 code:20015), the request body
    /// must NOT contain a `dimensions` key — provider returns its native dim.
    #[tokio::test]
    async fn embed_omits_dimensions_when_unset() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let calls = Arc::new(AtomicUsize::new(0));
        let call_counter = calls.clone();
        let app = Router::new().route(
            "/embeddings",
            post(move |Json(req): Json<serde_json::Value>| {
                call_counter.fetch_add(1, Ordering::SeqCst);
                assert!(
                    req.get("dimensions").is_none(),
                    "request must not carry dimensions when unset: {req}"
                );
                let texts = req["input"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                let vector: Vec<f32> = (0..8).map(|i| 0.1 + i as f32 * 0.01).collect();
                let data: Vec<serde_json::Value> =
                    texts.iter().map(|_| json!({ "embedding": vector })).collect();
                async move { Json(json!({ "data": data })) }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock embedding listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url,
            api_key: "sk-test".to_string(),
            model: "Pro/BAAI/bge-m3".to_string(),
            timeout_ms: 5_000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });

        let vectors = client.embed(&["one", "two"]).await.expect("embed without dimensions");
        assert_eq!(vectors.len(), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// Provider returning fewer embeddings than texts must fail loud, not silently
    /// truncate. No cache/Redis required: the HTTP mock responds with 2 embeddings
    /// for a 3-text request, so the count assertion in `embed_openai_compatible_text`
    /// (and the subsequent guard in `embed`) must surface an `Err`.
    #[tokio::test]
    async fn embed_fails_when_provider_returns_too_few_vectors() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;

        // Always respond with exactly 2 embedding entries, regardless of request size.
        let app = Router::new().route(
            "/embeddings",
            post(|_req: Json<serde_json::Value>| async move {
                let dim = 8usize;
                let vector: Vec<f32> = (0..dim).map(|i| 0.1 + i as f32 * 0.01).collect();
                let data: Vec<serde_json::Value> =
                    (0..2).map(|_| json!({ "embedding": vector })).collect();
                Json(json!({ "data": data }))
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock embedding listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // No `.with_cache(...)` → request always hits the HTTP mock.
        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url,
            api_key: "sk-test".to_string(),
            model: "mock-embedding".to_string(),
            timeout_ms: 5_000,
            api_style: None,
            dimensions: Some(8),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });

        // Request 3 texts; mock returns only 2 → must error, never truncate.
        let result = client.embed(&["one", "two", "three"]).await;
        assert!(
            result.is_err(),
            "embed() must return Err when provider returns fewer vectors than texts"
        );
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains("vectors")
                && (message.contains("texts") || message.contains("inputs")),
            "error should explain the count mismatch, got: {message}"
        );
    }

    /// Multi-batch embed (batch size 10) must preserve input order without
    /// quadratic `Vec::insert` reassembly.
    #[tokio::test]
    async fn embed_multi_batch_preserves_order() {
        use axum::{Json, Router, routing::post};
        use serde_json::json;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let http_calls = Arc::new(AtomicUsize::new(0));
        let call_counter = http_calls.clone();
        let app = Router::new().route(
            "/embeddings",
            post(move |Json(req): Json<serde_json::Value>| {
                call_counter.fetch_add(1, Ordering::SeqCst);
                let texts = req["input"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                let data: Vec<serde_json::Value> = texts
                    .iter()
                    .map(|t| {
                        // Encode text length into first dim so order is verifiable.
                        let mut vector = vec![0.0f32; 8];
                        vector[0] = t.len() as f32;
                        json!({ "embedding": vector })
                    })
                    .collect();
                async move { Json(json!({ "data": data })) }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock embedding listener");
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = EmbeddingClient::new(ModelProviderConfig {
            base_url,
            api_key: "sk-test".to_string(),
            model: "mock-multi-batch".to_string(),
            timeout_ms: 5_000,
            api_style: None,
            dimensions: Some(8),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        });

        // 25 texts → 3 batches (10+10+5) with TEXT_EMBEDDING_BATCH_SIZE=10.
        let owned: Vec<String> = (0..25).map(|i| format!("t{i:02}")).collect();
        let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
        let vectors = client.embed(&refs).await.expect("multi-batch embed");
        assert_eq!(vectors.len(), 25);
        assert_eq!(
            http_calls.load(Ordering::SeqCst),
            3,
            "expected three HTTP batches for 25 texts"
        );
        for (i, vector) in vectors.iter().enumerate() {
            assert_eq!(
                vector[0],
                owned[i].len() as f32,
                "vector order mismatch at index {i}"
            );
        }
    }
