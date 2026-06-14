# Product E2E Test Plan (Executable Version)

> **2026-06-13**：全功能覆盖、并行编排、真实依赖四件套与 Agent 勾选表已迁移至 [`full-functional-e2e-guide.md`](full-functional-e2e-guide.md)。本文档保留分层设计与 P0–P14 历史矩阵。

## 1. 现状与问题

### 1.1 当前测试的真实定位

`crates/app/tests/product_e2e/` 是 **HTTP 黑盒产品 E2E**，入口为 `POST /api/v1/chat`，被测系统为 **Unified Agent + ReActLoop**（检索 → 评估 → 合成，Messenger / LoopOptimizer 模型）：

| 维度 | Product E2E（本套件） | 旧 Strategy/Commander 路径（已废弃） |
|------|----------------------|--------------------------------------|
| 入口 | HTTP `POST /api/v1/chat` | 直接 `RagContext` + `StrategyExecutor` |
| RAG 检索 | Mock LLM 返回 `<code>` → `code_execution_result` 证据链；独立用例测 `auto_fallback` | planner/evaluator 独立 LLM 调用 + `dense_retrieval` native tool |
| Ingestion | HTTP upload → worker → PG + Milvus | 手动 fixture 注入 |
| 断言 | 协议层 + 结构化 `citations` / `DegradeReason` | 字符串 / 状态机 |

### 1.2 历史运行结论（2026-06 对齐后）

- Smoke mock 已对齐 ReActLoop：RAG happy path 走 **codegen 主路径**；`rag_fallback_smoke` 单独覆盖 **auto_fallback** 安全网
- 已删除 mock 中永不命中的 planner/evaluator 路由（Commander 残留）
- `degrade_trace.reason` 已强类型化为 `DegradeReason` 枚举（JSON 仍为 snake_case 字符串）
- 最不稳定因素仍是 infra（PG/Milvus/worker）与 LLM provider 抖动（Nightly 层）

---

## 2. 测试分层矩阵（新增 — 核心设计）

| 测试层 | 触发时机 | 外部依赖 | 目标 | 时长预算 | 通过门槛 |
|--------|---------|---------|------|---------|---------|
| **Smoke E2E** | 每个 PR | Mock LLM、Mock Search、Mock Embedding、**真实 PG + 真实 Milvus + 真实本地 Object Store** | 验证 HTTP/鉴权/上传/worker 编排/响应结构 | ≤ 5 min | 通过率 > 95%，重跑 2 次结果一致 |
| **Product Integration** | 主干合并后 | 真 PG + 真对象存储 + 真向量库，LLM 可半 Mock | 验证 ingestion 与检索主链路 | ≤ 10 min | 零 P0 用例失败 |
| **Nightly E2E** | 每晚 | 真实 LLM + 真实 Search | 验证真实回答质量与外部集成 | ≤ 30 min | 失败自动归因（provider/infra/product） |
| **Release Gate** | 发布前 | 受控真实依赖 | 少量高价值用例门禁 | ≤ 15 min | 100% P0 通过 |

**强约束**：
- Smoke E2E 不允许调用真实 LLM/Search/Embedding；但基础设施（PG/Milvus/Object Store）必须真实
- 只有 Nightly 和 Release Gate 允许真实 LLM/Search
- 所有层共享同一套 **场景 DSL 和 helper**；同一用例在不同层允许不同断言集，避免 `if smoke / if integration` 分支污染用例代码

**层级职责硬规则**：
- Smoke 只验证"**平台编排和最小业务闭环**"：上传能成功、ingestion 能完成、查询能返回结构化响应。用例数量控制在 3-5 个，不覆盖降级/多文档/格式输出。
- Integration 验证"**完整链路 + 产品规则**"：覆盖所有 P1 用例（格式输出、多文档）和部分 P2 用例（空文档、损坏文件、并发查询）。
- 两层在基础设施层面接近（都使用真实 PG/Milvus/Object Store），差异在于**断言深度和用例范围**。

---

## 3. 可量化验收标准

### 3.1 P0 用例（PR 级 Smoke）

