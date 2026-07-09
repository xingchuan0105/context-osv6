# 全功能 E2E 测试指南（Agent 执行手册）

> **受众**：Coding Agent、CI 维护者、发布前验收人。  
> **最后更新**：2026-06-13。  
> **关联文档**：[`e2e-gates.md`](e2e-gates.md)（门禁语义）、[`e2e-analysis-framework.md`](e2e-analysis-framework.md)（测试结果分析 TEAF）、[`e2e-test-registry.yaml`](e2e-test-registry.yaml)（机读测试索引）、[`product-e2e-plan.md`](product-e2e-plan.md)（历史设计与 P0–P14 矩阵）。

本文档从**产品全功能**角度定义「该测什么、用什么依赖、如何并行跑、通过标准是什么」。Agent 在改测试、补测、或回答「还缺什么覆盖」时，**以本文档为单一真相源**。

---

## 0. Agent 快速决策

```
改动了什么？
├─ HTTP/API/Agent/RAG/Ingestion（Rust）→ 跑 PR smoke + 相关 integration 子集
├─ 前端 UI/路由/计费/Legal → 跑 Vitest + Playwright smoke/journey
├─ 真实 LLM/Search/Embedding 行为 → 跑 llm_real（--ignored）+ Playwright skills
├─ 文档解析路由/Paddle/LiteParse → 跑 integration 解析用例 + 可选 paddle_pdf（--ignored）
└─ 发布前全量 → §7「发布门禁清单」
```

**硬规则**：

| 规则 | 说明 |
|------|------|
| Mock vs Real | PR smoke **禁止**真实 LLM/Search/Embedding；基础设施 PG/Milvus/Object Store **必须真实** |
| 真实四件套 | **真实文档解析**、**真实 LLM RAG**、**真实 Chat**、**真实 WebSearch** 只在 Integration（解析）与 Nightly/Skills（LLM+Search）层验收 |
| 并行 | 无共享 PG/Milvus 状态的模块可并行；RAG 冷启动与 `shared_rag_fixture` 模块必须串行 |
| 凭证 | 从 `avrag-rs/.env` 读取，禁止向用户重复索要已配置项 |
| Milvus 前置 | RAG 类测试前执行 `./scripts/e2e-precheck.sh` 或 `docker compose -f docker-compose.milvus.yml up -d` |

---

## 1. 测试分层总览

| 层 | ID | 触发 | 外部依赖 | 时长预算 | 入口命令 |
|----|-----|------|----------|----------|----------|
| **L1 PR Smoke** | `smoke` | 每个 PR | Mock LLM/Search/Embedding；真 PG/Milvus/Worker | ≤10 min | `./scripts/run-product-smoke-e2e.sh` |
| **L2 Integration** | `integration` | `master` push / 手动 | 同上 Mock；真基础设施；**真实解析管线**（LiteParse/Paddle mock jobs） | ≤15 min | `E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1` |
| **L3 Nightly Real** | `nightly` / `llm_real` | cron / 手动 | **真实 LLM + Embedding + Brave Search**；真基础设施 | ≤60 min | `E2E_MODE=nightly cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1` |
| **L4 Playwright Skills** | `skills` | nightly cron / 手动 | 真实栈 + 浏览器；RAG/Search **硬 citation** | ≤45 min | `cd frontend_next && pnpm exec playwright test --project=skills` |
| **L5 Playwright Journey** | `journey` | PR/手动 | 真实栈；WebSearch citation PR 软 / nightly 硬 | 按需 | `pnpm exec playwright test --project=journey` |
| **L6 单元/契约** | `unit` | PR | 无 Docker 或轻量 PG | 并行 | 见 §6 |

**CI 映射**（仓库根 `.github/workflows/`）：

| Workflow | 层 | 备注 |
|----------|-----|------|
| `smoke-e2e.yml` | L1 | PR 路径过滤 `avrag-rs/**` |
| `integration-e2e.yml` | L2 | `on.push.branches: [master, main]` |
| `nightly-llm-real.yml` | L3 | `SEARCH_REQUIRE_REAL=1` |
| `frontend-skills.yml` | L4 | 02:00 UTC |
| `frontend-journey.yml` / `frontend-smoke.yml` | L5 | 见各 workflow |
| `frontend-unit.yml` | L6 | Vitest |

> PR-1（2026-06-29）：`frontend-journey` / `frontend-skills` / `frontend-smoke` / `nightly-llm-real` 均通过 `scripts/ci-start-milvus.sh` 在测试前预启 Milvus（修复 journey/skills 上传→RAG 因 Milvus 未就绪导致的假绿/flaky）。

---

## 2. 全功能覆盖矩阵

按**产品能力**列出必须验证的行为、现有测试、依赖类型、并行组。  
图例：**M**=Mock 依赖，**R**=真实外部 API，**I**=真实基础设施（PG/Milvus/Worker），**P**=真实解析（worker 真跑，Paddle/LiteParse 可 mock endpoint）。

### 2.1 核心对话（Chat）

