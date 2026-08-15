//! W2 contract: desktop relay tokens + `/v1/relay/*` metered relay.
//!
//! Coverage (no live provider calls; upstream is a local mock):
//! - `POST/GET /api/v1/desktop/tokens` + `/revoke` — session-JWT CRUD, redacted list.
//! - Relay guard: missing/garbage/session-JWT/revoked tokens → 401.
//! - Chat relay: SSE streamed through verbatim, model pinned server-side,
//!   `stream_options.include_usage` forced, upstream auth = platform api_key,
//!   actual usage (incl. cache/reasoning split) metered → wallet debited.
//! - Preflight: empty wallet → 402 before any upstream call.
//! - Embeddings relay: `usage.total_tokens` metered.
//! - Model off the price whitelist → structured config error (no silent free ride).
//! - Unconfigured platform upstream → 503.

use std::sync::{Arc, Mutex, OnceLock};

use app_bootstrap::AppState;
use app_core::{
    ApplyLedgerInput, ApplyLedgerResult, AppConfig, Wallet, WalletLedgerEntry, WalletStorePort,
};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
};
use common::AppError;
use tower::ServiceExt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shared test lock (env + mock capture are process-global)
// ---------------------------------------------------------------------------

static TEST_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Mock platform upstream (DeepSeek/SiliconFlow stand-in)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CapturedCall {
    path: String,
    authorization: Option<String>,
    body: serde_json::Value,
}

struct MockUpstream {
    calls: Arc<Mutex<Vec<CapturedCall>>>,
}

static MOCK: OnceLock<MockUpstream> = OnceLock::new();

fn chat_sse_body() -> &'static str {
    concat!(
        "data: {\"id\":\"chatcmpl-mock\",\"model\":\"deepseek-v4-flash\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"你好\"}}]}\r\n\r\n",
        "data: {\"id\":\"chatcmpl-mock\",\"model\":\"deepseek-v4-flash\",\"choices\":[],",
        "\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":20,\"total_tokens\":120,",
        "\"prompt_cache_hit_tokens\":64,\"prompt_cache_miss_tokens\":36,",
        "\"completion_tokens_details\":{\"reasoning_tokens\":8}}}\r\n\r\n",
        "data: [DONE]\r\n\r\n"
    )
}

async fn mock_chat_completions(
    State(state): State<Arc<Mutex<Vec<CapturedCall>>>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let authorization = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    state.lock().unwrap().push(CapturedCall {
        path: "/chat/completions".to_string(),
        authorization,
        body: body.clone(),
    });
    if stream {
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            chat_sse_body(),
        )
            .into_response()
    } else {
        Json(serde_json::json!({
            "id": "chatcmpl-mock",
            "object": "chat.completion",
            "model": body["model"],
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "你好"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120,
                      "prompt_cache_hit_tokens": 64, "completion_tokens_details": {"reasoning_tokens": 8}}
        }))
        .into_response()
    }
}

async fn mock_embeddings(
    State(state): State<Arc<Mutex<Vec<CapturedCall>>>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let authorization = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    state.lock().unwrap().push(CapturedCall {
        path: "/embeddings".to_string(),
        authorization,
        body,
    });
    Json(serde_json::json!({
        "object": "list",
        "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2]}],
        "model": "BAAI/bge-m3",
        "usage": {"prompt_tokens": 7, "total_tokens": 7}
    }))
    .into_response()
}

/// Spawn the mock upstream once per test binary; point relay env at it.
fn mock() -> &'static MockUpstream {
    MOCK.get_or_init(|| {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = std::sync::mpsc::channel();
        let calls_clone = calls.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let app = Router::new()
                    .route("/chat/completions", axum::routing::post(mock_chat_completions))
                    .route("/embeddings", axum::routing::post(mock_embeddings))
                    .with_state(calls_clone);
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                tx.send(format!("http://{addr}")).unwrap();
                axum::serve(listener, app).await.unwrap();
            });
        });
        let base_url = rx.recv().unwrap();
        // SAFETY: set exactly once per test binary, before any router/readers exist.
        unsafe {
            std::env::set_var("AGENT_LLM_BASE_URL", &base_url);
            std::env::set_var("AGENT_LLM_API_KEY", "sk-platform-test");
            std::env::set_var("AGENT_LLM_MODEL", "deepseek-v4-flash");
            std::env::set_var("EMBEDDING_BASE_URL", &base_url);
            std::env::set_var("EMBEDDING_API_KEY", "sk-embed-test");
            std::env::set_var("EMBEDDING_MODEL", "BAAI/bge-m3");
        }
        MockUpstream { calls }
    })
}

