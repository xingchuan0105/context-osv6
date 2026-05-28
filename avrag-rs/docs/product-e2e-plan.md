# Product E2E Test Plan

## 1. 现状与问题

### 1.1 当前测试架构

当前 `crates/app/tests/` 下的 E2E 测试本质是 **Agent 策略集成测试**，不是产品 E2E：

| 维度 | 当前测试 | 产品 E2E |
|------|---------|---------|
| 入口 | `RagContext::from_request` + `StrategyExecutor::run` | HTTP `POST /api/v1/chat` |
| Ingestion | 手动 `ingest_test_document`（只写 Milvus chunks） | 完整 pipeline：upload → parse → chunk → summary → index → embed → graph |
| 数据层 | 只有 Milvus | PG + Milvus + Object Store |
| Worker | ❌ 不存在 | ✅ 独立进程，异步处理 |
| Search | 使用 Mock，从未运行 | 真实 SearchProvider（Brave） |
| 断言 | 字符串包含（`"slide"`） | 语义/结构验证 |

### 1.2 历史运行数据（关键结论）

- 37 次运行中，**Search 策略从未运行过**（代码 panic）
- Chat 早期大量失败（provider 不稳定），后期基本稳定
- RAG 间歇性 evaluator 死循环（145s budget 耗尽）
- 最稳定的测试是 `chat__html-renderer`（不依赖 format skill）

### 1.3 核心判断

> 当前测试即使 100% 通过，也不能证明"产品功能正常"。

需要一套从 **HTTP API → 完整 Ingestion → 回答** 的端到端测试。

---

## 2. 目标

建立一套**产品级 E2E 测试**，验证：

1. **用户能上传文档**，系统能完成完整 ingestion（含 summary、index、graph）
2. **用户能发起 Chat/RAG/Search 查询**，通过 HTTP API 得到正确回答
3. **系统降级策略有效**（当工具不可用时优雅降级，不死循环）
4. **回归可检测**（与基线对比，发现退化）

---

## 3. 测试范围（MVP）

### Phase 1: 核心链路（P0）

| 场景 | 用户动作 | 验证点 |
|------|---------|--------|
| **文档上传 + Chat** | 上传 txt → 问"文档讲了什么" | 回答基于文档内容，有 citation |
| **文档上传 + RAG** | 上传 pdf → 问特定问题 → 期望 RAG 路由 | 状态机走 Plan→Retrieve→Evaluate→Answer |
| **纯 Chat** | 问开放问题（无文档） | 路由到 Chat 策略，回答合理 |
| **Search** | 问需要外部知识的问题 | 路由到 Search，有 web citation |

### Phase 2: 格式输出（P1）

| 场景 | 验证点 |
|------|--------|
| Chat + presentation-html | 输出可渲染的 HTML slides |
| Chat + html-renderer | 输出完整 HTML 页面 |
| Chat + step-by-step-tutor | 输出结构化教学步骤 |

### Phase 3: 边界情况（P2）

| 场景 | 验证点 |
|------|--------|
| 空文档 | 优雅降级，不 panic |
| 大文档 | ingestion 完成，不超时 |
| 多文档 RAG | 跨文档检索，回答综合多来源 |
| 并发查询 | 不互相干扰 |

---

## 4. 技术架构

### 4.1 测试组织

```
crates/app/tests/product_e2e/
├── mod.rs              # 共享基础设施
├── fixtures/           # 测试文档
│   ├── antifragile.txt
│   └── simple.md
├── setup.rs            # Testcontainers / 服务启动
├── scenarios/
│   ├── chat_basic.rs   # Phase 1
│   ├── rag_basic.rs
│   ├── search_basic.rs
│   └── ingestion_full.rs
└── assertions.rs       # 语义断言库
```

### 4.2 基础设施方案

**方案 A：Testcontainers（推荐）**

