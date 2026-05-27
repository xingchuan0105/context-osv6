# E2E 测试升级设计：分层专用测试套件 + Playwright 截图 + 回归报告

> 基于方案 B（分层专用测试套件），覆盖 ingestion-answer 端到端链路、策略 × format skill 笛卡尔积输出验证、以及 query-answer 对比回归报告。

---

## 1. 概述

### 目标

升级现有 E2E 测试基础设施，使其能够：
1. 验证 **ingestion-answer 端到端链路**（文档上传 → 解析/分块 → embedding → 入库 → RAG 检索 → answer）
2. 验证 **策略 × format skill × scenario 的 golden output matrix**，覆盖每种组合下最自然、最具代表性的触发场景
3. 对 HTML/PPT 输出做 **Playwright 真实浏览器渲染 + 截图**
4. 生成 **query-answer 对比回归报告**，支持人工 review 和失败定位

### 非目标

- 不在 CI 中自动运行（继续保持 `#[ignore]`，本地手动触发）
- 不做像素级视觉回归比对（只验证渲染成功 + 结构正确）
- 不覆盖除文档上传外的其他 ingestion 场景（如网页爬取）

### 关键设计原则

- **职责分离**：截图、落盘、流程编排、报告生成各自独立模块
- **Run-scoped 隔离**：所有外部资源（Milvus collection、temp dir、HTTP server）挂统一 `run_id`
- **显式失败保留**：失败时保留完整调试产物，成功时最小化存储
- **可观察等待**：所有异步步骤用轮询 + timeout，不用固定 sleep

---

## 2. 架构和文件结构

### 新增文件

```
crates/app/tests/
├── e2e_chat.rs                    ← 现有，不变
├── e2e_rag.rs                     ← 现有，不变
├── e2e_search.rs                  ← 现有，不变
│
├── e2e_format_output.rs           ← 新增：策略 × format skill 笛卡尔积
├── e2e_ingestion_answer.rs        ← 新增：文档上传 → 回答 完整链路
├── e2e_regression_report.rs       ← 新增：收集结果 → 生成对比报告
│
└── e2e/
    ├── config.rs                  ← 现有，扩展 Playwright 配置
    ├── recording_llm.rs           ← 现有，不变
    ├── assertions.rs              ← 现有，新增 format 断言
    ├── playwright_helper.rs       ← 新增：Playwright 渲染 + 截图（Node CLI 子进程）
    └── result_serializer.rs       ← 新增：结果落盘 + 报告生成
```

### 结果落盘目录结构

```
tests/e2e_output/
└── {run_id}/                      ← 格式: e2e_{timestamp}_{short_uuid}
    ├── metadata.json              ← 运行时间、环境快照、总用例数、保留策略
    │                               ← 环境快照至少包含: git commit/branch、Rust toolchain、Node version、
    │                               ← Playwright version、embedding model/endpoint、Milvus endpoint/collection mode
    │
    ├── format_output/
    │   ├── chat__presentation-html__rust_ownership/
    │   │   ├── query.txt
    │   │   ├── answer.txt
    │   │   ├── answer.html
    │   │   ├── screenshot.png
    │   │   ├── llm_calls.jsonl
    │   │   ├── tool_calls.jsonl
    │   │   ├── meta.json
    │   │   ├── diagnostics.json
    │   │   └── retrieved_chunks.json   ← 优先从 retrieval tool call 的原始 payload 提取命中 chunk 摘要
    │   └── ...                         ← case 目录名格式: strategy__format_skill__scenario_slug
    │
    ├── ingestion_answer/
    │   └── rag__antifragile/
    │       └── ...
    │
    └── report.md                  ← 汇总报告
```

---

## 3. 组件设计

### 3.1 `playwright_helper.rs` — Playwright 渲染 + 截图

**核心决策**：优先走 **Node/CLI 子进程方案**，不依赖 `playwright-rs` crate。

理由：`playwright-rs` 仍是 pre-1.0 / API stabilizing，测试基础设施更重视稳定和可 debug，而非 Rust binding 的纯度。

#### 实现方式

Rust 侧职责：
1. 创建 temp dir
2. 写入 `input.html`
3. 调 `node screenshot.js --input=input.html --output=screenshot.png --viewport=1600x900`
4. 读取 png 和 diagnostics JSON
5. 清理 temp dir