// ---------------------------------------------------------------------------
// Stub wallet + capture observer (metering wiring assertions)
// ---------------------------------------------------------------------------

struct StubWallet {
    balance_fen: Mutex<i64>,
    applied_keys: Mutex<std::collections::HashSet<String>>,
}

impl StubWallet {
    fn with_balance(fen: i64) -> Self {
        Self {
            balance_fen: Mutex::new(fen),
            applied_keys: Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Simulate a user whose signup grant was already consumed.
    fn grant_consumed_empty(user_id: Uuid) -> Self {
        let wallet = Self::with_balance(0);
        wallet
            .applied_keys
            .lock()
            .unwrap()
            .insert(app_core::signup_grant_idempotency_key(user_id));
        wallet
    }

    fn balance(&self) -> i64 {
        *self.balance_fen.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl WalletStorePort for StubWallet {
    async fn get_wallet(&self, user_id: Uuid) -> Result<Option<Wallet>, AppError> {
        Ok(Some(self.ensure_wallet(user_id).await?))
    }

    async fn ensure_wallet(&self, user_id: Uuid) -> Result<Wallet, AppError> {
        Ok(Wallet {
            user_id,
            balance_fen: *self.balance_fen.lock().unwrap(),
            lifetime_paid_topup_fen: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    async fn apply_ledger_entry(
        &self,
        input: &ApplyLedgerInput,
    ) -> Result<ApplyLedgerResult, AppError> {
        let snapshot = |balance_fen: i64| Wallet {
            user_id: input.user_id,
            balance_fen,
            lifetime_paid_topup_fen: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        if self
            .applied_keys
            .lock()
            .unwrap()
            .contains(&input.idempotency_key)
        {
            return Ok(ApplyLedgerResult {
                wallet: snapshot(self.balance()),
                applied: false,
                ledger_id: Uuid::new_v4(),
            });
        }
        let mut balance = self.balance_fen.lock().unwrap();
        let new_balance = *balance + input.amount_fen;
        if new_balance < 0 {
            return Err(AppError::validation(
                "wallet_insufficient_balance",
                "insufficient wallet balance",
            ));
        }
        *balance = new_balance;
        drop(balance);
        self.applied_keys
            .lock()
            .unwrap()
            .insert(input.idempotency_key.clone());
        Ok(ApplyLedgerResult {
            wallet: snapshot(new_balance),
            applied: true,
            ledger_id: Uuid::new_v4(),
        })
    }

    async fn list_ledger(
        &self,
        _user_id: Uuid,
        _limit: i64,
    ) -> Result<Vec<WalletLedgerEntry>, AppError> {
        Ok(Vec::new())
    }
}

/// Captured usage-event fields (relay → PgUsageObserver → insert_llm_usage_event).
#[derive(Debug, Clone)]
struct CapturedUsage {
    provider: String,
    model: String,
    prompt_tokens: u32,
    completion_tokens: u32,
    cached_tokens: u32,
    reasoning_tokens: u32,
    usage_kind: String,
    billable: bool,
}

/// Stub usage-event store so tests exercise the REAL `PgUsageObserver`
/// (usage insert + wallet debit), same wiring as production bootstrap.
#[derive(Default)]
struct StubUsageLimitStore {
    events: Mutex<Vec<CapturedUsage>>,
}

#[async_trait::async_trait]
impl app_core::UsageLimitStorePort for StubUsageLimitStore {
    async fn insert_llm_usage_event(
        &self,
        _ctx: &app_core::MeteringContext,
        record: app_core::UsageLimitUsageRecord<'_>,
    ) -> Result<i64, AppError> {
        self.events.lock().unwrap().push(CapturedUsage {
            provider: record.provider.to_string(),
            model: record.model.to_string(),
            prompt_tokens: record.prompt_tokens,
            completion_tokens: record.completion_tokens,
            cached_tokens: record.cached_tokens,
            reasoning_tokens: record.reasoning_tokens,
            usage_kind: record.usage_kind.to_string(),
            billable: record.billable,
        });
        Ok(1)
    }

    async fn load_user_override(
        &self,
        _user_id: Uuid,
    ) -> Result<Option<app_core::UsageLimitOverrideRow>, AppError> {
        Ok(None)
    }

    async fn get_user_plan(&self, _user_id: Uuid) -> Result<String, AppError> {
        Ok("free".to_string())
    }

    async fn load_plan_policy(
        &self,
        _plan_id: &str,
    ) -> Result<Option<app_core::UsageLimitPlanPolicyRow>, AppError> {
        Ok(None)
    }

    async fn sum_usage_units_since(
        &self,
        _user_id: Uuid,
        _since: chrono::DateTime<chrono::Utc>,
    ) -> Result<i64, AppError> {
        Ok(0)
    }

    async fn oldest_usage_event_since(
        &self,
        _user_id: Uuid,
        _since: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, AppError> {
        Ok(None)
    }

    async fn load_usage_breakdown(
        &self,
        _user_id: Uuid,
        _since: chrono::DateTime<chrono::Utc>,
    ) -> Result<std::collections::HashMap<String, i64>, AppError> {
        Ok(std::collections::HashMap::new())
    }

    async fn load_model_rates(
        &self,
        _provider: &str,
        _model: &str,
    ) -> Result<(f64, f64, f64), AppError> {
        Ok((1.0, 0.02, 2.0))
    }

    async fn has_user_override(&self, _user_id: Uuid) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn has_estimated_usage(&self, _user_id: Uuid) -> Result<bool, AppError> {
        Ok(false)
    }
}

/// Billing context wired like production bootstrap: PgUsageObserver + wallet.
fn metered_billing(
    wallet: Arc<StubWallet>,
    usage_store: Arc<StubUsageLimitStore>,
) -> app_billing::BillingContext {
    let observer = app_billing::PgUsageObserver::new(usage_store).with_wallet(wallet.clone());
    app_billing::BillingContext::new(None, "off".to_string())
        .with_wallet(wallet)
        .with_usage_observer(Arc::new(observer))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn memory_state() -> AppState {
    AppState::new(AppConfig::default())
}

fn session_bearer(user_id: Uuid) -> String {
    transport_http::issue_jwt(&user_id, &user_id)
}

fn json_post(uri: &str, bearer: Option<&str>, body: serde_json::Value) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Mint a desktop token through the real session-JWT endpoint.
async fn mint_token_via_http(app: &Router, bearer: &str, name: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(json_post(
            "/api/v1/desktop/tokens",
            Some(bearer),
            serde_json::json!({"name": name}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body["ok"].as_bool().unwrap(), "mint failed: {body}");
    body["data"].clone()
}

fn captured_calls_for(mock: &'static MockUpstream, path: &str) -> Vec<CapturedCall> {
    mock.calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call.path == path)
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn desktop_token_crud_roundtrip() {
    let _guard = TEST_LOCK.lock().unwrap();
    mock();
    let app = transport_http::build_router(memory_state());
    let user_id = Uuid::new_v4();
    let bearer = session_bearer(user_id);

    // Mint: plaintext returned once, cos_dt_ shape.
    let minted = mint_token_via_http(&app, &bearer, "测试笔记本").await;
    let token = minted["token"].as_str().unwrap();
    assert!(token.starts_with("cos_dt_"));
    assert_eq!(token.len(), "cos_dt_".len() + 32);
    let token_id = minted["id"].as_str().unwrap().to_string();
    assert_eq!(minted["name"].as_str().unwrap(), "测试笔记本");

    // List: redacted (no token field), prefix present.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/desktop/tokens")
                .header("authorization", format!("Bearer {bearer}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list = body_json(response).await;
    let rows = list["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].get("token").is_none(), "list must be redacted");
    assert!(token.starts_with(rows[0]["prefix"].as_str().unwrap()));

    // No session → middleware 401.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/desktop/tokens")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Revoke.
    let response = app
        .clone()
        .oneshot(json_post(
            &format!("/api/v1/desktop/tokens/{token_id}/revoke"),
            Some(&bearer),
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    let revoked = body_json(response).await;
    assert!(revoked["ok"].as_bool().unwrap());
    assert!(revoked["data"]["revoked_at"].is_string());

    // Another user's revoke misses the row.
    let other = session_bearer(Uuid::new_v4());
    let response = app
        .clone()
        .oneshot(json_post(
            &format!("/api/v1/desktop/tokens/{token_id}/revoke"),
            Some(&other),
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"].as_str().unwrap(), "desktop_token_not_found");
}

#[tokio::test(flavor = "current_thread")]
async fn relay_guard_rejects_missing_garbage_jwt_and_revoked_tokens() {
    let _guard = TEST_LOCK.lock().unwrap();
    mock();
    let app = transport_http::build_router(memory_state());
    let user_id = Uuid::new_v4();
    let bearer = session_bearer(user_id);
    let chat_body = serde_json::json!({"model": "whatever", "messages": []});

    // Missing Authorization.
    let response = app
        .clone()
        .oneshot(json_post("/v1/relay/chat/completions", None, chat_body.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "desktop_token_required");

    // Garbage token.
    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/relay/chat/completions",
            Some("cos_dt_deadbeefdeadbeefdeadbeefdeadbeef"),
            chat_body.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "invalid_desktop_token");

    // A session JWT is NOT a desktop token (fail closed on the relay lane).
    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/relay/chat/completions",
            Some(&bearer),
            chat_body.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Revoked token.
    let minted = mint_token_via_http(&app, &bearer, "soon-revoked").await;
    let desktop_token = minted["token"].as_str().unwrap().to_string();
    let token_id = minted["id"].as_str().unwrap().to_string();
    let response = app
        .clone()
        .oneshot(json_post(
            &format!("/api/v1/desktop/tokens/{token_id}/revoke"),
            Some(&bearer),
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/relay/chat/completions",
            Some(&desktop_token),
            chat_body,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "invalid_desktop_token");
}

#[tokio::test(flavor = "current_thread")]
async fn relay_chat_streams_verbatim_pins_model_and_meters() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mock = mock();

    let mut state = memory_state();
    let wallet = Arc::new(StubWallet::with_balance(0));
    let usage_store = Arc::new(StubUsageLimitStore::default());
    state.test_set_billing(metered_billing(wallet.clone(), usage_store.clone()));
    let app = transport_http::build_router(state);

    let user_id = Uuid::new_v4();
    let bearer = session_bearer(user_id);
    let minted = mint_token_via_http(&app, &bearer, "streaming-laptop").await;
    let desktop_token = minted["token"].as_str().unwrap().to_string();

    let marker = format!("relay-test-{}", Uuid::new_v4());
    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/relay/chat/completions",
            Some(&desktop_token),
            serde_json::json!({
                "model": "gpt-4o", // client choice ignored — 型号固定
                "stream": true,
                "user": marker,
                "messages": [{"role": "user", "content": "ping"}]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(content_type.contains("text/event-stream"));
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    // Streamed through verbatim, including the [DONE] sentinel.
    assert_eq!(body, chat_sse_body());

    // Upstream saw: pinned model, forced stream_options, platform api_key.
    let calls = captured_calls_for(mock, "/chat/completions");
    let call = calls
        .iter()
        .find(|call| call.body["user"] == serde_json::json!(marker))
        .expect("mock captured the relayed chat call");
    assert_eq!(call.body["model"], "deepseek-v4-flash");
    assert_eq!(call.body["stream_options"]["include_usage"], true);
    assert_eq!(
        call.authorization.as_deref(),
        Some("Bearer sk-platform-test"),
        "upstream auth must be the platform key, never the desktop token"
    );

    // Metering: usage event row with cache/reasoning split; wallet debited at ×1.5.
    let events = usage_store.events.lock().unwrap();
    let event = events
        .iter()
        .find(|event| event.usage_kind == "chat" && event.model == "deepseek-v4-flash")
        .expect("chat usage event recorded");
    assert_eq!(event.prompt_tokens, 100);
    assert_eq!(event.completion_tokens, 20);
    assert_eq!(event.cached_tokens, 64);
    assert_eq!(event.reasoning_tokens, 8);
    assert!(event.billable);
    // Provider label derives from the upstream base_url (mock loopback → "unknown");
    // the whitelist match rides on the pinned model name.
    assert_eq!(event.provider, "unknown");
    drop(events);
    // signup grant 2000 fen − 1 fen list price (flash, tiny usage ceils to 1).
    assert_eq!(wallet.balance(), 1999);
}

#[tokio::test(flavor = "current_thread")]
async fn relay_chat_insufficient_balance_refused_before_upstream() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mock = mock();

    let user_id = Uuid::new_v4();
    let mut state = memory_state();
    // Wallet at 0 with the signup grant already consumed → preflight must refuse.
    let wallet = Arc::new(StubWallet::grant_consumed_empty(user_id));
    state.test_set_billing(
        app_billing::BillingContext::new(None, "off".to_string()).with_wallet(wallet),
    );
    let app = transport_http::build_router(state);

    let bearer = session_bearer(user_id);
    let minted = mint_token_via_http(&app, &bearer, "broke-laptop").await;
    let desktop_token = minted["token"].as_str().unwrap().to_string();

    let marker = format!("relay-test-{}", Uuid::new_v4());
    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/relay/chat/completions",
            Some(&desktop_token),
            serde_json::json!({
                "model": "ignored",
                "stream": true,
                "user": marker,
                "messages": [{"role": "user", "content": "ping"}]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "payer_funds_required");

    // Preflight refused BEFORE any upstream call.
    assert!(
        captured_calls_for(mock, "/chat/completions")
            .iter()
            .all(|call| call.body["user"] != serde_json::json!(marker)),
        "upstream must not be called when the wallet is empty"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn relay_embeddings_meters_actual_tokens() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mock = mock();

    let mut state = memory_state();
    let wallet = Arc::new(StubWallet::with_balance(0));
    let usage_store = Arc::new(StubUsageLimitStore::default());
    state.test_set_billing(metered_billing(wallet.clone(), usage_store.clone()));
    let app = transport_http::build_router(state);

    let user_id = Uuid::new_v4();
    let bearer = session_bearer(user_id);
    let minted = mint_token_via_http(&app, &bearer, "embed-laptop").await;
    let desktop_token = minted["token"].as_str().unwrap().to_string();

    let marker = format!("relay-test-{}", Uuid::new_v4());
    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/relay/embeddings",
            Some(&desktop_token),
            serde_json::json!({"model": "ignored", "user": marker, "input": "hello relay"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    // Response passed through verbatim.
    assert_eq!(body["usage"]["total_tokens"], 7);
    assert_eq!(body["data"][0]["embedding"][0], 0.1);

    // Upstream got the pinned embedding model + platform embed key.
    let calls = captured_calls_for(mock, "/embeddings");
    let call = calls
        .iter()
        .find(|call| call.body["user"] == serde_json::json!(marker))
        .expect("mock captured the relayed embeddings call");
    assert_eq!(call.body["model"], "BAAI/bge-m3");
    assert_eq!(call.authorization.as_deref(), Some("Bearer sk-embed-test"));

    // Metering: actual total_tokens recorded; wallet debited (bge-m3 whitelist).
    let events = usage_store.events.lock().unwrap();
    let event = events
        .iter()
        .find(|event| event.usage_kind == "embedding_multimodal")
        .expect("embedding usage event recorded");
    assert_eq!(event.model, "BAAI/bge-m3");
    assert_eq!(event.prompt_tokens, 7);
    drop(events);
    assert_eq!(wallet.balance(), 1999);
}

#[tokio::test(flavor = "current_thread")]
async fn relay_model_not_whitelisted_is_config_error() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = memory_state();
    let user_id = Uuid::new_v4();
    let minted = app_core::mint_desktop_token(&state.desktop_token_store(), user_id, "wl")
        .await
        .unwrap();

    let service = transport_http::RelayService::from_upstreams(
        Some(transport_http::RelayUpstream {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-test".to_string(),
            model: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            timeout_ms: 5_000,
        }),
        None,
    );
    let app = transport_http::build_relay_router(state.clone(), service).with_state(state);

    let response = app
        .oneshot(json_post(
            "/v1/relay/chat/completions",
            Some(&minted.token),
            serde_json::json!({"messages": []}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "relay_model_not_whitelisted");
}

#[tokio::test(flavor = "current_thread")]
async fn desktop_relay_config_reports_public_base_and_pinned_models() {
    let _guard = TEST_LOCK.lock().unwrap();
    mock();
    // SAFETY: guarded by TEST_LOCK; no other test in this binary reads this var.
    unsafe {
        std::env::set_var("AVRAG_PUBLIC_BASE_URL", "https://app.contextlm.top/");
    }
    let app = transport_http::build_router(memory_state());
    let user_id = Uuid::new_v4();
    let bearer = session_bearer(user_id);

    // No session → middleware 401.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/desktop/relay-config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/desktop/relay-config")
                .header("authorization", format!("Bearer {bearer}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body["ok"].as_bool().unwrap(), "relay-config failed: {body}");
    // Trailing slash on the public base is trimmed before the /v1/relay suffix.
    assert_eq!(
        body["data"]["relay_base_url"].as_str().unwrap(),
        "https://app.contextlm.top/v1/relay"
    );
    assert_eq!(body["data"]["chat_model"], "deepseek-v4-flash");
    assert_eq!(body["data"]["embedding_model"], "BAAI/bge-m3");
}

#[tokio::test(flavor = "current_thread")]
async fn relay_unconfigured_upstream_is_503() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = memory_state();
    let user_id = Uuid::new_v4();
    let minted = app_core::mint_desktop_token(&state.desktop_token_store(), user_id, "na")
        .await
        .unwrap();

    let service = transport_http::RelayService::from_upstreams(None, None);
    let app = transport_http::build_relay_router(state.clone(), service).with_state(state);

    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/relay/chat/completions",
            Some(&minted.token),
            serde_json::json!({"messages": []}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "relay_upstream_not_configured");

    let response = app
        .oneshot(json_post(
            "/v1/relay/embeddings",
            Some(&minted.token),
            serde_json::json!({"input": "x"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