| 能力 | 验收标准 | 现有测试 | 层 | 依赖 | 并行组 |
|------|----------|----------|-----|------|--------|
| 通用对话（无检索） | HTTP 200；`answer` 非空；SSE `start`→`done` | `smoke::chat_smoke` | L1 | M+I | **G-parallel-smoke** |
| 通用对话（真实 LLM） | 非空回答；无 mock 路由；产物落盘 | `llm_real::chat_real`（general）；`llm_real::rag_real`（RAG） | L3 | R+I | **G-serial-llm** |
| **真实 Chat（端到端）** | 浏览器发消息；历史持久化；模式指示器 | `journey/workspace-chat.spec.ts`（general） | L5 | R+I | Playwright 项目内串行 |
| 流式 SSE 契约 | 事件顺序、`done` 载荷 | `transport-http` `chat_stream_contract` | L6 | 轻量 | 并行 |
| 流式可观测性 | reasoning delta、trace、debug prompt_snapshot | `integration::streaming_chat`（6，含断线取消/重连） | L2 | M+I | **G-serial-integration** |
| 多轮记忆 / 指代 | `resolved_query` 写 PG；memory tool；notebook 跨 session 检索 | `smoke::memory_multiturn_smoke`（5） | L1 | M+I | **G-serial-rag** |
| 多轮（真实 LLM） | 第二轮引用第一轮上下文 | `llm_real::multi_turn` | L3 | R+I | **G-serial-llm** |
| 格式输出 HTML | 有效 HTML 结构 | `integration::format_output`；`llm_real::format_real` | L2/L3 | M / R | integration 串行 |

**缺口（待补）**：

- [x] **P1** `llm_real::chat_real`：专用「纯 general chat」真实 LLM 用例 ✅ 2026-06-13
- [x] **P2** Product E2E：SSE 客户端断线取消（`chat_stream_client_disconnect_aborts_without_hang`）✅ 2026-06-13
- [x] **P2** Product E2E：SSE 断线后会话重连（`chat_stream_disconnect_reconnect_continues_session`）✅ 2026-06-13

---

### 2.2 RAG 检索与文档问答

| 能力 | 验收标准 | 现有测试 | 层 | 依赖 | 并行组 |
|------|----------|----------|-----|------|--------|
| 上传→ingestion→completed | PG 有 summary/chunks | `smoke::ingestion_smoke` | L1 | M+I | **G-serial-rag** |
| Mock RAG 引文 | `citations` 含 `doc_id`；`[[cite]]` 入 answer | `smoke::rag_smoke` | L1 | M+I | **G-serial-rag** |
| RAG auto_fallback | `degrade_trace` / fallback 路径 | `smoke::rag_fallback_smoke` | L1 | M+I | **G-serial-rag** |
| Codegen 多工具链 | dense + doc_profile + chunk | `smoke::rag_codegen_multitool_smoke` | L1 | M+I | **G-serial-rag** |
| 多文档引文 | ≥2 `doc_id` | `integration::multi_doc` | L2 | M+I | **G-serial-integration** |
| 空文档降级 | chunk_count=0 + degrade | `integration::ingestion_full` | L2 | M+I | 串行 |
| 并发查询安全 | 双请求均 200 + citation | `integration::concurrent_query`（mock） | L2 | M+I | 串行 |
| 并发引文独立 | 两查询 citation chunk 集合不同 | `integration::concurrent_query::real_llm_*`（`#[ignore]`） | L3 | R+I | **G-serial-llm** |
| 跨租户隔离 | B 无法引用 A 的 doc | `tenants::isolation`（2） | L2 | M+I | 串行 |
| **真实 LLM RAG** | 真实 embedding 检索；引文非空；answer 实质内容 | `llm_real::rag_real::real_llm_rag_document_qa_returns_citation` | L3 | **R+I** | **G-serial-llm** |
| 真实 LLM 多工具 RAG | trace 含多种 retrieval tools | `llm_real::rag_real::real_llm_rag_complex_query_uses_multiple_tools` | L3 | **R+I** | 串行 |
| 真实 PDF 语料 RAG（txt 与 PDF 分列） | | | | | |
| — txt + 真实 LLM RAG | 真实 embedding 检索；引文非空 | `llm_real::rag_real`（`antifragile.txt`） | L3 | **R+I** | 串行 |
| — bundled PDF + 真实 LLM RAG | P4 `liteparse_hybrid` 路由 + 引文 | `llm_real::pdf_corpus`（`phase0-mini.pdf`，2+1 ignored） | L3 | **R+I** | 串行 |
| — PDF 一条龙（小 fixture） | ingest→RAG→cite | `llm_real::pdf_rag_e2e` | L3 | **R+I** | 串行 |
| — 本地大书 staging（手动） | 可选 `E2E_LLM_REAL_STAGING_PDF` | `pdf_corpus::real_llm_rag_staging_local_book_pdf` | staging | **R+I+P** | 手动 |
| 浏览器 RAG 黄金集 | upload→ready→citation>0 | `skills/rag-available.spec.ts`；`journey/workspace-upload-rag.spec.ts` | L4/L5 | **R+I** | Playwright |

**缺口（待补）**：

- [x] **P0** PNG 全链路进 PR smoke（`smoke::paddle_image_smoke` 升级为 ingest 全链路）✅ 2026-06-13
- [x] **P1** `llm_real`：`phase0-mini.pdf` ingest → nightly RAG 一条龙（`pdf_rag_e2e`）✅ 2026-06-13

---

### 2.3 Web Search（联网）

