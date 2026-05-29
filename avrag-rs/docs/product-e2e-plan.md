# Product E2E Test Plan (Executable Version)

## 1. 现状与问题

### 1.1 当前测试的真实定位

`crates/app/tests/` 下的 E2E 测试本质是 **Agent 策略集成测试**，不是产品 E2E：

| 维度 | 当前测试 | 产品 E2E |
|------|---------|---------|
| 入口 | `RagContext::from_request` + `StrategyExecutor::run` | HTTP `POST /api/v1/chat` |
| Ingestion | 手动 `ingest_test_document`（只写 Milvus chunks） | HTTP upload → worker pipeline → PG + Milvus |
| Search | Mock 未运行，从未验证 | 真实/受控 SearchProvider |
| 断言 | `"slide" 字符串包含` | 结构化字段 + 协议契约 |

### 1.2 历史运行结论

- 37 次运行，Search 策略**从未执行**
- Chat 早期大量 provider 错误，后期稳定
- RAG 间歇性 evaluator 死循环（145s budget 耗尽）
- 最不稳定因素：LLM provider 抖动 + 字符串断言脆弱

---

## 2. 测试分层矩阵（新增 — 核心设计）

| 测试层 | 触发时机 | 外部依赖 | 目标 | 时长预算 | 通过门槛 |
|--------|---------|---------|------|---------|---------|
| **Smoke E2E** | 每个 PR | Mock LLM、Mock Search、Mock Embedding、Testcontainers PG+Milvus | 验证 HTTP/鉴权/上传/worker 编排/响应结构 | ≤ 5 min | 通过率 > 95%，重跑 2 次结果一致 |
| **Product Integration** | 主干合并后 | 真 PG + 真对象存储 + 真向量库，LLM 可半 Mock | 验证 ingestion 与检索主链路 | ≤ 10 min | 零 P0 用例失败 |
| **Nightly E2E** | 每晚 | 真实 LLM + 真实 Search | 验证真实回答质量与外部集成 | ≤ 30 min | 失败自动归因（provider/infra/product） |
| **Release Gate** | 发布前 | 受控真实依赖 | 少量高价值用例门禁 | ≤ 15 min | 100% P0 通过 |

**强约束**：
- Smoke E2E 不允许调用真实 LLM/Search/Embedding
- 只有 Nightly 和 Release Gate 允许真实外部依赖
- 所有层共享同一套用例代码，通过 feature flag 切换依赖真假

---

## 3. 可量化验收标准

### 3.1 P0 用例（PR 级 Smoke）

| # | 场景 | 验收标准 | 判定方式 |
|---|------|---------|---------|
| 1 | 用户上传文档，worker 完成 ingestion | HTTP 200 → 轮询状态为 `completed` → PG 中存在 summary + TOC + chunks | 结构化 API 响应字段 + PG 查询 |
| 2 | 用户问文档相关问题，系统返回 RAG 回答 | HTTP 200 → 响应 `citations` 数组非空 → `citations[].doc_id` 匹配上传文档 | 响应 JSON 字段断言 |
| 3 | 用户问开放问题，系统返回 Search 回答 | HTTP 200 → 响应 `citations` 数组非空 → `citations[].source_type == "web"` | 响应 JSON 字段断言 |

### 3.2 P1 用例（主干合并后）

| # | 场景 | 验收标准 |
|---|------|---------|
| 4 | Chat + presentation-html | 响应含 `format_output` → `format_output.type == "presentation-html"` → 内容可解析为有效 HTML |
| 5 | Chat + html-renderer | 同上，格式为 `html-renderer` |
| 6 | 多文档 RAG | `citations` 包含 ≥2 个不同 `doc_id` |
| 7 | 空文档上传 | HTTP 200 → 状态 `completed` → 回答"文档内容为空"类降级文案 |

### 3.3 失败场景（P2 — 主干合并后）