Node 脚本职责：
1. 接收 HTML 文件路径、输出 PNG 路径、viewport 参数
2. `chromium.launch({ headless: true })`
3. `page.goto(http://127.0.0.1:temp_port/index.html)`（不用 `file://`）
4. 截图策略区分：
   - `screenshot_webpage()`：`page.screenshot({ path, fullPage: true })`
   - `screenshot_presentation()`：`page.screenshot({ path, clip: viewport })`，截取固定 slide viewport，不 fullPage
5. 收集 console/page errors
6. 将 diagnostics 写到 JSON 文件
7. 退出码返回成功/失败

**本地 HTTP server**：由 `playwright_helper.rs` 内部私有实现，在 temp dir 上启动极简 HTTP server，生命周期绑定单次 render。不提前抽成独立公共模块。

**不用 `file://` 的原因**：本地图片、字体、相对资源在 `file://` 下容易触发浏览器本地文件访问限制。

#### 接口

```rust
pub struct ViewportConfig {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f32,
}

pub enum AspectRatio {
    Wide16_9,
    Standard4_3,
}

impl AspectRatio {
    pub fn viewport(&self) -> ViewportConfig {
        match self {
            Self::Wide16_9 => ViewportConfig {
                width: 1600,
                height: 900,
                device_scale_factor: 1.5,
            },
            Self::Standard4_3 => ViewportConfig {
                width: 1400,
                height: 1050,
                device_scale_factor: 1.5,
            },
        }
    }
}

pub struct PresentationRenderConfig {
    pub aspect_ratio: AspectRatio,
    pub slide_index: usize,
    pub device_scale_factor: f32,
    pub theme_override: Option<String>,
}

pub struct RenderDiagnostics {
    pub console_errors: Vec<String>,
    pub page_errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub struct ScreenshotArtifact {
    pub png_bytes: Vec<u8>,
    pub viewport: ViewportConfig,
    pub diagnostics: RenderDiagnostics,
}

pub async fn screenshot_html(
    html_content: &str,
    viewport: ViewportConfig,
) -> Result<ScreenshotArtifact, PlaywrightError>;

pub async fn screenshot_presentation(
    html_content: &str,
    config: PresentationRenderConfig,
) -> Result<ScreenshotArtifact, PlaywrightError>;

pub async fn screenshot_webpage(
    html_content: &str,
) -> Result<ScreenshotArtifact, PlaywrightError>;
```

#### 失败时保留调试产物

参考 Playwright 的 `retain-on-failure` 模式：
- 失败时额外保留 `trace.zip`（可选）、DOM snapshot、`console_errors.json`
- 成功时仅保留 `screenshot.png`

### 3.2 `result_serializer.rs` — 结果落盘 + 报告生成

#### `TestResult` Schema

```rust
#[derive(Serialize, Deserialize)]
pub struct TestResult {
    pub run_id: String,
    pub test_name: String,
    pub query: String,
    pub strategy: String,
    pub format_skill: Option<String>,
    pub status: TestStatus,
    pub answer_text: String,
    pub answer_html: Option<String>,
    pub screenshot_path: Option<PathBuf>,
    pub llm_calls: Vec<LlmCall>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub retrieval_hits: Option<u32>,
    pub token_usage: Option<TokenUsage>,
    pub duration_ms: u64,
    pub timestamp: String,
    pub error_message: Option<String>,
    pub diagnostics: Option<RenderDiagnostics>,
    pub failure_kind: Option<TestFailureKind>,
}

#[derive(Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Serialize, Deserialize)]
pub enum TestFailureKind {
    DependencyMissing,
    SetupFailed,
    ExecutionFailed,
    AssertionFailed,
    CleanupFailed,
    Timeout,
}
```

#### 产物保留策略

```rust
pub enum ArtifactRetentionPolicy {
    Never,       // 仅保留 meta 和错误摘要
    OnFailure,   // 失败保留，成功最小化（默认推荐）
    Always,      // 全部保留
}
```

- 本地开发：默认 `OnFailure` 或 `Always`
- CI：默认 `OnFailure`

#### 接口

```rust
pub fn save_test_result(
    output_dir: &Path,
    result: &TestResult,
    policy: ArtifactRetentionPolicy,
) -> Result<PathBuf, std::io::Error>;

pub fn load_run_results(run_dir: &Path) -> Vec<TestResult>;

pub fn generate_markdown_report(
    run_dir: &Path,
    results: &[TestResult],
) -> Result<String, std::io::Error>;
```

### 3.3 `e2e_format_output.rs` — Format 输出回归测试

#### 测试矩阵：Golden Scenario

不是纯机械笛卡尔积，而是 `strategy × format_skill × scenario`：

