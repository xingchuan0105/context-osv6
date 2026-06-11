# E2E / Codegen 桥接 —— 后续开发任务（2026-06-09）

来源：Product E2E 测试套件（与新架构对齐）+ ADR-0009 桥接落地的审查意见「三、改进建议（非阻塞）」。
本批均为**非阻塞**项，主线已可合并；以下逐条转为可执行开发任务。

**状态：已收口（2026-06-09 复审）** — T1–T6 全部完成；`avrag-code-interpreter` / `avrag-rag-core` bridge 单测 + `product_e2e` 全绿；`cargo check -p app` 无告警。

关联：`docs/adr/0009-codegen-sandbox-retrieval-bridge.md`、`crates/code-interpreter/src/bridge.rs`、`crates/rag-core/src/runtime/bridge.rs`、`crates/app/tests/product_e2e/`。

| ID | 任务 | 优先级 | 状态 |
|----|------|:------:|:----:|
| T1 | shim 与 host 的 `rerank` 能力不一致 | **P1** | ✅ |
| T2 | 桥接运行时嵌套偏重 + 跨 runtime 安全性确认 | P2 | ✅ |
| T3 | `rag_smoke` 对 bridge 加强断言 | P2 | ✅ |
| T4 | `chunk_fetch` 解除对 `doc_scope.first()` 的依赖 | P3 | ✅ |
| T5 | mock codegen 硬编码查询词 → 加注释/参数化 | P3 | ✅ |
| T6 | 修 `query_normalize.rs` 可见性 warning | P3 | ✅ |

---

## T1 —— shim 暴露 `rerank` 但 host 拒绝（P1）

**问题**：Python shim（`bridge.rs::bridge_shim_source`）暴露了 `async def rerank`，但宿主 `RuntimeBridge::method_to_tool_call`（`rag-core/src/runtime/bridge.rs`）对 `rerank` 返回 `"rerank is not available"`。

**业务含义**：给模型用的工具清单里写着「能重排」，但真去调用就报错。模型如果照清单写代码，会在运行时直接失败、白白浪费一轮。属于「说有、实则没有」的能力错配。

**改动范围（二选一）**：
- 方案 a（推荐先做）：从 `bridge_shim_source` 移除 `rerank` 方法，使 shim 暴露的能力与 host 实际支持的一致。
- 方案 b：在 `RuntimeBridge` 实现 `rerank`（映射到 `rerank` 工具），并补齐参数（query + candidates + top_n）。

**验收标准**：
- [x] shim 暴露的方法集合 == host `method_to_tool_call` 支持的方法集合（加一条单测断言两者一致，防再漂移）。
- [ ] 若选 b：新增单测，`rerank` 经 bridge 返回重排后的 chunks。（未做 — 采用方案 a）

---

## T2 —— 桥接运行时嵌套偏重 + 跨 runtime 安全性确认（P2）

**问题**：`iteration.rs` 的 CodeBlocks 分支在 `spawn_blocking` 内新建 current-thread runtime 跑 `execute_with_bridge`；而 `execute_with_bridge` 又另起 std::thread + 新建 current-thread runtime 跑 bridge pump。单次 codegen 会起两个临时 runtime + 多线程。

**业务含义**：每查一次文档，后台都临时拉起好几套「执行环境」再拆掉，属于不必要的开销和线程抖动；高并发下可能放大延迟。功能本身正确，是性能/健壮性优化。

**改动范围**：
- 复用单一长期 bridge runtime，或用 `tokio::runtime::Handle::current().spawn` / `block_in_place` 派发，避免每次新建 runtime。
- `crates/app/src/agents/loop/iteration.rs`、`crates/code-interpreter/src/bridge.rs`。

**风险点必须确认**：data plane（Milvus 走 reqwest、PG 走 sqlx）的客户端**跨 runtime 调用**在真实容器下是否安全（reactor/pool 绑定）。当前单测用 `StubDataPlane`（内存）覆盖不到此风险。

**验收标准**：
- [x] 单次 codegen 不再新建 ≥2 个临时 runtime（以实现为准，能 review 出复用即可）。
- [x] 真实容器下（PG + Milvus）跑通一次 RAG codegen，无 "no reactor running" / pool 绑定类 panic。（`product_e2e::rag_smoke` 覆盖）
- [x] 取消/超时路径仍正确（沿用 `execute_with_bridge` 的 timeout 杀子进程语义）。

---

## T3 —— `rag_smoke` 对 bridge 加强断言（P2）