| # | 场景 | 验收标准 | 判定方式 | 唯一失败归因 |
|---|------|---------|---------|-------------|
| 1 | 用户上传文档，worker 完成 ingestion | HTTP 200 → 轮询状态为 `completed` → PG 中存在 summary + TOC + chunks | 结构化 API 响应字段 + PG 查询 | infra: PG/Milvus 不可用；product: worker 未启动/崩溃；test: fixture 路径错误 |
| 2 | 用户问文档相关问题，系统返回 RAG 回答 | HTTP 200 → 响应 `citations` 数组非空 → `citations[].doc_id` 匹配上传文档 | 响应 JSON 字段断言 | infra: Milvus 空；product: evaluator 死循环/策略路由错误；test: doc_id 传递错误 |
| 3 | 用户问开放问题，系统返回 Search 回答 | HTTP 200 → 响应 `citations` 数组非空 → `citations[].source_type == "web"` | 响应 JSON 字段断言 | infra: Search mock 未注入；product: 策略路由到 Chat 而非 Search；test: query 不含搜索触发词 |

### 3.2 P1 用例（主干合并后）

| # | 场景 | 验收标准 | 唯一失败归因 |
|---|------|---------|-------------|
| 4 | Chat + presentation-html | 响应含 `format_output` → `format_output.type == "presentation-html"` → 内容可解析为有效 HTML | product: format skill 未触发；test: HTML 解析器变更 |
| 5 | Chat + html-renderer | 同上，格式为 `html-renderer` | 同上 |
| 6 | 多文档 RAG | `citations` 包含 ≥2 个不同 `doc_id` | product: 检索仅命中单文档；test: doc_scope 只传了一个 |
| 7 | 空文档上传 | HTTP 200 → 状态 `completed` → PG 中 chunk_count == 0 → 回答含 `degrade_trace` 或降级标识字段 | product: 空文档未正常完成解析；test: fixture 非空 |

### 3.3 失败场景（P2 — 主干合并后）

| # | 场景 | 验收标准 | 唯一失败归因 |
|---|------|---------|-------------|
| 8 | 损坏文件上传 | HTTP 4xx/5xx 或状态 `failed` → 响应/日志含可读错误码 | product: parser panic 未捕获；test: fixture 不够损坏 |
| 9 | Worker 处理中超时 | 状态最终为 `failed` 或 `timeout`，不无限挂起 | infra: worker 未运行；product: 超时阈值配置错误 |
| 10 | Search provider 429 | 降级为内部知识或返回降级文案；HTTP 200；`degrade_trace` 非空 | product: 降级路径未实现；test: mock 行为注入失败 |
| 11 | Embedding 服务不可用 | RAG 降级为 lexical_retrieval 或返回降级文案；`degrade_trace` 含 `embedding_unavailable` | product: 降级路径未实现 |
| 12 | 重复上传同文件 | 第二次上传返回相同 `document_id`；PG 中该文件只存在一条记录 | product: 幂等逻辑缺失；test: 未等待首次 ingestion 完成 |
| 13 | 并发查询同一文档 | 两查询各自 HTTP 200；`citations` 独立且正确；无交叉污染 | infra: 连接池耗尽；product: session 状态共享 bug |
| 14 | 多租户文档隔离 | 用户 A 上传文档 → 用户 B 查询相同内容 → 用户 B 的 citation 不含用户 A 的 `doc_id` | product: 鉴权/scope 过滤失效；test: auth context 构造错误 |

---

## 4. 断言分层（替代"语义相似度"等脆弱方案）

### 4.1 协议层（所有测试必须）

```rust
// 不依赖 LLM 输出内容，只验证 API 契约
fn assert_http_ok(resp: &Response) { assert_eq!(resp.status, 200); }
fn assert_schema_valid(resp: &Value, schema: &JSONSchema) { schema.validate(resp).unwrap(); }
fn assert_has_citations(resp: &ChatResponse) {
    assert!(resp.citations.is_some() && !resp.citations.unwrap().is_empty());
}
fn assert_citation_doc_id(resp: &ChatResponse, expected_doc_id: &str) {
    let ids: Vec<_> = resp.citations.iter().map(|c| c.doc_id.as_str()).collect();
    assert!(ids.contains(&expected_doc_id), "expected citation from {}", expected_doc_id);
}
```

### 4.2 产品层（主干合并后启用）