| 能力 | 验收标准 | 现有测试 | 层 | 依赖 | 并行组 |
|------|----------|----------|-----|------|--------|
| Mock Search 引文 | `source_type==web`；`[[n]]` 标记 | `smoke::search_smoke` | L1 | M+I | **G-parallel-smoke** |
| Search 429 降级 | 无 web citation；`degrade_trace` | `failure::provider_down` | L2 | M+I | 串行 |
| Search 超时 / 空结果降级 | 无 web citation；degrade 或降级文案 | `failure::search_degrade`（2） | L2 | M+I | 串行 |
| **真实 WebSearch（HTTP）** | Brave 真调用；web citation；无 degrade | `llm_real::search_real`；`smoke::search_real_smoke`（`#[ignore]` + `SEARCH_USE_REAL=1`） | L3/L1 staging | **R+Search** | **G-serial-llm** / 手动 |
| 浏览器 Search 黄金集 | mode=search；citation>0 | `skills/search-available.spec.ts` | L4 | **R+Search** | Playwright |
| 浏览器 Journey Search | PR 软 citation / nightly 硬 | `journey/workspace-chat.spec.ts`（web search） | L5 | **R+Search** | `E2E_TIER=nightly` 时硬门禁 |

**环境变量（真实 Search 必设）**：

```bash
SEARCH_PROVIDER=brave          # 或项目默认
SEARCH_API_KEY=<from .env>
SEARCH_REQUIRE_REAL=1          # llm_real 强制；Brave 不可达则失败
```

**缺口（待补）**：

- [x] **P1** Product E2E：`SEARCH_USE_REAL=1` smoke 变体（`smoke::search_real_smoke`，`#[ignore]`）✅ 2026-06-13
- [x] **P2** Search 超时 / 空结果降级专测（`failure::search_degrade`）✅ 2026-06-13

---

### 2.4 文档解析与入库（Ingestion）

| 能力 | 验收标准 | 现有测试 | 层 | 依赖 | 并行组 |
|------|----------|----------|-----|------|--------|
| TXT 上传解析 | completed；chunks>0 | `smoke::ingestion_smoke` | L1 | M+I | **G-serial-rag** |
| **LiteParse PDF（真实解析）** | `phase0-mini.pdf`→chunks>0；`liteparse_hybrid` | `integration::liteparse_pdf_e2e` | L2 | **I+P** | **G-serial-integration** |
| **docx Office（mock）** | mock office-parser → chunks | `integration::office_docx_e2e` | L2 | **M+I** | 串行 |
| **docx Office（真实 JVM）** | 真实 office-parser | `integration::office_docx_staging_e2e`（`#[ignore]`） | staging | **I+P** | staging 脚本 |
| **PNG Paddle 路由（mock Jobs）** | mock Paddle jobs→text/figure chunks | `smoke::paddle_image_smoke`（PR）；`integration::paddle_image_e2e`（路由元数据） | L2/L1 | **I+P** | **G-serial-rag** |
| **Black Swan PDF（真实 Paddle Jobs）** | 20 页 hybrid；`slow_ocr`/paddle | `smoke::paddle_pdf_smoke`（`#[ignore]`，manual-only） | staging | **I+P** | `./scripts/run-staging-ingest-e2e.sh` |
| **pptx Office（mock）** | mock office-parser → chunks | `integration::office_pptx_e2e` | L2 | **M+I** | 串行 |
| **pptx Office（真实 JVM）** | 真实 office-parser | `integration::office_pptx_staging_e2e`（`#[ignore]`） | staging | **I+P** | staging 脚本 |
| **xlsx Office（真实 JVM）** | 真实 office-parser | `integration::office_xlsx_staging_e2e`（`#[ignore]`） | staging | **I+P** | staging 脚本 |
| 损坏文件 | failed / 4xx | `integration::bad_file` | L2 | M+I | 串行 |
| Worker 超时 | failed，不挂死 | `failure::timeout` | L2 | M+I | 串行 |
| 重复上传幂等 | 相同 `document_id` | `integration::duplicate_upload` | L2 | M+I | 串行 |
| Ingestion 路由单测 | 扩展名→route | `ingestion` crate 单测 + worker metadata 契约 | L6 | 无 | PR smoke 预跑 |

**真实解析本地命令**：

```bash
# LiteParse PDF（integration，mock LLM）
cd avrag-rs
E2E_MODE=integration cargo test -p app --test product_e2e \
  integration::liteparse_pdf_e2e::phase0_mini_liteparse_pdf_ingest_e2e \
  --features product-e2e -- --test-threads=1 --nocapture

# Paddle PNG 全链路（integration）
E2E_MODE=integration cargo test -p app --test product_e2e \
  integration::paddle_image_e2e::paddle_ocr_image_routing_metadata_contract \
  --features product-e2e -- --test-threads=1 --nocapture

# Black Swan 大 PDF（需本地 PDF + 真实 Paddle Jobs）
./scripts/run-staging-ingest-e2e.sh
# 或单项：
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::paddle_pdf_smoke::black_swan_paddle_pdf_smoke \
  -- --ignored --test-threads=1 --nocapture
```

**缺口（待补）**：

- [x] **P0** Excel/Office 入库：`integration::office_xlsx_e2e` + mock office-parser ✅ 2026-06-13
- [x] **P1** `paddle_image_smoke` 进入 `RAG_SERIAL_MODULES` ✅ 2026-06-13
- [x] **P2** 文档删除 HTTP 黑盒：`integration::document_lifecycle` ✅ 2026-06-13
- [x] **P2** 文档重新处理（reindex）HTTP 黑盒：`document_lifecycle::reindex_completed_document_requeues_ingestion` ✅ 2026-06-13
- [x] **P0** `pdf_corpus` 对齐 P4 LiteParse（bundled PDF，去 MinerU/office 前置）✅ 2026-06-14
- [x] **P0** `paddle_pdf_smoke` 迁入 `SMOKE_MANUAL_ONLY_MODULES` ✅ 2026-06-14
- [x] **P1** docx office-parser：`integration::office_docx_e2e` + staging 脚本 ✅ 2026-06-15
- [x] **P1** pptx office-parser：`integration::office_pptx_e2e` + staging 脚本 ✅ 2026-06-15
- [x] **P1** Playwright PDF 上传：`journey/workspace-upload-pdf-rag.spec.ts` ✅ 2026-06-14