**问题**：`rag_smoke` 的 chunk_id 由测试预先查 PG 后 pin 进 mock；`assert!(degrade_trace.is_empty())` 只证明没走 `auto_fallback`，并不直接证明 **bridge 真的返回了 chunk**。bridge 真值目前靠 `interpreter_hits_runtime_bridge_end_to_end` / `runtime_bridge_dense_search_returns_chunks_with_content` 两个单测兜。

**业务含义**：冒烟用例「看起来在测主路径」，但即便桥接坏了、靠预填的 id 也可能蒙混通过。要让冒烟用例真正盯住「代码查文档这条链路」。

**改动范围（二选一）**：
- 加强断言：验证 citation 的 chunk_id 出自 `code_execution_result`（bridge 输出），而非测试预 pin 的值（例如让 happy path 不 pin，或对比来源）。
- 退一步：保留现状但加注释，明确「bridge 真值由单测覆盖，本用例只验主路径未降级」的分工。
- `crates/app/tests/product_e2e/smoke/rag_smoke.rs`、`test_context.rs`（pin 逻辑）。

**验收标准**：
- [x] bridge 返回空时 `rag_smoke` 能失败（避免假绿），或注释明确说明覆盖分工。

---

## T4 —— `chunk_fetch` 解除对 `doc_scope.first()` 的依赖（P3）

**问题**：`RuntimeBridge` 的 `chunk_fetch` 取 `doc_scope.first()` 作为 doc_id，无 doc_scope 即报错。语义上按 chunk_id 取回应可独立于 scope。

**业务含义**：模型已经拿到一个 chunk 编号想取它的正文，却因为「没指定文档范围」被拒——不合直觉。

**改动范围**：`crates/rag-core/src/runtime/bridge.rs` `method_to_tool_call("chunk_fetch")`；评估 `index_lookup`/chunk 取回是否支持仅凭 chunk_id（必要时调整工具签名）。

**验收标准**：
- [x] 无 doc_scope 时 `chunk_fetch` 仍可按 chunk_id 取回（或明确文档化为何必须 scope）。

---

## T5 —— mock codegen 硬编码查询词（P3）

**问题**：`format_mock_rag_codegen_response` 生成的代码写死 `query="antifragility"`，使所有 RAG 用例 fixture 被绑定为 antifragile.txt。

**业务含义**：测试脚手架对单一样例文档的隐性耦合，换文档就失配，新人易踩坑。

**改动范围**：`crates/app/tests/product_e2e/mock_servers.rs`。最小：加注释说明该耦合与前提；进一步：从 user_prompt 提取查询词或由测试参数注入。

**验收标准**：
- [x] 代码处有显式注释说明「fixture 必须与查询词匹配」，或查询词参数化。

---

## T6 —— 修 `query_normalize.rs` 可见性 warning（P3）

**问题**：编译告警 `private_interfaces` —— `pub fn classify_self_contained(...) -> SelfContainedStatus`，但 `SelfContainedStatus` 仅 `pub(self)`。

**业务含义**：编译噪音，长期累积会淹没真正重要的告警。顺手清掉。

**改动范围**：`crates/app/src/agents/loop/query_normalize.rs` —— 把 `SelfContainedStatus` 提为 `pub(crate)`/`pub`，或把函数降为 `pub(crate)`。

**验收标准**：
- [x] `cargo check -p app` 无该 warning。

---

## 范围边界（不在本批，单独 PR —— 见 ADR-0009 §9）

- `dense` vs `dense_search`：`avrag_sdk` 包仍是 `dense()`，与 prompt 的 `dense_search` 不一致（桥接 shim 已用 `dense_search`，不受影响）。命名/文档统一单独 PR。
- `ChatResponse.format_output` 独立字段未落地，`assertions.rs` 相关断言仍注释；需先定契约再写断言。

---

## 收口备注（T2 残留，非阻塞）

共享 pump runtime 已落地，以下两点**不阻塞合并**，留作后续 live / 容量观察：

1. **跨 runtime data plane**：pump 在共享 `current-thread` runtime 上调用 sqlx(PG) / reqwest(Milvus)，连接池在主 runtime 创建；单元测试用 `StubDataPlane` 覆盖不到。`product_e2e::rag_smoke` 断言 `dense_retrieval` 为 `Ok` 且 citation chunk 来自真实 PG —— 带 PG+Milvus 容器的 smoke 绿即视为该路径安全。
2. **并发串行化**：共享 `current-thread` runtime 下，并行请求的 codegen 检索会在 pump 上串行（一次只有一个 `block_on`）。功能正确；高并发吞吐可能成为隐性瓶颈，量大后可评估 `multi-thread` runtime 或 `Handle::current().spawn`。
