# avrag-rs

`avrag-rs` 是 `context-osv6` 的 Rust 工作区，实现 M1（Workspace 基础设施）+ M2（RAG 知识库问答）核心功能。

正式前端：
- `../frontend_next/` 是当前正式前端实现，负责承接 Rust API、SSE、citation lookup、正文内联 citation 与图片块渲染。
- `../frontend_rust/` 是历史 Rust 前端工程；`avrag-api` 不再提供 Leptos SSR fallback。

## 当前产品架构目标（2026-04-26）

最新目标架构见：[Current Product Architecture: Main Agent + RAG API + Milvus Retrieval Plane](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md)。

核心方向：
- `Main Agent` 是唯一用户级 agent，负责 mode routing、记忆、指代消解、RAG tool planning 与最终回答。
- `RAG API` 是检索服务，不是自主 agent；它可以使用模型做三元组抽取、query entity extraction、relation/path rerank、chunk rerank 等检索子任务。
- Postgres 保留产品控制面：用户、组织、workspace、权限、会话、任务、审计、用量、计费。
- Milvus 作为统一检索数据面：BM25 sparse、文本向量、多模态向量、KG entities、KG relations、graph passages。
- 旧文档中以 Qdrant/Tantivy/PostgreSQL BM25 为最终目标的描述均视为历史实现或过渡状态。

## 已实现模块

| 模块 | 路径 | 说明 |
|------|------|------|
| **RAG Runtime** | `crates/rag-core/` | Execute-plan 检索内核，通过 Milvus data plane 执行 BM25 sparse、text dense、multimodal dense 与 graph relation retrieval |
| **LLM** | `crates/llm/` | EmbeddingClient、RetrievalPlanner、AnswerSynthesizer、RerankerClient（OpenAI 兼容协议） |
| **Storage PG** | `crates/storage-pg/` | PostgreSQL 全量操作：documents/chunks/sessions/chat_memory/notifications/audit_log |
| **Cache Redis** | `crates/cache-redis/` | DocumentLock 分布式锁、TTL 支持 |
| **Ingestion** | `crates/ingestion/` | ParserFactory（PDF/Office/代码）、Chunker、Summary extraction、Worker skeleton |
| **Search** | `crates/search/` | Exa API 集成、Web search planning + synthesis |
| **Auth** | `crates/auth/` | OrgId、ActorId、AuthContext、Permission 检查 |
| **Billing** | `crates/billing/` | Stripe checkout/portal/webhook/subscription |
| **Analytics** | `crates/analytics/` | Product events、cost events、daily rollups、first-pass anomaly helpers |
| **Admin** | `crates/admin/` | 组织/用户/用量/封锁/健康检查 |
| **Share** | `crates/share/` | Token 生成、成员验证 |
| **Transport HTTP** | `crates/transport-http/` | Router、handlers、SSE streaming、rate limiting、`/metrics` Prometheus exposition |
| **App** | `crates/app/` | AppState bootstrap、chat 路由（RAG/general/search 模式） |

## 三种 Agent 模式

| 模式 | agent_type 参数 | 说明 |
|------|----------------|------|
| **RAG** | `"rag"` | Dense + Sparse hybrid retrieval → synthesizer |
| **General** | `"general"` | 无检索，synthesizer 直接回答（支持 ChatMemory） |
| **Search** | `"search"` | Web search via Exa API → synthesizer |

## 目录结构