---

### 2.5 鉴权、分享、协作

| 能力 | 验收标准 | 现有测试 | 层 | 依赖 | 并行组 |
|------|----------|----------|-----|------|--------|
| 无 auth → 401 | chat / docs | `smoke::auth_boundary`（6，含 JWT 200） | L1 | M+I | **G-parallel-smoke**（模块内 `--test-threads=1`） |
| 跨 org 读写隔离 | 404/403 | `auth_boundary` + `tenants::isolation` | L1/L2 | M+I | 见上 |
| Share token 只读聊天 | 有效 token 200；无效 401 | `smoke::share_boundary`（4，含 invite HTTP） | L1 | M+I | **G-parallel-smoke** |
| 注册登录全流程 | JWT cookie / refresh | `smoke/auth-flow.spec.ts` | L5 | R | Playwright |
| 邀请协作 | invite accept/decline | `journey/invite-collaboration.spec.ts` | L5 | R | Playwright |
| Legal 同意 / 重签 | gate 阻断 | `smoke/legal-consent.spec.ts` + `transport-http` legal | L5/L6 | PG | Vitest + PW |
| 工作区提示词库 | 发送入库、点击插入、连点拼接、streaming 忽略 | `smoke/query-library.spec.ts` + Vitest `query-library-*` | L5 | R | Playwright functional |

**缺口（待补）**：

- [x] **P1** Product E2E：Bearer JWT 合法路径 chat 200 ✅ 2026-06-13
- [x] **P1** Product E2E：`invite_member` HTTP 黑盒 ✅ 2026-06-13
- [x] **P2** billing checkout `consent_required` HTTP 黑盒：`smoke::billing_boundary` ✅ 2026-06-13

---

### 2.6 计费、管理、桌面

| 能力 | 现有测试 | 层 | 缺口 |
|------|----------|-----|------|
| 定价页 / Paywall | `billing/*.spec.ts` | L5 | — |
| Checkout consent gate | `smoke::billing_boundary` | L1 | — |
| Usage 仪表 | `billing/usage-*.spec.ts` | L5 | — |
| Admin 导航 | `smoke/admin-navigation.spec.ts` | L5 | — |
| Workspace CRUD | `journey/notebook-crud.spec.ts` | L5 | Product E2E 仅 create |
| Tauri IPC | desktop 13 单元测 | L6 | 非 product E2E |

---

### 2.7 降级与韧性

| 场景 | 测试 | 层 |
|------|------|-----|
| Embedding 503 → lexical | `failure::embedding_down` | L2 |
| Search 429 | `failure::provider_down` | L2 |
| Worker 处理超时 | `failure::timeout` | L2 |
| Embedding 缓存命中 | `integration::embedding_cache` | L2（需 Redis） |

---

### 2.8 Agent API / MCP（外部代理接入）

外部代理通过 API Key + MCP `tools/call` 接入：建 key → 上传入库 → 提问带引文；越权一律 403/JSON-RPC error。

| 能力 | 验收标准 | 现有测试 | 层 | 依赖 | 并行组 |
|------|----------|----------|-----|------|--------|
| MCP 全流程（key→上传→入库→提问） | `citations` 非空、`doc_id` 匹配、answer 有实质内容 | `integration::mcp_agent_flow` | L2 | M+I | **G-serial-integration** |
| MCP 越权：workspace key 调 org 工具 | 403 `workspace_key_cannot_call_org_tools` | `integration::mcp_auth_boundary` | L2 | M+I | 串行 |
| MCP 越权：org key 调 workspace 工具 | 403 `org_key_cannot_call_workspace_tools` | `integration::mcp_auth_boundary` | L2 | M+I | 串行 |
| MCP 越权：跨 notebook 查询 | 403 `notebook_scope_mismatch` | `integration::mcp_auth_boundary` | L2 | M+I | 串行 |
| API key 访问用户态资源（notes 等） | 403 `api_key_forbidden` | `integration::mcp_auth_boundary` | L2 | M+I | 串行 |
| API key scope/权限边界（契约） | workspace/org key 各自权限剥离 | `transport-http` `api_key_security_contract`（16） | L6 | 轻量/PG | 并行 |
| MCP tools/call 信封 + ingestion（契约） | create_upload→complete→status；error 信封 | `transport-http` `mcp_unified_contract`（10） | L6 | 轻量/PG | 并行 |

**缺口（待补）**：

- [x] **P0** Product E2E：MCP agent 全流程（`mcp_agent_flow`）✅ 2026-06-29
- [x] **P0** Product E2E：MCP/API key 越权边界（`mcp_auth_boundary`，4 用例）✅ 2026-06-29
- [ ] **P1** `CAP-AGENT` 能力标签：registry 给 `mcp_*` 测试补 `capabilities: [CAP-AUTH, CAP-INGEST]`（PR-2 后续）

---

### 2.9 OpenAI 兼容 API / 限流 / 配额（PR-3）

