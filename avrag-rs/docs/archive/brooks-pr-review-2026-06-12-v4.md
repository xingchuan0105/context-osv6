# Brooks-Lint Review

**Mode:** PR Review
**Scope:** 工作区未提交代码变更 8 文件（+114/−74）：`product_e2e` 测试基建 4 文件 + `e2e-gates.md` + `frontend_next` 运行时传输层 3 文件。brooks-* 报告文档的归档/重写为并行审查工作流产物，不计入审查对象。深入探测（v4）：已核对 transport 适配层、IPC 契约、mock 服务器回退链、CI workflow feature 标志与孤儿代码。
**Health Score:** 78/100
**Trend:** 93 → 78（−15，对照 v3 报告口径；`.brooks-lint-history.json` 中上一条 PR Review 记录为 43，v2/v3 当时未入库，本次起恢复入库）

**一句话结论：** 两条工作流（Rust E2E 稳定化 + 前端 transport 收敛）方向都正确，且 `e2e-gates.md` 的诚实记录值得肯定；但 E2E 侧留下一条「两端已建、中段缺失」的死管道与一个弱化未验证的并发门禁，前端侧 Tauri IPC 适配层对 Web 路径契约的收窄（AbortSignal / ApiError / body）将随迁移面扩大而放大。

---

## 变更概览

| 文件 | 变更 | 目的 |
|------|------|------|
| `tests/product_e2e/setup.rs` | PG/Milvus 释放路径的 slot 清理移入 `block_on_with_timeout` 异步块 | 修复 `#[tokio::test]` drop 内 `blocking_lock()` panic |
| `tests/product_e2e/mock_servers.rs` | mock LLM handler 读取 `x-mock-rag-query` 头；`resolve_dense_search_query` / `mock_rag_retrieve_codegen_content` 增加 `header_query` 参数 | 尝试 per-request 查询注入（替代有竞态的全局 cell） |
| `tests/product_e2e/test_context/http.rs` | chat 请求加 `x-mock-rag-query` 头（非流式无条件；流式仅 `agent_type == "rag"`） | 同上 |
| `tests/product_e2e/integration/concurrent_query.rs` | 删除 `assert_independent_citation_chunks`、答案差异与主题关键词断言 | mock LLM 合成与查询无关，无法满足原断言 |
| `docs/e2e-gates.md` | 新增「Integration regression status」分区：已修/未验证/开放问题 + 复跑清单 | 记录修复进展与残留风险 |
| `hooks/chat-session/use-chat-stream.ts` | `streamWorkspaceChat` → transport 层 `streamChat`；ref 同步 effect 补 deps 数组 | 桌面/Web 双端复用同一 hook |
| `lib/api-access/client.ts` | 本地 `request` 实现替换为 transport 层 `restRequest` 委托 | 收敛重复的 fetch 逻辑 |
| `lib/runtime/tauri-ipc.ts` | `requestViaIPC` body 解析增加 `typeof === "string"` 守卫 | 防止非字符串 body 触发 `JSON.parse` 异常 |

核查通过的项（不构成 finding）：`integration-e2e.yml` 已带 `--features product-e2e`（e2e-gates.md 开放问题 ③ 可销项）；`use-chat-stream.ts` 的 effect deps 数组与 9 个 ref 写入一一对应，无 stale 风险；api-access 收敛到 transport 是正确的 DRY 方向；v3 报告预警的 `assert_independent_citation_chunks` 单文档 flaky 风险已被本次变更响应。

---

## Findings

### 🟡 Warning

**Accidental Complexity — `x-mock-rag-query` 双端死管道：发送端与接收端都已建好，中间却不通**

Symptom: `test_context/http.rs` 在 chat API 请求上发送 `x-mock-rag-query` 头（非流式 L285 无条件、流式 L475 仅 rag 模式，两处行为还不一致）；`mock_servers.rs` L846 在 mock LLM handler 读取同名头。但该头发给的是被测应用的 `/api/v1/chat`，应用的 LLM 客户端不会把入站请求头转发给 LLM 上游——`e2e-gates.md` 自己两次写明 "does **not** propagate to mock LLM" / "is **not** forwarded to mock LLM today"。运行时 `header_query` 恒为 `None`，新参数与新请求头均为死代码。

Source: Fowler — *Refactoring*, Speculative Generality; Ousterhout — *A Philosophy of Software Design*, Ch. 3 Strategic vs. Tactical Programming