```
bins/
  api/       — HTTP API 启动入口
  worker/    — Ingestion worker 启动入口（ParserFactory + real embedding）
crates/
  auth/      — AuthContext、OrgId、ActorId、Permission 检查
  billing/   — Stripe 全链路
  cache-redis/ — DocumentLock 分布式锁
  chatmemory/ — Session summary/user profile/working memory 三层读写
  common/    — 共享模型（ChatRequest/Response、RagPlan、Citation 等）
  guardrails/ — Input/Output guard pipeline（§14 PRD）
  ingestion/ — ParserFactory、Chunker、WorkerRuntime skeleton
  llm/       — LLM client、Embedding、Planner、Synthesizer、Reranker
  analytics/ — Product/cost events、daily rollups、anomaly detection helpers
  rag-core/  — RAG runtime（RAG/general/search 三模式；目标收缩为 RAG API 检索执行内核）
  search/    — Exa web search executor
  share/     — Token-based sharing
  storage-pg/ — PostgreSQL 全量操作
  telemetry/ — Tracing/logging 初始化
  test-kit/  — 测试工具
  transport-http/ — Router、handlers、SSE、rate limit、metrics
```

## 本地运行

```bash
# API server
cargo run -p avrag-api

# Worker
cargo run -p avrag-worker
```

前端默认使用 `../frontend_next`：

```bash
cd ../frontend_next
pnpm install
pnpm typecheck
```

`../frontend_rust` 不再由 `avrag-api` 服务，仅保留为历史工程。

环境变量参考 `.env.example`。

RAG 检索默认使用 Milvus：

```bash
MILVUS_URL=http://127.0.0.1:19530
```

密码重置邮件默认兼容 163 SMTP：

- `EMAIL_PROVIDER=smtp`
- `SMTP_HOST=smtp.163.com`
- `SMTP_PORT=465`
- `SMTP_USER` / `SMTP_PASS`
- `SMTP_FROM`
- `SMTP_FROM_NAME` 可选
- `RESET_CODE_SECRET` 必填

## Observability

- `GET /metrics` 输出 Prometheus text exposition，至少包含 `http_requests_total`、`http_inflight_requests`
- PostgreSQL analytics 表由 `migrations/0019_observability_events.up.sql` 创建：
  `product_events`、`cost_events`、`daily_user_metrics`、`daily_product_metrics`、`user_anomalies`
- worker 可通过 `ANALYTICS_ROLLUP_ENABLED=true` 启用日汇总与异常扫描，频率由 `ANALYTICS_ROLLUP_INTERVAL_SECS` 控制
- 当前 `estimated_cost_cents` 是 first-pass 派生值，直接跟随 `usage_units`

上线后优先观察：

- `/metrics` 是否持续暴露 `http_requests_total`
- `product_events` 是否覆盖注册、登录、上传、URL 导入、chat 完成/失败
- `cost_events` 是否覆盖 graphflow 与 worker summary
- `share_access_logs` 是否持续记录公开 share 页访问；owner 侧汇总会回写到 `daily_user_metrics.shared_kb_open_count`
- `user_anomalies` 是否出现 `request_burst` 或 `failed_chat_loop`

## 测试

```bash
cargo test --workspace
```

E2E 基线环境体检：

```bash
bash scripts/check-e2e-env.sh
```

Citation 最小金标回归（严格模式）：

```bash
export E2E_STRICT_CITATIONS=1
bash scripts/check-e2e-env.sh --strict-citations
npx playwright test e2e/rust-frontend-e2e.spec.ts --config=playwright.config.ts --grep "T08b: RAG citation minimum golden set"
```

## 当前状态（2026-03-20）

- ✅ 全部 52 个 guardrail 测试通过
- ✅ §14 Guardrails 全量实现（Input: prompt injection/privilege escalation/scope; Output: PII/ citation/ harmful content）
- ✅ §26 Rate limiting 全量实现（X-RateLimit-* headers、multi-dimension 检查）
- ✅ §15 Prometheus metrics（rag_quality、rate_limit_active_keys）
- ✅ §35 multi-mode planner（general/search/rag 三模式，SearchExecutor 已接入 RagRuntime）
- ✅ Worker 使用真实 ParserFactory + EmbeddingClient
- ✅ Redis DocumentLock 已接入 worker
- ✅ Legacy RAG fallback 已清理（RAG mode 要求 rag_runtime 配置）
- ⚠️  无远程 git push（无 upstream 配置）