```rust
// 验证业务规则，不依赖 LLM 措辞
fn assert_answer_has_doc_citation(resp: &ChatResponse) {
    let has_doc = resp.citations.iter().any(|c| c.source_type == "document");
    assert!(has_doc, "expected at least one document citation");
}
fn assert_answer_has_web_citation(resp: &ChatResponse) {
    let has_web = resp.citations.iter().any(|c| c.source_type == "web");
    assert!(has_web, "expected at least one web citation");
}
fn assert_html_structure_valid(html: &str) {
    let doc = scraper::Html::parse_document(html);
    assert!(doc.select(&Selector::parse("html").unwrap()).next().is_some());
    assert!(doc.select(&Selector::parse("body").unwrap()).next().is_some());
}
```

### 4.3 质量层（仅 Nightly/Release Gate，离线评估）

```rust
// LLM-as-judge 或规则引擎，不阻塞 PR
fn quality_score_answer(answer: &str, query: &str, context: &str) -> f32 {
    // 调用离线评估模型或规则集
    // 返回 0-1 分数，用于趋势分析，不用于 pass/fail
}
```

**关键规则**：PR 级测试只用**协议层 + 产品层**断言；质量层只用于 nightly 报告。

### 4.4 `degrade_trace` Schema（降级可观测性）

```rust
#[derive(Debug, serde::Deserialize)]
struct DegradeTraceItem {
    pub stage: String,           // 触发组件/阶段，如 "dense_retrieval" / "degraded_no_evidence"
    pub reason: DegradeReason,   // 强枚举，JSON 序列化为 snake_case 字符串
    pub impact: String,          // 对用户/链路的影响描述
}
```

**使用约定**：
- 任何降级路径必须在 `degrade_trace` 中留下至少一条记录
- 测试断言通过 `DegradeReason` 枚举匹配（如 `EmbeddingUnavailable`、`Search429`），**禁止**子串匹配
- 新增降级原因须同步扩展 `contracts::chat::DegradeReason`

---

## 5. 技术架构

### 5.1 目录结构

```
crates/app/tests/
├── product_e2e/                    # 新增：产品 E2E
│   ├── mod.rs                      # TestContext, shared helpers
│   ├── setup.rs                    # Testcontainers 编排
│   ├── fixtures/
│   │   ├── antifragile.txt
│   │   ├── empty.txt
│   │   └── corrupted.pdf
│   ├── assertions.rs               # 协议层 + 产品层断言
│   ├── smoke/                      # PR 级（Mock 依赖）
│   │   ├── ingestion_smoke.rs      # P0-1: 上传并完成 ingestion
│   │   ├── rag_smoke.rs            # P0-2: 文档问答
│   │   └── search_smoke.rs         # P0-3: 搜索问答
│   ├── integration/                # 主干合并后（真基础设施）
│   │   ├── ingestion_full.rs       # P1: 完整 ingestion 链路
│   │   ├── format_output.rs        # P1: HTML/PPT 格式
│   │   └── multi_doc.rs            # P1: 多文档 RAG
│   ├── failure/                    # P2: 降级与边界
│   │   ├── bad_file.rs
│   │   ├── timeout.rs
│   │   └── provider_down.rs
│   └── tenants/                    # 多租户隔离
│       └── isolation.rs
├── e2e/                            # 现有：保留，后续重命名
│   ├── config.rs
│   ├── assertions.rs
│   └── ...                         # Agent 策略集成测试
└── e2e_output/                     # 运行结果目录
    ├── failures/{run_id}/{test_name}/
    ├── llm_real/{run_id}/{test_name}/
    └── observability/{run_id}/{test_name}/
```

### 5.2 TestContext 设计

```rust
// tests/product_e2e/mod.rs
pub struct TestContext {
    pub app_state: AppState,
    pub http_client: reqwest::Client,
    pub base_url: String,
    pub pg: TestcontainerHandle,       // PostgreSQL
    pub milvus: TestcontainerHandle,   // Milvus
    pub object_store: TempDir,         // 本地文件系统对象存储
    pub worker: WorkerHandle,          // 内嵌 worker 进程
    pub mocks: MockRegistry,           // Mock LLM/Search/Embedding
}

impl TestContext {
    /// 创建 Smoke 上下文（Mock 依赖）
    pub async fn new_smoke() -> Self { ... }

    /// 创建 Integration 上下文（真实基础设施）
    pub async fn new_integration() -> Self { ... }

    /// HTTP API 辅助方法
    pub async fn upload_document(&self, fixture: &str
    ) -> Result<UploadResponse> { ... }

    pub async fn chat(&self, query: &str, doc_scope: &[Str]
    ) -> Result<ChatResponse> { ... }

    pub async fn wait_for_ingestion(&self, doc_id: &str, timeout: Duration
    ) -> Result<DocumentStatus> { ... }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // 自动清理 containers、临时目录、worker 进程
    }
}
```

