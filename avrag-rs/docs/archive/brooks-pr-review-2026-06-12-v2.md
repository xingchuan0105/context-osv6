# Brooks-Lint Review

**Mode:** PR Review  
**Scope:** 工作区未提交变更 — `concurrent_query.rs`（+11 行）；对 P2-13 测试及其 product_e2e 上下文做深入探测  
**Health Score:** 83/100  
**Trend:** 43 → 83（+40）— 审查范围从 196 文件巨型 diff 收窄至单文件测试变更

**一句话结论：** 本次 diff 仅为调试用的 `eprintln!`，不应合入；同文件还存在「名为并发、实为串行」与「未验证 citation 独立性」两项既有测试缺口，建议在移除调试输出后一并补齐。

---

## 变更概览

| 维度 | 内容 |
|------|------|
| 文件 | `avrag-rs/crates/app/tests/product_e2e/integration/concurrent_query.rs` |
| 行数 | +11 / −0 |
| 意图（推断） | 排查 `assert_has_citations` 失败时，打印 citations 与 tool_results |
| 关联需求 | 模块注释 P2-13：同一文档上的并发查询应产生独立结果 |

### 与同类测试对比

| 测试 | 路径 | `chat_without_mock_chunk_pin` | tool_results 断言 | 并发执行 |
|------|------|-------------------------------|-------------------|----------|
| `rag_document_qa_returns_citation` | smoke/rag_smoke.rs | ✓ | `assert_codegen_bridge_dense_retrieval` 等 | 单请求 |
| `concurrent_rag_queries_return_independent_citations` | integration/concurrent_query.rs | ✓ | **无**（本次 diff 仅 eprintln） | **无**（串行 await） |

---

## Findings

### 🟡 Warning

**Accidental Complexity — 调试输出不应进入版本库**

Symptom: diff 在 `into_business()` 与 `assert_has_citations` 之间插入两段无条件 `eprintln!`，打印 `chat1.citations`、`tool_results` 及 `chat2.answer`；无 `[product_e2e]` 前缀，与 `builder.rs` / `http.rs` 中既有日志约定不一致。

Source: Ousterhout — *A Philosophy of Software Design*, Ch. 3 Strategic vs. Tactical Programming; McConnell — *Code Complete*, 构造阶段应移除临时诊断代码

Consequence: 每次 CI/本地跑通也会向 stderr 倾倒 LLM 答案与工具调用详情，掩盖真实失败信号；合入后主分支长期携带「调查进行中」痕迹，reviewer 无法区分 WIP 与完成态。

Remedy: 合入前删除这两段 `eprintln!`；若需诊断 flaky failure，在断言 helper 的失败消息中嵌入 context（现有 `assert_has_citations` 已含 `answer`），或仅在 `RUST_BACKTRACE`/自定义 env 下打印。

---

**Domain Model Distortion — 测试名与执行方式不一致（「并发」实为串行）**

Symptom: 函数名 `concurrent_rag_queries_return_independent_citations` 与模块注释「Concurrent queries」暗示并行请求；实现为先 `await` `http1` 再 `await` `http2`（约 L30–45），全库 product_e2e 中无 `tokio::join!` / 双 spawn 用于该场景。

Source: Evans — *Domain-Driven Design*, Ubiquitous Language; Meszaros — *xUnit Test Patterns*, 测试名应表达场景与预期

Consequence: 会话隔离、检索缓存、mock 状态等在**真并发**下的竞态不会被此测试捕获；P2-13 在计划层面可能被标记为「已覆盖」而实际未验证并发语义。

Remedy: 用 `tokio::join!(ctx.chat_without_mock_chunk_pin(...), ctx.chat_without_mock_chunk_pin(...))` 或两个独立 `tokio::spawn` + 共享 `TestContext`（若 API 允许）发起并行请求；若产品暂不支持并发，重命名测试与注释为 `sequential_rag_queries_...` 并下调 P2-13 优先级。

---

**Coverage Illusion — 「独立 citations」未被断言**