```rust
// tests/product_e2e/setup.rs
pub async fn start_product_services() -> TestContext {
    // 1. PostgreSQL (testcontainers)
    let pg = start_postgres().await;
    // 2. Milvus (testcontainers 或 embedded)
    let milvus = start_milvus().await;
    // 3. Redis (testcontainers，可选)
    let redis = start_redis().await;
    // 4. MinIO / 本地文件系统 (Object Store)
    let object_store = LocalFileSystemObjectStore::new(temp_dir());
    // 5. 启动 AppState
    let config = AppConfig {
        database_url: Some(pg.connection_string()),
        milvus: MilvusConfig { url: milvus.url(), ... },
        redis: RedisConfig { url: redis.url(), ... },
        object_storage: ObjectStorageConfig { base_path: temp_dir(), ... },
        // LLM/Embedding 指向 staging 环境或 mock
        agent_llm: staging_llm_config(),
        embedding: staging_embedding_config(),
        search: staging_search_config(), // 或 mock
        ..AppConfig::default()
    };
    let app_state = AppState::bootstrap(config).await.unwrap();
    // 6. 启动 HTTP server (绑定随机端口)
    let router = build_router(app_state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(axum::serve(listener, router));
    // 7. 启动 Worker (内嵌或独立进程)
    let worker = spawn_worker(pg.connection_string(), ...).await;

    TestContext { app_state, port, pg, milvus, worker }
}
```

**方案 B：Staging 环境复用**

如果 staging 环境稳定，可以直接指向 staging 的 PG + Milvus + 服务：
- 优点：不用管理容器，更快
- 缺点：测试互相干扰，不能并行

**推荐 Phase 1 用方案 A（Testcontainers），Phase 2 增加方案 B 作为 nightly。**

### 4.3 Ingestion 测试策略

当前 `ingest_test_document` 只写 Milvus chunks。产品 E2E 需要完整 pipeline：

```rust
async fn ingest_document_full(
    ctx: &TestContext,
    fixture: &str,
) -> anyhow::Result<String> {
    // 1. 通过 HTTP API 上传文档
    let upload_resp = upload_document(&ctx.client, fixture).await?;
    let doc_id = upload_resp.document_id;
    // 2. 等待 worker 完成 ingestion
    wait_for_document_status(&ctx, &doc_id, DocumentStatus::Completed, timeout_secs: 60).await?;
    // 3. 验证 summary、index、chunks 都已生成
    let summary = ctx.app_state.get_document_summary(&doc_id).await?;
    assert!(summary.is_some(), "document summary should be generated");
    let toc = ctx.app_state.list_document_toc(&doc_id).await?;
    assert!(!toc.is_empty(), "document TOC should be generated");
    let chunks = ctx.app_state.list_document_chunks(&doc_id).await?;
    assert!(!chunks.is_empty(), "document chunks should be generated");
    Ok(doc_id)
}
```

### 4.4 断言策略

**从"字符串包含"升级为"语义/结构验证"：**

```rust
// assertions.rs

/// 验证回答引用了指定文档的证据
pub fn assert_answer_has_citation(answer: &str, doc_id: &str) {
    assert!(answer.contains(&format!("[{}]", doc_id)) || answer.contains("根据文档"),
        "answer should cite the document");
}

/// 验证 HTML 输出结构有效（可解析）
pub fn assert_html_valid(html: &str) {
    let document = scraper::Html::parse_document(html);
    assert!(document.select(&scraper::Selector::parse("html").unwrap()).next().is_some());
    assert!(document.select(&scraper::Selector::parse("body").unwrap()).next().is_some());
}

/// 验证 PPT 结构（有 slides）
pub fn assert_has_slides(html: &str, min_slides: usize) {
    let document = scraper::Html::parse_document(html);
    let slides: Vec<_> = document.select(&scraper::Selector::parse(".slide, [class*=slide]").unwrap()).collect();
    assert!(slides.len() >= min_slides, "expected at least {} slides, got {}", min_slides, slides.len());
}

/// 验证回答与查询语义相关（使用 embedding 相似度）
pub async fn assert_semantically_relevant(answer: &str, query: &str, embedding_client: &EmbeddingClient) {
    let answer_emb = embedding_client.embed(&[answer]).await.unwrap()[0].clone();
    let query_emb = embedding_client.embed(&[query]).await.unwrap()[0].clone();
    let similarity = cosine_similarity(&answer_emb, &query_emb);
    assert!(similarity > 0.5, "answer should be semantically relevant to query (sim={})", similarity);
}
```