### 5.3 Mock 设计（Smoke 层专用）

```rust
// MockRegistry 控制所有外部依赖；基础设施（PG/Milvus/Object Store）始终真实
pub struct MockRegistry {
    pub llm: MockLlmProvider,        // 固定响应，无网络
    pub search: MockSearchProvider,  // 固定搜索结果
    pub embedding: MockEmbedding,    // 固定向量（deterministic）
}

// MockEmbedding 关键：相同输入 → 相同输出，保证检索可复现
impl MockEmbedding {
    pub fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.deterministic_hash_vector(t)).collect()
    }
}
```

**Smoke 层基础设施强约束**：
- PG: Testcontainers PostgreSQL（真实进程）
- Milvus: Testcontainers Milvus Standalone（真实进程）
- Object Store: 临时本地目录（通过 `AppConfig.object_storage.base_path` 指向 `TempDir`）
  - **接口一致性要求**：本地目录的读写接口（路径拼接、元数据保存、文件名规则）必须与生产对象存储保持一致，使用同一 `ObjectStore` trait 实现，禁止测试特例
- 禁止对基础设施使用任何 stub/mock

### 5.4 失败可观测性（所有层强制）

测试失败时自动收集并保存：

```
crates/app/tests/e2e_output/
└── {bucket}/                     # failures | llm_real | observability
    └── {run_id}/
        └── {test_name}/
            ├── request.json          # HTTP 请求体（failures bucket）
            ├── response_body.json    # HTTP 响应体（failures bucket）
            ├── trace_id.txt          # 分布式 trace ID
            ├── worker_logs.txt       # worker 日志（最后 500 行）
            ├── retrieval_results.json # 检索结果快照
            ├── model_routing.json    # LLM 路由决策
            └── screenshot.png        # Playwright 截图（如有 HTML）
```

**llm_real / observability bucket**（`#[ignore]` 实跑 LLM 测试，`save_llm_artifact`）：

```
crates/app/tests/e2e_output/{llm_real|observability}/{run_id}/{test_name}/
├── response.json
├── reasoning_summary.txt
├── trace_reasoning.jsonl
├── prompt_snapshots.json
└── metadata.json
```

- 失败快照：`TestContext::save_failure_artifacts()` → `failures/` bucket
- 实跑 LLM 审计：`TestContext::save_llm_artifact()` → `llm_real/` + 镜像写入 `observability/`

所有 CI workflow 的 artifact upload 路径统一为 `crates/app/tests/e2e_output/**/**`。

**路径规范（最终版）**：
- 根目录：`crates/app/tests/e2e_output/`
- Bucket：`failures`（失败快照）、`llm_real`（实跑 LLM 回归）、`observability`（同 run_id 的观测副本，含 reasoning 文件）
- 运行目录：`{bucket}/{run_id}/`（格式：`e2e_{timestamp}_{short_commit}`）
- 测试目录：`{test_name}/`（Rust 测试函数名）
- 产物命名：failures 固定上表 7 个文件名；llm_real/observability 见上表 5+1 个文件名

---

## 6. 实施计划（修订版）

### Phase 0: 基础设施约定（1 天）

1. 在 `tests/product_e2e/` 建立目录结构
2. 实现 `TestContext::new_smoke()`（Mock 版本）
3. 实现 `MockRegistry`（Mock LLM/Search/Embedding）
4. 实现协议层断言库
5. **GitHub Actions 草案**（见 7.1）

### Phase 1: P0 Smoke 用例（2 天）

| 用例 | 文件 | 内容 |
|------|------|------|
| ingestion_smoke.rs | 上传 fixture → 轮询状态 → 验证 PG 有数据 | |
| rag_smoke.rs | 上传 → 问问题 → 验证 citation doc_id 正确 | |
| search_smoke.rs | 问开放问题 → 验证 citation source_type == "web" | |

**目标**：PR 级 CI 5min 内跑完，通过率 > 95%。

### Phase 2: 真实基础设施（2 天）