Consequence: 后来者（很可能就是下一个要修 `concurrent_query` 的人）看到 mock 侧支持 per-request 头，会假定注入机制可用；而 `resolve_dense_search_query` 的四级静默回退链（header → messages 解析 → 全局 cell → `"antifragility"` 默认值）会把「头没传到」完全掩盖成「测试通过但断言的是错误数据」，调试成本极高。

Remedy: 二选一并当周落地：(a) 在 `product-e2e` feature 下让应用的 LLM 客户端透传 `x-mock-rag-query`，机制端到端打通后顺带可恢复 `concurrent_query` 原断言；(b) 删除两端管道，仅保留 e2e-gates.md 的方案记录。不要让「半座桥」进主干。

**Coverage Illusion — `concurrent_query` 门禁弱化后未重跑验证，测试名与断言内容失实，断言助手成孤儿**

Symptom: 测试仍名为 `concurrent_rag_queries_return_independent_citations`，但 independence 相关断言（`assert_independent_citation_chunks`、`assert_ne!` 答案、主题关键词）已全部移除，仅余并发安全断言；`e2e-gates.md` 自记 "Assertions relaxed; **pass not confirmed** after change"，全套 integration 状态 "not green (interrupted mid-run)"；`assertions.rs` 中 `assert_independent_citation_chunks` 失去唯一调用方成为孤儿；另外 `brooks-merged-fix-plan` K6 原计划是「放宽为非完全相同集合或改两文档 scope」，实际执行是整体删除，决策未回写计划。

Source: Feathers — *Working Effectively with Legacy Code*, Ch. 1（未经验证的变更等同无保护变更）; Meszaros — *xUnit Test Patterns*（测试名应表达验证的行为）; Google Engineering — *How Google Tests Software*（change coverage）

Consequence: 套件中唯一的并发独立性门禁消失且无替代物（real-LLM 变体仅停留在文档建议）；误导性测试名让读者以为该性质仍受保护；孤儿 pub fn 在测试二进制中可能触发 dead_code 告警。

Remedy: 按 e2e-gates.md 复跑清单执行三项目标测试确认绿；重命名为如 `concurrent_rag_queries_are_safe_on_codegen_bridge`；孤儿断言要么删除、要么随计划中的 `#[ignore]` real-LLM 变体落位；在 K6 条目回写「mock 路径改为仅验并发安全」的决策。

**LSP violation — `streamChatViaIPC` 接受 `options.signal` 但完全忽略，桌面端「停止生成」失效**

Symptom: 本次将 `use-chat-stream.ts` 从直连 `streamWorkspaceChat` 改接 transport 层 `streamChat` 后，桌面分支落到 `tauri-ipc.ts` 的 `streamChatViaIPC`：签名照抄 Web 实现含 `options?: { signal?: AbortSignal }`，函数体内 signal 无任何消费——不注册 abort 监听、不调用取消 command、事件监听器也只在 invoke 完成后才解除。Web 路径则将 signal 传入 `fetch` 真实生效。

Source: Martin — *Clean Architecture*, Liskov Substitution Principle（适配器声明同一接口却不履行行为契约，替换后破坏调用方）

Consequence: 桌面端用户点「停止」只会清空本地 UI 状态，Rust 核心继续生成、事件继续派发，可能复活已清空的 streaming 状态；`send` 中的 `AbortError` 分支在桌面端成为死路。桌面壳一旦发布即带此缺陷。

Remedy: IPC 路径实现真实取消：`signal.addEventListener("abort", ...)` 中 invoke 一个 `chat_cancel`（携带 `requestId`）并立即 `unlisten()`；为 transport 适配层补一组契约测试（signal 取消、事件停止、错误类型），两个实现共用同一套用例。

**Hyrum's Law — `requestViaIPC` 收窄 REST 契约：错误不再是 `ApiError`、`init.headers` 被丢弃、非字符串 body 静默变 `null`**

Symptom: `api-access/client.ts` 迁移到 `restRequest` 后，桌面分支 `requestViaIPC` 只透传 method/path/body/token：Tauri `invoke` 的拒绝值不是 `ApiError`；调用方传入的自定义 headers 被无声丢弃；本次新增的守卫 `typeof init.body === "string" ? JSON.parse(init.body) : null` 把原本会大声抛 `SyntaxError` 的非字符串 body（如 `FormData`）改为静默置 `null` 发出。前端现有 5 处 `instanceof ApiError` 分支（`use-workspace-data` 的 404 降级、paywall / usage / featureFlag 的 `feature_disabled` 检测、`auth/errors` 的统一文案）在 IPC 路径下全部失配。