OpenAI 兼容路由 `POST /v1/workspaces/{workspace_id}/chat/completions` 复用标准 chat 鉴权（API Key Bearer + `query` 权限 + notebook scope）；限流按 API key 的 `rate_limit_rpm` 每 key 计数，配额按用户滚动 5h/7d 用量计。

| 能力 | 验收标准 | 现有测试 | 层 | 依赖 | 并行组 |
|------|----------|----------|-----|------|--------|
| OpenAI 路由：无 auth | 401 | `transport-http` `openai_completions_contract` | L6 | 轻量 | 并行 |
| OpenAI 路由：workspace key + `stream:false` | 200 + 非空 `answer` | `transport-http` `openai_completions_contract` | L6 | 轻量 | 并行 |
| OpenAI 路由：`stream:true` | 200 + `text/event-stream` + `data:` 帧 | `transport-http` `openai_completions_contract` | L6 | 轻量 | 并行 |
| OpenAI 路由：notebook scope 不匹配 | 403 | `transport-http` `openai_completions_contract` | L6 | 轻量 | 并行 |
| 每 key 限流 `rate_limit_rpm:2` | 第 3 次 429 + `Retry-After` + `x-ratelimit-limit:2` + `rate_limit_exceeded` | `integration::rate_limit_boundary` | L2 | M+I | **G-serial-integration** |
| 配额耗尽 | chat 429 + `quota_exceeded`（区别于 `rate_limit_exceeded`） | `integration::quota_boundary` | L2 | M+I | **G-serial-integration** |

**注意**：`E2E_ENABLED=true`（product_e2e bootstrap 默认）会把每 key 限流覆盖为 1000 RPM、edge 限流放宽到 10000；`rate_limit_boundary` 在测内临时关掉 `E2E_ENABLED`（Drop guard 还原）并用唯一 `x-forwarded-for` 隔离 edge 计数，以强制 2 RPM 每 key 限流。**必须** `--test-threads=1`（`G-serial-integration`）运行，避免 env toggle 与并发测试 race。

**缺口（待补）**：

- [x] **P1** OpenAI 兼容 API 契约（`openai_completions_contract`，4 用例）✅ 2026-06-29
- [x] **P1** 限流 429 Product E2E（`rate_limit_boundary`）✅ 2026-06-29
- [x] **P1** 配额 429 Product E2E（`quota_boundary`）✅ 2026-06-29

---

## 3. 真实依赖四件套（Agent 必跑清单）

发布前或改动 Agent/RAG/Search/Ingestion 后，**至少**执行下列四项（可并行其中独立项）：

### 3.1 真实文档解析

| 项 | 命令 | 通过标准 |
|----|------|----------|
| LiteParse PDF | §2.4 `liteparse_pdf_e2e` | `DocumentStatus::Completed`，`chunk_units > 0`，summary 含 liteparse |
| Paddle PNG | §2.4 `paddle_image_e2e` | completed；OCR 文本或 figure chunk；`paddle_jobs_count=1` |

### 3.2 真实 LLM RAG 检索

```bash
cd avrag-rs
E2E_MODE=nightly cargo test -p app --test product_e2e \
  llm_real::rag_real::real_llm_rag_document_qa_returns_citation \
  --features product-e2e -- --ignored --test-threads=1 --nocapture
```

**通过标准**：`assert_has_citations`；`assert_citation_doc_id`；`assert_citation_referenced_in_answer`；`answer` 长度 > 阈值；产物在 `e2e_output/llm_real/`。

**进阶**（可选）：

```bash
# 多工具 + PDF 语料
E2E_MODE=nightly cargo test -p app --test product_e2e llm_real::rag_real -- --ignored --test-threads=1
E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e \
  llm_real::pdf_corpus -- --ignored --test-threads=1
```

### 3.3 真实 Chat

```bash
# 浏览器 general chat + 历史
cd frontend_next
pnpm exec playwright test e2e/specs/journey/workspace-chat.spec.ts -g "general mode"

# Rust 侧（多轮真实 LLM）
cd avrag-rs
E2E_MODE=nightly cargo test -p app --test product_e2e \
  llm_real::multi_turn --features product-e2e -- --ignored --test-threads=1 --nocapture
```

### 3.4 真实 WebSearch

```bash
# HTTP 黑盒
cd avrag-rs
SEARCH_REQUIRE_REAL=1 E2E_MODE=nightly cargo test -p app --test product_e2e \
  llm_real::search_real --features product-e2e -- --ignored --test-threads=1 --nocapture

# 浏览器黄金集（硬 citation）
cd frontend_next
pnpm exec playwright test --project=skills e2e/specs/skills/search-available.spec.ts
```

---

## 4. 并行执行编排

### 4.1 PR Smoke（`run-product-smoke-e2e.sh`）

```
预构建 cargo build（避免锁争用）
    ↓
契约单测（ingestion 路由 + worker metadata）  ─┐
并行 batch A（各起独立 cargo test 进程）：      │
  NON_RAG_MODULES × 5（含 billing_boundary）       ├─ wait
  UNIT_TEST_FILTERS × 4                        │
    ↓                                          ─┘
串行 batch B（共享 Milvus collection 状态）：
  RAG_SERIAL_MODULES × 7（含 paddle_image_smoke 全链路；1 个 #[ignore] paddle_pdf）
```

**注册表单一真相源**：`avrag-rs/scripts/run-product-smoke-e2e.sh` 中 `NON_RAG_MODULES` + `RAG_SERIAL_MODULES`。新增 `smoke::foo` 模块必须同步修改脚本，否则 `--check-modules` 失败。

