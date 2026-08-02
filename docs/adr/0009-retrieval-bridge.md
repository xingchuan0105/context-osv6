# ADR 0009: Retrieval Bridge（沙箱 codegen → 宿主 RAG 的 fd 管道 RPC）

## Status

**Accepted** — 2026-08-02（补录：CONTEXT.md 已长期引用，`docs/adr/` 缺失文件；决策本身早已实现）

**Implementation status** — **Shipped**。`avrag-rs/crates/code-interpreter/src/bridge.rs`（宿主侧 `HostBridge` trait + fd3/fd4 管道 RPC）与 `avrag-rs/crates/rag-core/src/runtime/bridge.rs`（`RuntimeBridge` 实现，经 `RagRuntime` 工具派发）均已落地；C1 收拢后 `resolve_doc_ids` 复用全仓唯一 `intersect_doc_scope`（`rag-core/src/runtime/scoped_rag_dispatch.rs`）。

## Context

沙箱 Code Interpreter 里模型会写 `client.dense_search(...)` 之类的检索调用。两个候选方案：

1. **沙箱直连数据平面**——沙箱持有 Milvus/检索端点凭据，模型在沙箱内直接检索。风险：检索入口分裂（沙箱路径绕过宿主授权/作用域强制）；沙箱凭据面大。
2. **沙箱 → 宿主 RPC**——检索只经宿主单入口。沙箱发 JSON RPC，宿主强制 `doc_scope` 后走统一的工具派发，检索结果回传沙箱。

既有事实：工具派发已统一到 `RagRuntime`（ADR-0006：RAG 执行面只认 AgentLoop + ToolCall）；doc-scope 强制是安全接缝（C1）；codegen Python shim 由 `contracts::sdk_primitives::SDK_PRIMITIVES` 注册表单源生成。

## Decision

1. **非网络管道**：检索桥用父进程与沙箱子进程之间的 **fd 管道（fd3/fd4）行分隔 JSON RPC**，不开放网络端口。
2. **宿主强制 scope**：模型写 `client.dense_search(...)` 时，宿主 `RuntimeBridge` 对调用做 `doc_scope` 强制（`resolve_doc_ids` → 唯一 `intersect_doc_scope`），再调用 `tools::dispatch(&runtime, &auth, &tool_call)` 派发。
3. **单入口复用**：沙箱检索不新增派发路径；`HostBridge::call(method, args)` 映射到既有 `RagRuntime` 工具（dense/lexical/doc_profile/doc_summary/…）。
4. **shim codegen 单源**：Python 侧 `client.*` 方法由 `SDK_PRIMITIVES` 注册表 codegen，方法签名与 payload 形状派生自注册表，不手写。

## Consequences

- 检索作用域强制在宿主侧成立，沙箱无数据平面凭据（安全面收敛）。
- doc-scope 语义与 C1 接缝共享同一实现，无第二份 intersect。
- 沙箱结果经 `captured_results`/`CapturedBridgeCall` 回流宿主，供 citation/degrade 组装（K2 别名计数按 worker run 共享）。
- 代价：沙箱内检索有管道 RPC 往返开销；`HostBridge` 是同步 trait 边界，未来若需跨进程扩展（如独立检索服务）需重设计。