Source: Winters et al. — *Software Engineering at Google*, Hyrum's Law（错误类型与状态码是事实契约）; McConnell — *Code Complete*, 防御式编程（失败模式必须可见）

Consequence: api-access 是第一个迁移模块，当下仅影响桌面端 API Key 设置页的错误文案；但 transport 收敛是明确方向，每迁移一个模块，桌面端的错误处理/降级逻辑就多失效一片，且 body 误用时无任何报错信号。

Remedy: 在 `requestViaIPC` 内把 Rust 侧错误映射回 `new ApiError(status, code, message)`（Tauri command 返回结构化错误）；非字符串 body 直接 `throw new TypeError("requestViaIPC only supports JSON string bodies")`；在 transport.ts 模块注释中明确「IPC 路径不支持自定义 headers」。

### 🟢 Suggestion

**Knowledge Duplication — api-access 迁移留下孤儿 `decodeError` / `ErrorEnvelope`，`request` 退化为 Middle Man**

Symptom: `lib/api-access/client.ts` 的 `request` 改为单行委托 `restRequest` 后，本文件的 `decodeError`（14 行）与 `ErrorEnvelope` 类型失去全部调用方，`ApiError` import 仅被孤儿引用；`request<T>` 本身成为纯转发（3 个调用点可直呼 `restRequest`）。`tsconfig` 未开 `noUnusedLocals`，编译不会提醒。

Source: Fowler — *Refactoring*, Duplicate Code / Middle Man

Consequence: 与 `auth/client.ts` 中真正在用的 `decodeError` 形成会漂移的死副本，下次有人改错误解析逻辑时可能改错文件。

Remedy: 删除 `decodeError`、`ErrorEnvelope` 与 `ApiError` import；`request` 内联为直接调用 `restRequest`（或保留但加一行注释说明仅为收口点）。

**Cognitive Overload — PG 与 Milvus 释放路径形状不一致，teardown 超时被静默吞掉**

Symptom: `release_shared_postgres` 将容器停止与 slot 清理放进同一个 `block_on_with_timeout` 块；`release_shared_milvus` 因 keep-alive 条件拆成两个连续块。`block_on_with_timeout` 内 `let _ = tokio::time::timeout(10s, fut)` 丢弃超时结果，超时发生时 slot 残留只能依赖 `acquire_shared_postgres` 的 ready-check 自愈，无任何日志线索。

Source: Ousterhout — *A Philosophy of Software Design*（一致性降低认知成本）; McConnell — *Code Complete*（错误路径应可见）

Consequence: 读者需要逐行比对才能确认两条释放路径语义等价；CI 偶发的「上一个容器没停干净」类问题缺少第一现场日志。

Remedy: 抽一个共用的「停止 + slot 清理」收口函数统一两处形状；`timeout` 返回 `Err` 时 `eprintln!("[product_e2e] teardown timed out: {container_name}")`。

**Recommended fix order:** Coverage Illusion（先重跑验证，恢复门禁可信度）→ 死管道（打通或拆除）→ LSP / Hyrum（IPC 契约，可同一个 PR）→ 两个 Suggestion 顺手清理。

---

## Summary

最重要的动作：执行 `e2e-gates.md` 自带的复跑清单，把「assertions relaxed; pass not confirmed」变成已验证状态——在此之前 E2E 侧的修复成果无法兑现。其次在 `x-mock-rag-query` 上做出明确决策（打通或拆除），避免半成品机制进入主干误导后续并发测试。前端 transport 收敛方向正确，但应在迁移面扩大前先补齐 IPC 适配层的三项契约（AbortSignal、ApiError、body 校验），否则每迁移一个模块就埋一片桌面端隐患。整体趋势：相比 v3（93）下降 15 分，主因不是回退，而是本轮变更把两条工作流的「半成品状态」同时暴露在工作区；另注意本工作区混有两个不相关工作流的改动，建议分开提交以保持每个 PR 可独立评审。

---

*报告生成：2026-06-12 · Brooks-Lint PR Review v4 · 上一版报告已归档至 `docs/archive/brooks-pr-review-2026-06-12-v3.md`*