```bash
./scripts/run-product-smoke-e2e.sh --check-modules   # 仅守卫，不跑测
./scripts/run-product-smoke-e2e.sh                   # 全量 PR smoke
```

### 4.2 Integration 全量

**必须** `--test-threads=1`（`shared_rag_fixture`、`streaming_chat`、Milvus 前缀共享）。

```bash
cd avrag-rs
./scripts/e2e-precheck.sh   # 仓库根；检查 Milvus
E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  -- --test-threads=1 --nocapture
```

**可并行子集**（本地调试时多终端，**不要**与全量 integration 同时跑）：

| 终端 | 过滤器 | 说明 |
|------|--------|------|
| T1 | `integration::liteparse_pdf_e2e` | 真实 PDF |
| T2 | `integration::paddle_image_e2e` | 真实 PNG |
| T3 | `failure::` | 降级三件套 |
| T4 | `tenants::` | 隔离 |

### 4.3 Nightly Real LLM

**必须** `--ignored --test-threads=1`（API 成本 + 非确定性）。

```bash
E2E_MODE=nightly cargo test -p app --test product_e2e llm_real \
  --features product-e2e -- --ignored --test-threads=1 --nocapture
```

### 4.4 Playwright 多项目

```bash
cd frontend_next
# PR 常用
pnpm exec playwright test --project=auth --project=functional --project=journey

# 真实 RAG+Search 黄金集
pnpm exec playwright test --project=skills

# 一站式（Goal D）
./scripts/e2e-d-gate.sh   # 仓库根；含 mock×2 + llm_real + Playwright
```

### 4.5 Agent 推荐「改动后最小验证」并行组

同时开 3 个终端（无共享状态冲突）：

```bash
# 终端 1 — PR smoke
cd avrag-rs && ./scripts/run-product-smoke-e2e.sh

# 终端 2 — 前端单元
cd frontend_next && pnpm test

# 终端 3 — transport 契约
cd avrag-rs && cargo test -p transport-http
```

若改动 ingestion/解析，再加终端 4：

```bash
E2E_MODE=integration cargo test -p app --test product_e2e \
  'integration::(liteparse_pdf_e2e|paddle_image_e2e)' \
  --features product-e2e -- --test-threads=1
```

---

## 5. 测试清单（Agent 勾选表）

### 5.1 PR 必过（Mock + 基础设施）

- [ ] `./scripts/run-product-smoke-e2e.sh` 全绿
- [ ] `cargo test -p transport-http`
- [ ] `frontend_next`: `pnpm test`（Vitest）
- [ ] 若动 frontend：`pnpm exec playwright test --project=auth` 或相关 spec

### 5.2 合并到 master 后（Integration）

- [ ] `E2E_MODE=integration` 全套件 0 fail（约 59 runnable + ignored）
- [ ] `integration::liteparse_pdf_e2e` — **真实 PDF 解析**
- [ ] `integration::paddle_image_e2e` — **真实 PNG 解析**

### 5.3 Nightly / 发布前（真实 LLM + Search）

- [ ] `llm_real::rag_real` — **真实 LLM RAG**
- [ ] `llm_real::search_real` + `SEARCH_REQUIRE_REAL=1` — **真实 WebSearch**
- [ ] `llm_real::multi_turn` — **真实多轮 Chat**
- [ ] Playwright `--project=skills` — 浏览器 **真实 RAG + Search** citation 硬门禁
- [ ] `llm_real::pdf_corpus`（bundled `phase0-mini.pdf` + 可选 staging 大书）
- [ ] Playwright `workspace-upload-pdf-rag.spec.ts`（PDF 上传旅程）
- [ ] 可选：`RUN_QUALITY_JUDGE=1` Playwright judge（分数 &lt;6 仅警告）

### 5.4 已知 `[#ignore]` 手动项

| 测试 | 条件 |
|------|------|
| `smoke::paddle_pdf_smoke` | staging：Black Swan PDF + 真实 Paddle（`SMOKE_MANUAL_ONLY`） |
| `llm_real::*` | `.env` 中 AGENT/MEMORY/INGESTION/EMBEDDING/SEARCH 凭证 |
| `integration::concurrent_query::real_llm_*` | nightly + 真实 LLM |
| `smoke::backend_launcher` | 开发用，非 CI |

---

## 6. 单元与契约测（并行，无 Docker）

| 包 / 路径 | 用途 | 命令 |
|-----------|------|------|
| `product_e2e` 单元 | gate、setup、mock_routing | PR smoke 脚本已包含 |
| `transport-http` | SSE、legal、auth 契约 | `cargo test -p transport-http` |
| `app` agent 单测 | exit_policy、query_normalize、answer_contract | `cargo test -p app --lib` |
| `unified_agent_contract` | ADR-0008 cite 契约 | `cargo test -p app --test unified_agent_contract` |
| `avrag-share` | invite / public-read 契约 | `cargo test -p avrag-share` |
| `desktop` | Tauri IPC | `cargo test --manifest-path desktop/src-tauri/Cargo.toml` |
| `frontend_next` Vitest | 组件/transport 纯函数 | `pnpm test` |

---

## 7. 发布门禁清单（全功能）

按顺序执行；**阶段 3 与 4 可并行**：