```rust
struct FormatScenario {
    strategy: StrategyKind,
    format_skill: &'static str,
    query: &'static str,
    expected_markers: &'static [&'static str],
}

const SCENARIOS: &[FormatScenario] = &[
    FormatScenario {
        strategy: StrategyKind::Chat,
        format_skill: "presentation-html",
        query: "生成一个 PPT 总结 Rust 所有权机制",
        expected_markers: &["slide", "presentation"],
    },
    FormatScenario {
        strategy: StrategyKind::Rag,
        format_skill: "presentation-html",
        query: "根据 Antifragile 文档，生成一个 PPT 总结其核心观点",
        expected_markers: &["slide", "presentation", "Antifragile"],
    },
    // ... 其他组合
];
```

#### 断言层级

1. answer 非空
2. 若应为 HTML：包含 `<html` 或核心容器标签、可被浏览器成功渲染、有非零尺寸截图、无严重 console/page error
3. 若应为 presentation：至少含 slide-like 容器、截图成功
4. 若应为 tutor/framework：包含预期结构标记或关键 section

### 3.4 `e2e_ingestion_answer.rs` — Ingestion-Answer 端到端

#### 测试流程

1. **生成 run_id**：`e2e_{timestamp}_{short_uuid}`
2. **创建独立 Milvus collection**：`{run_id}_antifragile`
3. **上传文档**：读取 `tests/fixtures/antifragile.pdf`
4. **解析分块**：调用 RAG 解析 pipeline
5. **Embedding**：通过 `E2EConfig` 的 embedding client 生成向量
6. **入库**：写入 Milvus（staging 环境）
7. **等待可查询**（轮询，非 sleep）：
   - collection exists
   - row count / segment flush 完成
   - query 一次 smoke test 能查到刚写的数据
8. **发起 RAG 查询**：构造 `AgentRequest`，query 针对文档内容
9. **运行 agent**：`RagStrategy` + `StrategyExecutor`
10. **验证**：answer 非空、包含文档相关引用、retrieval 工具被调用
11. **保存结果**：落盘到 `e2e_output/{run_id}/ingestion_answer/{test_name}/`
12. **清理**：显式 drop collection

#### 清理策略

三层清理：
1. **主清理**：测试结束时显式 drop run collection
2. **Guard 清理**：即使 panic / early return，通过 guard 尽量执行
3. **兜底 GC**：定期删除超过 TTL 的 `e2e_*` collections

```rust
struct MilvusTestGuard {
    collection_name: String,
    keep_on_failure: bool,
}

impl Drop for MilvusTestGuard {
    fn drop(&mut self) {
        if !self.keep_on_failure {
            // 同步 best-effort 记录未清理告警
        }
    }
}
```

注意：Rust `Drop` 里做 async 不方便，所以正常路径显式 async cleanup，`Drop` 只记录未清理告警。

#### Ingestion 元数据

每个 chunk 元数据至少带：
- `run_id`
- `test_name`
- `source_file`
- `chunk_id`
- `ingested_at`

### 3.5 `e2e_regression_report.rs` — 报告生成

```rust
#[tokio::test]
#[ignore = "requires e2e_format_output and e2e_ingestion_answer to run first"]
async fn generate_regression_report() {
    // 1. 扫描 tests/e2e_output/ 下最新的运行目录
    // 2. 读取所有 TestResult
    // 3. 生成 Markdown 报告：
    //    - 汇总表：query × strategy × format → answer 预览 + 截图链接
    //    - 同一 query 不同 strategy 的对比
    //    - token 消耗统计
    //    - 失败用例列表（含 TestFailureKind 分类）
    //    - Skipped tests 列表
    //    - Render diagnostics 汇总
    //    - Retrieval diagnostics 汇总
    // 4. 报告写入 tests/e2e_output/{run_id}/report.md
}
```

---

## 4. 数据流

```
Query → AgentRequest → StrategyExecutor
                           │
                           ├── LLM calls (recorded by RecordingLlmProvider)
                           ├── Tool calls (recorded by CollectingSink)
                           │
                           ↓
                      AgentRunResult
                           │
              ┌───────────┴───────────┐
              ↓                       ↓
        text answer             HTML answer
              │                       │
              ↓                       ↓
         answer.txt            playwright_helper::render()
              │                       │
              │              ┌────────┴────────┐
              │              ↓                 ↓
              │        screenshot.png    diagnostics.json
              │                       │
              └───────────┬───────────┘
                          ↓
              result_serializer::save()
                          │
                          ↓
              e2e_output/{run_id}/{test_name}/
                          │
                          ↓
              e2e_regression_report.rs 读取所有结果
                          │
                          ↓
                      report.md
```