### 4.5 Search 测试策略

Search 需要真实的外部搜索能力。方案：

1. **Mock SearchProvider**（用于 CI）：返回固定结果，验证策略流程
2. **真实 Brave API**（用于 nightly）：验证端到端质量

```rust
// 方案 1: Mock（默认）
struct E2ESearchProvider {
    fixtures: HashMap<String, Vec<SearchResult>>,
}

// 方案 2: 真实（需 BRAVE_API_KEY）
// 直接复用 production SearchExecutor
```

---

## 5. 实施计划

### Phase 1: 基础设施（2-3 天）

1. **Testcontainers 集成**
   - 添加 `testcontainers` 依赖
   - 实现 `start_postgres()`、`start_milvus()`（或嵌入式 Milvus Lite）
   - 实现 `TestContext` 生命周期管理（Drop 自动清理）

2. **HTTP 测试客户端**
   - 封装 `reqwest` 客户端，自动处理 auth
   - 提供 `upload_document()`、`chat()` 等 helper

3. **Worker 集成**
   - 在测试中内嵌启动 `PgTaskProcessor`（不跑完整 worker bin）
   - 或实现一个同步 ingestion helper 直接调用 `run_document_pipeline`

### Phase 2: 核心场景测试（2-3 天）

1. **ingestion_full.rs**：上传 → 等待完成 → 验证 summary/index/chunks
2. **chat_basic.rs**：纯 Chat 查询 → 验证路由和回答
3. **rag_basic.rs**：上传文档 → RAG 查询 → 验证 citation 和状态机
4. **search_basic.rs**：Search 查询 → 验证 web citation（mock）

### Phase 3: 格式输出测试（1-2 天）

1. 复用现有 `e2e_format_output.rs` 的逻辑，但走 HTTP API
2. 用 Playwright 截图验证渲染结果

### Phase 4: CI 集成（1 天）

1. 添加 GitHub Actions workflow
2. 并行运行（每个测试用独立 TestContext）
3. 集成 `e2e-analyzer` diff 和 baseline 功能

---

## 6. 风险与对策

| 风险 | 影响 | 对策 |
|------|------|------|
| Testcontainers 启动慢（PG+Milvus） | 测试时间 > 10min | 使用 Milvus Lite（嵌入式）或复用容器；并行测试 |
| LLM 调用成本高 | CI 费用高 | Mock LLM 用于结构测试；真实 LLM 只用于 nightly |
| Milvus schema 冲突 | 多测试并行失败 | 每个测试独立 collection_prefix |
| Worker 异步难以断言 | ingestion 测试不稳定 | 轮询等待 + 超时；或内嵌同步 pipeline |
| Search 需要外部 API | 不可靠 | Mock 用于 CI；真实用于 nightly |

---

## 7. 与现有测试的关系

```
产品 E2E（新增）          当前 Agent 策略测试（保留）
    │                           │
    ├─ HTTP API 入口            ├─ 直接调用 Strategy
    ├─ 完整 Ingestion           ├─ 手动 chunk+embed
    ├─ 真实服务依赖             ├─ 最小依赖
    ├─ 语义断言                 ├─ 字符串断言
    └─ 慢（分钟级）             └─ 快（秒级，如 mock LLM）
```

**保留当前测试**，改名为 `agent_strategy_integration_tests`，继续用于：
- 快速验证策略层改动
- LLM prompt 调优
- 状态机回归

**新增产品 E2E**，用于：
- 验证完整产品链路
- 发布前门禁
- 基础设施变更回归

---

## 8. 下一步行动

1. **评审本计划** — 确认范围、方案、优先级
2. **创建 Implementation Issue** — 按 Phase 拆解任务
3. **先实现 Phase 1 基础设施** — Testcontainers + TestContext
4. **并行修复 Search Mock 的 panic** — 让现有 format output 测试也能覆盖 Search