1. 实现 `TestContext::new_integration()`（Testcontainers PG + Milvus）
2. 用 feature flag 切换 `new_smoke` / `new_integration`
3. 主干合并 workflow 调用 integration 版本

### Phase 3: P1 + P2 用例（3 天）

- 格式输出（HTML/PPT 结构验证）
- 多文档 RAG
- 空文档、损坏文件、超时、429、embedding 降级
- 并发查询
- 重复上传幂等

### Phase 4: 多租户与可观测性（2 天）

- 跨租户隔离测试
- 失败自动收集 artifacts
- nightly workflow + e2e-analyzer 集成

---

## 7. CI/CD 设计

### 7.1 GitHub Actions — Smoke E2E（PR 级）

```yaml
# .github/workflows/smoke-e2e.yml
name: Smoke E2E
on: [pull_request]

jobs:
  smoke-e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run Smoke E2E
        run: cargo test --ignored -p app --test product_e2e -- --test-threads=4
        env:
          E2E_MODE: smoke          # 激活 Mock 依赖
          E2E_MAX_DURATION_SECS: 300
      - name: Upload failure artifacts
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-failure-artifacts
          path: crates/app/tests/e2e_output/**/**
```

### 7.2 GitHub Actions — Product Integration（主干合并）

```yaml
# .github/workflows/product-integration.yml
name: Product Integration
on:
  push:
    branches: [master, main]

jobs:
  product-integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Start Testcontainers
        run: |
          docker compose -f tests/product_e2e/docker-compose.test.yml up -d
      - name: Run Integration Tests
        run: cargo test --ignored -p app --test product_e2e --features integration
        env:
          E2E_MODE: integration     # 激活真实基础设施
          DATABASE_URL: postgres://test:test@localhost:5432/test
          MILVUS_URL: http://localhost:19530
      - name: Teardown
        if: always()
        run: docker compose -f tests/product_e2e/docker-compose.test.yml down -v
```

### 7.3 Nightly E2E

```yaml
# .github/workflows/nightly-e2e.yml
name: Nightly E2E
on:
  schedule:
    - cron: '0 2 * * *'  # 每天凌晨 2 点
  workflow_dispatch:

jobs:
  nightly-e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run Nightly E2E with real dependencies
        run: cargo test --ignored -p app --test product_e2e --features nightly
        env:
          E2E_MODE: nightly
          E2E_LLM_BASE_URL: ${{ secrets.STAGING_LLM_URL }}
          E2E_LLM_API_KEY: ${{ secrets.STAGING_LLM_KEY }}
          E2E_BRAVE_API_KEY: ${{ secrets.BRAVE_API_KEY }}
      - name: Generate regression report
        run: cargo run -p e2e-analyzer -- diff baseline current --output report.md
      - name: Upload report
        uses: actions/upload-artifact@v4
        with:
          name: nightly-report
          path: report.md
```

---

## 8. 示例用例模板

### 8.1 P0 — RAG Smoke 用例

```rust
// tests/product_e2e/smoke/rag_smoke.rs
use product_e2e::{HttpResponse, ChatResponse, assertions::*};

#[tokio::test]
async fn rag_document_qa_returns_citation() {
    let ctx = TestContext::new_smoke().await;

    // 1. 上传文档
    let upload = ctx.upload_document("fixtures/antifragile.txt").await.unwrap();
    assert_eq!(upload.status, 202);

    // 2. 等待 ingestion 完成
    let status = ctx.wait_for_ingestion(&upload.document_id, Duration::from_secs(30)).await.unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    // 3. 发起 RAG 查询 → 返回 HTTP 原始响应
    let http_resp: HttpResponse = ctx.chat("What is antifragility?", &[upload.document_id]).await.unwrap();

    // 4. 协议层断言（只验 HTTP 契约，不依赖业务字段）
    assert_http_ok(&http_resp);                      // status == 200
    assert_schema_valid(&http_resp.body_json, &CHAT_RESPONSE_SCHEMA);

    // 5. 反序列化为业务对象 → 后续所有产品层断言用它
    let resp: ChatResponse = serde_json::from_value(http_resp.body_json).unwrap();

    // 6. 产品层断言（验业务规则，不依赖 LLM 措辞）
    assert_has_citations(&resp);
    assert_citation_doc_id(&resp, &upload.document_id);
    assert_answer_has_doc_citation(&resp);
    assert!(resp.answer.len() > 50, "answer should be substantive");
}
```