| # | 场景 | 验收标准 |
|---|------|---------|
| 8 | 损坏文件上传 | HTTP 4xx/5xx 或状态 `failed` → 有可读错误信息 |
| 9 | Worker 处理中超时 | 状态最终为 `failed` 或 `timeout`，不无限挂起 |
| 10 | Search provider 429 | 降级为内部知识或返回"搜索暂不可用"，不 panic |
| 11 | Embedding 服务不可用 | RAG 降级为 lexical_retrieval 或返回降级文案 |
| 12 | 重复上传同文件 | 幂等，不创建重复文档 |
| 13 | 并发查询同一文档 | 不互相干扰，各自返回正确 citation |

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
// MockRegistry 控制所有外部依赖
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

### 5.4 失败可观测性（所有层强制）

测试失败时自动收集并保存：

```
tests/e2e_output/
└── {run_id}/
    └── {test_name}/
        ├── request.json          # HTTP 请求体
        ├── response.json         # HTTP 响应体
        ├── trace_id.txt          # 分布式 trace ID
        ├── worker_logs/          # worker 日志（最后 500 行）
        ├── retrieval_results.json # 检索结果快照
        ├── model_routing.json    # LLM 路由决策
        └── screenshot.png        # Playwright 截图（如有 HTML）
```

通过 `TestContext::save_failure_artifacts()` 统一实现。

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
          path: crates/app/tests/e2e_output/*/failed-*
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
#[tokio::test]
async fn rag_document_qa_returns_citation() {
    let ctx = TestContext::new_smoke().await;

    // 1. 上传文档
    let upload = ctx.upload_document("fixtures/antifragile.txt").await.unwrap();
    assert_eq!(upload.status, 202);

    // 2. 等待 ingestion 完成
    let status = ctx.wait_for_ingestion(&upload.document_id, Duration::from_secs(30)).await.unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    // 3. 发起 RAG 查询
    let resp = ctx.chat("What is antifragility?", &[upload.document_id]).await.unwrap();

    // 4. 协议层断言
    assert_http_ok(&resp);
    assert_has_citations(&resp);
    assert_citation_doc_id(&resp, &upload.document_id);

    // 5. 产品层断言
    assert_answer_has_doc_citation(&resp);
    assert!(resp.answer.len() > 50, "answer should be substantive");
}
```

### 8.2 P2 — 降级用例

```rust
// tests/product_e2e/failure/provider_down.rs
#[tokio::test]
async fn search_429_returns_degraded_answer() {
    let mut ctx = TestContext::new_smoke().await;
    ctx.mocks.search.set_behavior(MockBehavior::Return429);

    let resp = ctx.chat("What is the weather in Tokyo?", &[]).await.unwrap();

    // 协议层：仍然 HTTP 200（不暴露内部错误）
    assert_http_ok(&resp);

    // 产品层：没有 web citation，但有降级文案
    let has_web = resp.citations.iter().any(|c| c.source_type == "web");
    assert!(!has_web, "should not have web citation when search is down");
    assert!(resp.answer.contains("暂无法搜索") || resp.answer.contains("search unavailable"));
}
```

---

## 9. 与现有测试的关系

```
产品 E2E（新增）          Agent 策略集成测试（保留，后续重命名）
    │                           │
    ├─ HTTP 黑盒入口            ├─ 直接调用 Strategy
    ├─ Mock/真实分层            ├─ 固定 mock
    ├─ 协议+产品断言            ├─ 字符串/状态机断言
    ├─ 慢（5-30min）            ├─ 快（秒级）
    └─ PR/主干/nightly          └─ 本地快速验证
```

**保留现有测试**，后续改名为 `agent_strategy_integration_tests`，继续用于：
- 策略层快速回归
- LLM prompt 调优
- 状态机 schema 验证

---

## 10. 下一步行动

1. **评审本文档** — 确认分层矩阵、验收标准、用例范围
2. **创建 GitHub Issue** — 按 Phase 拆解任务，标记 `good first issue`
3. **Phase 0 开工** — 目录结构 + TestContext + MockRegistry
4. **现有测试更名** — `e2e_*.rs` → `agent_strategy_*.rs`（避免混淆）