Symptom: 测试仅对 `chat1`、`chat2` 分别调用 `assert_has_citations`、`assert_citation_doc_id`（同一 `upload.document_id`）；未断言两路回答内容不同、 citation chunk 集合不交叉、或 query 特异性（如「antifragility」vs「who wrote」）。

Source: Feathers — *Working Effectively with Legacy Code*, Ch. 1; Google — *How Google Tests Software*, change coverage vs line coverage

Consequence: 两路请求若因共享 mock/缓存返回相同 citation 集合，测试仍可通过；「independent」仅为命名，无行为约束。

Remedy: 至少增加：`assert_ne!(chat1.answer, chat2.answer)` 或关键词断言（如 chat2 答案含作者相关信息）；可选：对两路 `citations` 的 `chunk_id` 集合做「非完全相同」或「与 query 语义一致」的检查。

---

### 🟢 Suggestion

**Knowledge Duplication — tool_results 调试格式与 assertions 重复**

Symptom: 新增 `eprintln` 中 `tool_results.iter().map(|r| (&r.tool, &r.status))` 与 [`assertions.rs`](../crates/app/tests/product_e2e/assertions.rs) 内 `assert_tool_result_ok` / `assert_distinct_tool_count` 失败消息格式相同。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY

Consequence: 诊断信息维护两处；未来若 `ToolStatus` 展示方式变化，调试块与断言消息分叉。

Remedy: 若需验证 bridge，直接调用现有 `assert_codegen_bridge_dense_retrieval(&chat1)`（与 `rag_smoke.rs` 对齐），而非手写 eprintln。

---

**Test Obscurity — 与 rag_smoke 断言深度不一致**

Symptom: 同用 `chat_without_mock_chunk_pin` 走真实 bridge，但 `rag_smoke` 含 `assert_citations_use_document_chunks`、`degrade_trace` 空检查等；本测试仅 citation 数量与 doc_id，且本次 diff 增加的是 stderr 输出而非结构化断言。

Source: Meszaros — *xUnit Test Patterns*, Behavior Verification

Consequence: integration 层测试比 smoke 层更弱，金字塔倒挂；bridge 回归在本路径上反馈更慢、更模糊。

Remedy: 从 `rag_smoke` 复制与 bridge 相关的断言子集（tool_results + document chunk ids），保持 smoke 快、integration 深的分工。

---

## Quick Test Check（Step 7）

| 信号 | 结果 |
|------|------|
| 生产代码变更 + 缺测试 | 跳过 — 仅测试文件变更 |
| Mock Abuse | 跳过 — 无 mock 测试改动 |
| Test Obscurity | 见上 — 调试 eprintln 降低可读性；测试名/行为偏差见 Warning |

---

## 环境备注（不在 Health Score 内）

- `cargo test -p app concurrent_rag_queries_return_independent_citations --no-run` 因 `app_bootstrap::adapters` 私有路径等问题未能完成编译，属 workspace 既有问题，不在本次 11 行 diff 范围内。
- 上一份全量 PR 审查（196 文件）已归档至 [`archive/brooks-pr-review-2026-06-12-v1.md`](./archive/brooks-pr-review-2026-06-12-v1.md)。

---

## 推荐修复顺序

1. **删除** 本次 diff 中的 `eprintln!`（合入前必须）
2. 改为 `tokio::join!` 实现真并发，或重命名测试以反映串行语义
3. 增加「独立性」断言（答案差异 / citation 差异）
4. 对齐 `rag_smoke` 的 bridge 断言（`assert_codegen_bridge_dense_retrieval` 等）

---

## Summary

变更体量极小，风险集中在「调试代码误合入」与 P2-13 测试长期名不副实：既不并发，也未证明 citations 独立。移除 `eprintln` 后，建议用一次小步 commit 补齐并发执行与独立性断言，再关闭 P2-13；无需再跑全库 196 文件级 brooks 审查，除非工作区再次扩大。

相关文档：[`brooks-test-quality-review-2026-06-12.md`](./brooks-test-quality-review-2026-06-12.md)（全库测试质量）；[`product-e2e-plan.md`](./product-e2e-plan.md)（P2-13 来源）。