### 8.2 P2 — 降级用例

```rust
// tests/product_e2e/failure/provider_down.rs
use product_e2e::{HttpResponse, ChatResponse, assertions::*};

#[tokio::test]
async fn search_429_returns_degraded_answer() {
    let mut ctx = TestContext::new_smoke().await;
    ctx.mocks.search.set_behavior(MockBehavior::Return429);

    // 3. 发起查询 → HTTP 原始响应
    let http_resp: HttpResponse = ctx.chat("What is the weather in Tokyo?", &[]).await.unwrap();

    // 4. 协议层：仍然 HTTP 200（不暴露内部错误）
    assert_http_ok(&http_resp);

    // 5. 反序列化为业务对象
    let resp: ChatResponse = serde_json::from_value(http_resp.body_json).unwrap();

    // 6. 产品层：没有 web citation，但有降级标识
    let has_web = resp.citations.iter().any(|c| c.source_type == "web");
    assert!(!has_web, "should not have web citation when search is down");
    assert!(
        resp.degrade_trace.iter().any(|d| d.reason == "search_429"),
        "expected degrade_trace to record search_429 fallback"
    );
}
```

---

## 9. 与现有测试的关系

```
product_e2e/（本套件）              crates/app/tests/e2e/（旧集成，待重命名）
    │                                      │
    ├─ HTTP 黑盒入口                       ├─ 直接调用 runtime / 策略层
    ├─ ReActLoop mock 对齐                 ├─ 历史 Strategy 契约
    ├─ Mock LLM + 真 PG/Milvus/worker      ├─ 更快、更偏单元/集成
    └─ Smoke / Integration / Failure       └─ 本地快速回归
```

本套件 **不** 再维护 Commander 模型的 planner/evaluator mock 路由；RAG mock 分两条路径：
- **Happy path**：retrieve 轮返回 `<code>` → 沙箱 stdout → `<code_execution_result>`
- **Fallback path**：`set_mock_rag_skip_codegen(true)` → 服务端 `auto_fallback`

---

## 10. 实现注记（冻结后首批代码评审关注项）

### 10.1 `degrade_trace.reason` 枚举化 — ✅ 已实现

`contracts::chat::DegradeReason` 已落地；`DegradeTraceItem.reason` 为强类型，JSON 仍为 snake_case 字符串（向后兼容）。E2E 使用 `assert_degrade_reason(resp, DegradeReason::…)`。

### 10.2 Search 路由触发条件稳定化

Smoke P0-3（Search 问答）依赖 query 能稳定触发 Search 策略。实现时要求：
- 要么通过 `doc_scope: []`（无文档范围）强制走 Search 路径
- 要么在 Mock LLM 的 prompt 响应中固定返回 `"needs_search": true`
- **禁止**依赖自然语言路由判定（如 query 含"天气""新闻"等关键词），避免 LLM 抖动导致偶发漂移

---

## 11. 实施状态（已完工）

| Phase | 内容 | 状态 |  commit |
|-------|------|:----:|:-------:|
| Phase 0 | 目录结构 + TestContext + MockRegistry + CI workflow 草案 | ✅ | `46ecf70` |
| Phase 1 | P0 Smoke（Ingestion / RAG / Search） | ✅ | `1febe3d` / `8c53a7d` / `37984f6` |
| Phase 2 | Integration（多文档、空文档、并发、重复上传、format_output） | ✅ | `70bb523` / `9abd8b4` |
| Phase 3 | Failure + Tenant（Search 429、Embedding 503、Worker 超时、损坏文件、跨组织隔离） | ✅ | `e3351d5` / `0583713` / `04b34d3` |
| Phase 4 | 失败产物收集 + CI Workflow | ✅ | `7fc06b1` |
| 边界修复 | `doc_scope` 传播到 retrieval tools | ✅ | `53188bc` |

**最终套件结果**（2026-06 架构对齐后）：
```
15+ passed（含 rag_fallback_smoke），mock 路由单元测试同步更新
```

**遗留缺口（非阻塞）**：
- Nightly/Release Gate（需真实 LLM/Search provider + LLM-as-judge 质量层）
- `ChatResponse.format_output` 独立字段（当前 format 内容在 `answer` 字符串中返回）
- codegen SDK 真检索（沙箱 禁 socket；Smoke 用 stdout 模拟证据链，非 SDK HTTP）