0. **DB migrations**（记忆检索 / L2 清理）：按序执行 `0043_chat_message_fts` → `0044_drop_chat_session_summary` → `0045_assistant_message_search_tokens`；`PgAppRepository::migrate()` 会自动 jieba 重分段 `search_tokens`（可用 `AVRAG_SKIP_SEARCH_TOKEN_RESEGMENT=1` 跳过）
1. **预检**：`./scripts/e2e-precheck.sh`
2. **L1 PR Smoke**：`./avrag-rs/scripts/run-product-smoke-e2e.sh`
3. **L2 Integration 全量**：§4.2
4. **L3 Nightly**：§3.2 + §3.4 命令
5. **L4 Skills**：`pnpm exec playwright test --project=skills`
6. **L5 Journey + Billing smoke**：`pnpm exec playwright test --project=journey --project=auth`
7. **产物审计**：`cargo run -p e2e-analyzer -- llm-real summary --run <run_dir>`

---

## 8. 补测 Backlog（按优先级）

> **准上线补测实施（2026-06-26）**：分阶段任务、PR 拆分、验收命令见 [`plans/2026-06-26-e2e-launch-readiness-plan.md`](plans/2026-06-26-e2e-launch-readiness-plan.md)（**仅测试/CI，不改主程序**）。

> **PR 进度（准上线补测）**：
> - [x] **PR-1**：Milvus CI（`scripts/ci-start-milvus.sh` + 4 个 workflow）+ 契约测入库（`api_key_security_contract`、`mcp_unified_contract`）+ `e2e-test-registry.yaml` 同步 — ✅ 2026-06-29
> - [x] **PR-2**：MCP / API Key Product E2E（`integration::mcp_agent_flow` 全流程带引文 + `integration::mcp_auth_boundary` 4 个越权用例）+ 指南 §2.8 Agent API/MCP 行 + registry 同步 — ✅ 2026-06-29
> - [x] **PR-3**：OpenAI API + 限流/配额 E2E（`transport-http` `openai_completions_contract` 4 用例：401/200+body/SSE/403 + `integration::rate_limit_boundary` 每 key 2 RPM 第 3 次 429/`Retry-After`/`x-ratelimit-limit:2` + `integration::quota_boundary` 配额耗尽 chat 429/`quota_exceeded`）+ 指南 §2.9 + registry 同步（102 tests） — ✅ 2026-06-29
> - [x] **PR-4**：Billing master 自动门禁（`frontend-journey.yml` 加并行 `billing-e2e` job：master push / `workflow_dispatch` 自动跑 `e2e/specs/billing/paywall-flow.spec.ts` + `usage-dashboard.spec.ts`（`--project=billing`，env `PRICING_REVAMP_ROLLOUT=100` / `NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1` / `E2E_RESET_SECRET`，timeout 30min，失败上传 `playwright-billing-report` 阻断，与 journey 同级；billing 无 RAG 路径 → 不需 Milvus，`DATABASE_URL` 未设 → avrag-api in-memory 启动）+ `e2e-gates.md` Layer overview 加 billing 门禁行） — ✅ 2026-06-29
> - [x] **PR-5**：Playwright api-access + citation —— §7.1 `smoke/api-access.spec.ts`（创建 key→明文仅显一次→列表见 prefix/RPM/生效中→撤销回空态）本地 1 passed 4.7s ✅ 2026-06-29；§7.2 `journey/citation-interaction.spec.ts`（点 `workspace-citation`→"引用片段" dialog→👍 反馈 POST 200 + 按钮 disabled/👍）本地 1 passed 46.5s ✅ 2026-06-29（更新 `avrag-rs/.env` 的 `EMBEDDING_API_KEY` 为有效 DashScope key、`INGESTION_LLM_MODEL` 校正为 `gemini-3.1-flash-lite-preview` 后入库完成）；e2e-gates.md 已加 Functional/Journey 行；journey-e2e CI 注入 3 secret（`DASHSCOPE_API_KEY`/`DMX_API_KEY`/`DEEPSEEK_API_KEY`）+ `AGENT_LLM_MODEL` 覆盖为 `deepseek-v4-flash`；**CI secret 注入机制本地模拟验证通过**（.env 挪开 + process env 传 3 key 同 YAML 注入方式，§7.2 1.4m passed ✅ 2026-06-29）；真实 GitHub journey CI 待推 master（origin/master 落后本地 207 提交且 `4cb8f67` 移除 CI，journey workflow 不在默认分支 → Actions "找不到" + `workflow_dispatch` 不可用）
> - [x] **PR-6**：`rag_quality` + release gate —— `ProductionRagEvaluator`（`crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs`，llm_real tier 接真实 RagRuntime，复用 `shared_rag_fixture` 冷入库 `antifragile.txt`）定基线 Recall@15=80%/Citation=80%/Hallucination=80%（Q1 "谁提出 antifragility" 检索缺口→refusal 0%，Q2-Q5 单 rich chunk 100%）；非流式 answer 载 `[[cite:CHUNK_ID]]`（UUID），evaluator 用 `chat.citations` 建 `chunk_to_cite` map 改写为 `[citation:N]` 再打分；`release-e2e-gate.yml`（`workflow_dispatch`/`release.published`，写 `.env` 注入 3 RAG secret `DASHSCOPE_API_KEY`/`DMX_API_KEY`/`DEEPSEEK_API_KEY`）；**硬门禁 Recall drop ≤3% from baseline 0.80**，Citation ≥95% + Hallucination ≤2% 上报不阻断（Q1 refusal 拖累 citation 至 80% 达不到 ≥95%，hallucination 词重叠启发式见 GOTCHAS.md）；p8 双向验证：基线 80% 绿 + wrong doc_scope→Recall 0%→FAILED 红 ✅ 2026-06-29；e2e-gates.md 已加 Release gate 行 + 章节