---

## 5. 错误处理和测试策略

### 5.1 失败分类

```rust
pub enum TestFailureKind {
    DependencyMissing,   // Playwright 未装、Milvus 连不上
    SetupFailed,         // temp dir 创建失败、HTTP server 启动失败
    ExecutionFailed,     // Agent 执行异常、LLM 调用失败
    AssertionFailed,     // answer 为空、截图失败、结构标记缺失
    CleanupFailed,       // collection 删除失败
    Timeout,             // 某阶段超时
}
```

报告里区分：
- 是 Playwright 没装导致 skip
- 还是 Milvus 写入失败
- 还是 answer 空
- 还是 cleanup 没成功

### 5.2 产物保留策略

```rust
pub enum ArtifactRetentionPolicy {
    Never,
    OnFailure,
    Always,
}
```

- `Never`：仅保留 meta 和错误摘要
- `OnFailure`：失败保留完整产物，成功保留最小必要产物（默认推荐）
- `Always`：全量保留

默认：
- 本地开发：`OnFailure` 或 `Always`
- CI：`OnFailure`

### 5.3 超时策略

为每个阶段定义独立 timeout：

| 阶段 | 建议 timeout |
|------|-------------|
| dependency check | 10s |
| HTTP server ready | 10s |
| Playwright render | 30s |
| ingestion (parse + embed + insert) | 60s |
| Milvus query ready (poll) | 30s |
| agent execution | 120s |
| cleanup | 20s |

不用固定 `sleep`，全部改为轮询可观察条件 + timeout。

### 5.4 并发控制

涉及共享外部资源，需限制并发：

- **单 test 内部**：async 并发无共享副作用的步骤
- **同类 case 之间**：默认限制并发
- **写外部状态的测试**（尤其 ingestion）：串行或半串行

```rust
static INGESTION_PERMITS: Semaphore = Semaphore::const_new(1);
```

建议配置：
- `format_output`：并发 2-4
- `ingestion_answer`：并发 1

### 5.5 清理保证

- **主逻辑失败**时，cleanup 继续 best-effort 执行
- **Cleanup 失败**不能覆盖原始失败原因
- Cleanup 错误应单独记录在 `meta.json` / `report.md`

报告格式：
```
primary failure: ExecutionFailed — answer empty after 3 iterations
cleanup warning: CleanupFailed — collection drop timed out after 20s
```

### 5.6 Skip 语义

不满足依赖时：
- `eprintln!("[SKIP] Playwright not available ...")`
- 返回 `TestStatus::Skipped`
- 记入回归报告，不标记为 Passed

---

## 6. Determinism and Reproducibility

E2E 测试涉及 LLM 输出，天然存在不可复现性。以下做法用于将不可复现范围控制在可接受的边界内：

- **固定 seed / temperature**：所有 LLM 调用使用固定 temperature（如 `0.0` 或 `0.1`），降低输出漂移
- **固定 query fixture**：golden scenario 的 query 文本保持稳定，不轻易改名或改语义
- **固定 source fixture**：ingestion 用的文档（如 `antifragile.pdf`）固定版本，不随外部更新而变
- **输出断言以结构和语义为主**：不对比全文精确匹配，只验证核心标记、渲染成功、引用来源存在
- **环境快照归档**：每次运行记录 git commit、toolchain、模型版本，便于事后追溯"为什么上次能过这次不能"

---

## 7. 运行方式

```bash
# 运行 format 输出测试
cargo test --ignored -p app --test e2e_format_output

# 运行 ingestion-answer 测试
cargo test --ignored -p app --test e2e_ingestion_answer

# 生成回归报告（依赖前两层的落盘结果）
cargo test --ignored -p app --test e2e_regression_report

# 串行运行 ingestion（避免并发冲突）
cargo test --ignored -p app --test e2e_ingestion_answer -- --test-threads=1
```

---

## 7. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Playwright 未安装导致大量 skip | 高 | 低 | 显式 skip 记入报告，不影响现有测试 |
| Milvus staging 不稳定导致 flaky | 中 | 中 | run-scoped 隔离 + 轮询等待 + 串行运行 |
| LLM 输出不可复现导致截图不一致 | 高 | 低 | 不对比像素，只验证渲染成功 + 结构正确 |
| 产物积累占用磁盘 | 中 | 低 | ArtifactRetentionPolicy + TTL 兜底清理 |
| 测试运行时间过长 | 中 | 中 | 分层运行，可单独跑某一层；并发控制 ingestion 串行 |