Agent 实现新测试时，请同步更新本节与 `run-product-smoke-e2e.sh`（若属 smoke 模块）。

| 优先级 | 能力 | 建议落点 | 依赖 | 状态 |
|--------|------|----------|------|------|
| **P0** | `integration-e2e.yml` 触发 `master` | `.github/workflows/integration-e2e.yml` | CI | ✅ 2026-06-13 |
| **P0** | PNG 全链路进 PR smoke | `smoke::paddle_image_smoke`（全链路 ingest） | I+P | ✅ 2026-06-13 |
| **P0** | Excel 入库 integration | `integration::office_xlsx_e2e` | mock office-parser | ✅ 2026-06-13 |
| **P1** | 真实 general chat llm_real | `llm_real::chat_real.rs` | R | ✅ 2026-06-13 |
| **P1** | PDF ingest → nightly RAG 一条龙 | `llm_real::pdf_rag_e2e.rs` | R+I+P | ✅ 2026-06-13 |
| **P1** | JWT auth chat 200 | `smoke::auth_boundary` | M+I | ✅ 2026-06-13 |
| **P1** | Share invite HTTP | `smoke::share_boundary` | M+I | ✅ 2026-06-13 |
| **P2** | billing consent HTTP | `smoke::billing_boundary` | M+I+PG seed | ✅ 2026-06-13 |
| **P2** | 文档删除 | `integration::document_lifecycle` | M+I | ✅ 2026-06-13 |
| **P2** | Stream cancel | `integration::streaming_chat` | M+I | ✅ 2026-06-13 |
| **P2** | SSE 断线重连（同 session_id） | `integration::streaming_chat` | M+I | ✅ 2026-06-13 |
| **P2** | Search 超时/空结果降级 | `failure::search_degrade` | M+I | ✅ 2026-06-13 |
| **P1** | `SEARCH_USE_REAL=1` smoke | `smoke::search_real_smoke`（`#[ignore]`） | R+Search | ✅ 2026-06-13 |
| **P2** | 文档 reindex HTTP | `integration::document_lifecycle` | M+I | ✅ 2026-06-13 |

---

## 9. 环境变量速查

| 变量 | Smoke | Integration | Nightly | Playwright Skills |
|------|-------|-------------|---------|-------------------|
| `E2E_MODE` | `smoke` | `integration` | `nightly` | — |
| `AGENT_LLM_*` | mock | mock | **真实** | 经后端 .env |
| `EMBEDDING_*` | mock | mock | **真实** | 经后端 .env |
| `SEARCH_API_KEY` | mock | mock | **真实** | 经后端 .env |
| `SEARCH_REQUIRE_REAL` | — | — | `1` | — |
| `SEARCH_USE_REAL` | `1`（仅 `search_real_smoke`，`#[ignore]`） | — | — | — |
| `SEARCH_TIMEOUT_MS` | — | mock 可注入（`failure::search_degrade`） | 经 `.env` | — |
| `RUN_QUALITY_JUDGE` | — | — | 可选 `1` | 可选 `1` |
| `E2E_TIER` | — | — | — | `nightly` 时 journey search 硬 citation |
| `E2E_LLM_REAL_BLACK_SWAN_PDF` | — | — | 大 PDF 路径 | — |

凭证来源：`avrag-rs/.env`（`llm_real` 会自动 load，不覆盖已 export 的值）。

---

## 10. 文档维护规则

1. 新增 `product_e2e::smoke::*` 模块 → 更新 `run-product-smoke-e2e.sh` + 本文档 §4.1。
2. 新增 `llm_real` 或 `integration` 用例 → 更新 §2 矩阵与 §5 勾选表。
3. 新增 Playwright spec → 更新 §2 对应能力行与 `e2e-gates.md` Journey 表。
4. CI workflow 变更 → 更新 §1 表格。
5. 每轮 Brooks 测试审查后，将 §8 backlog 已闭环项标为 ✅ 并注明日期。
6. 测试枚举变更后运行 `./scripts/generate-e2e-test-registry.py` 同步 `e2e-test-registry.yaml`。

---

## 附录 A：Product E2E 测试枚举（86）

完整列表：

```bash
cd avrag-rs
cargo test --test product_e2e -p app --features product-e2e -- --list
```

模块计数（2026-06-13）：

| 模块 | 测试数 | 默认 CI |
|------|--------|---------|
| `smoke::` | 25（含 3 ignored） | L1 |
| `integration::` | 20（含 1 ignored real_llm concurrent） | L2 |
| `failure::` | 5 | L2 |
| `tenants::` | 2 | L2 |
| `llm_real::` | 16（含 7 非 ignore 单测 + 9 ignore E2E） | L3 |
| 基础设施单测 | 18 | L1/L6 |

## 附录 B：Playwright Spec 索引

| 目录 | Spec | 真实 LLM/Search | Citation 门禁 |
|------|------|-----------------|---------------|
| `smoke/` | auth-flow, auth-failure, legal-consent, admin-navigation, query-library | 栈依赖 | 部分 |
| `journey/` | workspace-chat, workspace-upload-rag, notebook-crud, invite, share, session, analyze | upload-rag / search 是 | upload-rag 硬；search 分层 |
| `skills/` | rag-available, search-available, format-output, analyze, notebook | **是** | **硬** |
| `billing/` | pricing, paywall, usage, visual | — | — |
