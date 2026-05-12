# Kilo Agent 迁移任务验收报告

验收时间：2026-04-30 09:14:09 CST
工作区：`/home/chuan/context-osv6/avrag-rs`

## 结论

Kilo 这轮实现已经明显推进到“主干基本成形 + 关键回归验证已补”的状态：

- Chat/general 主路径基本完成，可以认为已达验收线。
- WebSearch Brave 主路径已经接入，并且 Search non-stream/stream 都走 `UnifiedAgentService -> WebSearchAgent`；本轮已补 fake LLM answer synthesis 与 streaming event order 回归测试。剩余主要是 live Brave LLM Context smoke、degrade/observability 语义说明。
- RAG v2 core 关键拓扑大体完成：schema guard、original query text dense、graph 结构化输入、两 placeholder 规则都有代码和测试支撑；本轮已补 Minsky query→answer E2E artifact。该 E2E 反而确认了当前 production RAG 仍由 `MainAgent` planner/answer 承载，`RagAgent` 仍是 fail-closed adapter，未成为真实生产 agent。
- 后端关键/full checks 与前端 stream/ui/typecheck 本轮均已重跑通过；此前 `cargo test -p transport-http` 的两个失败点已定位并用最小测试修正收口。
- 当前 diff 范围仍偏大，包含明显超出本任务主线的 `crates/ingestion/src/parser/mineru.rs` 大改和大量 untracked `.hermes`/`.kilo` artifacts；不建议原样合并。

综合评分：84/100。
合并就绪度：76/100。
建议：不要直接整包 merge；先拆分/清理 diff，再补 live Brave smoke / RAG agent 架构收口。

## 分项评分

| 模块 | 评分 | 状态 |
|---|---:|---|
| Chat/general 主路径 | 92/100 | 基本完成 |
| WebSearch Brave 主路径 | 83/100 | 主路径和关键回归测试已补，缺 live smoke |
| RAG plan v2 core | 86/100 | core + E2E artifact 完成，agent 架构未完全收口 |
| 前后端 contract / streaming | 90/100 | 后端与前端本轮均重跑通过 |
| 安全与 diff hygiene | 72/100 | 未见 hardcoded secret，但 scope 仍太脏 |

## 1. Chat/general 验收

### 已完成

1. `general` 作为 alias 解析到 canonical `chat`。
   - `crates/app/src/agents/mod.rs:21-40`

2. non-streaming Chat/general 已走统一 agent service。
   - `crates/app/src/chat/service_modes.rs:138-206`
   - 路径：`execute_general_mode_core -> build_agent_request(... AgentKind::Chat) -> agent_service.run(...)`
   - 输出：`mode = chat`、`response.agent_type = chat`、`trace.mode = chat`

3. streaming Chat/general 已走统一 agent service 和 app 层 `SseSink`。
   - `crates/app/src/lib_impl/chat_streaming.rs:293-379`
   - 路径：`execute_general_chat_stream -> UnifiedAgentService -> ChatAgent -> SseSink.without_done_event()`

4. ChatAgent 使用 LLM 真 stream，不是先完整生成再切块。
   - `crates/app/src/agents/chat_agent.rs:80-107`

5. `ChatRequest` 默认 agent 已改为 `chat`。
   - `contracts/src/chat.rs:11-12`
   - `contracts/src/chat.rs:455-457`

6. memory runtime 下 Chat/general 不再因为 `pg().is_none()` 绕到 memory compat；当前 `TASK_MEMORY_MODE` 只为 memory-adapter RAG compat 保留。
   - `crates/app/src/chat/graphflow_tasks_core.rs:86-108`

### 剩余 gap

- 旧 `MainAgent::answer_general` / `answer_general_stream` 仍存在于 `crates/app/src/main_agent/mod.rs`，但当前未作为 Chat/general 生产主路径。建议后续 cleanup，不是当前 P0。
- `TASK_MEMORY_MODE` / `execute_memory_chat_compat` 仍存在，虽然当前主要是 memory RAG compat；后续如果要彻底单一路径，需要把它降级到测试/兼容明确边界。

### 评分

92/100。主路径已经达到验收线。

## 2. WebSearch Brave 验收

### 已完成

1. search crate 默认 provider 已改为 Brave LLM Context。
   - `crates/search/src/config.rs:11-17`

2. `SearchExecutor` 支持 `brave_llm_context` 主路径，Perplexity 作为 legacy provider 保留。
   - `crates/search/src/executor.rs:20-63`

3. Brave LLM Context provider 已实现：请求 Brave context endpoint，解析 `grounding` / `sources` 为 `SearchResult`。
   - `crates/search/src/provider.rs:14-38`
   - `crates/search/src/provider.rs:210-283`

4. `WebSearchAgent` streaming callback 未 await 的问题已修成 channel bridge。
   - `crates/app/src/agents/web_search_agent.rs:64-87`

5. Search non-stream 和 stream 都已走 `UnifiedAgentService -> WebSearchAgent`。
   - non-stream：`crates/app/src/chat/service_modes.rs:209-282`
   - stream：`crates/app/src/lib_impl/chat_streaming.rs:382-468`

6. Brave evidence 后接 answer LLM synthesis；stream 模式下 answer LLM 使用 `complete_stream` 发送 delta。
   - `crates/app/src/agents/web_search_agent.rs:89-118`
   - `crates/app/src/agents/web_search_agent.rs:262-305`

### 剩余 gap

P1：缺 live Brave LLM Context E2E。
- 当前 unit/parser/provider tests 与 WebSearchAgent fake LLM 回归测试已通过，但仍未证明真实 Brave API request/response shape 在当前 key/config 下跑通。

P1：Brave evidence fetch 本身不是 streaming。
- `stream_brave_llm_context` 只发 `Searching` 和 `SourcesCollected`，随后完整拿到 context；真正 token streaming 来自后续 answer LLM synthesis。
- 这可以接受，但需要在 trace/degrade/文档中明确“Brave context fetch buffered，answer synthesis stream”。

P1：fallback 会把 Brave evidence 文本作为 answer。
- 当 `answer_llm` 不可用或 synthesis 失败，`WebSearchAgent` 退回 `search_response.synthesized_answer`。
- 这比报错友好，但产品语义上仍是“证据列表”，不是合成答案；需要 UX/trace 明确标识。

已补回归测试：
- `search_answer_prompt_contains_evidence_and_citation_contract`
- `brave_answer_synthesis_streams_fake_llm_deltas_in_order`
- `search_stream_updates_are_emitted_to_sink`
- `avrag-search` provider/default/missing-key/parser tests

### 评分

83/100。主路径、answer synthesis、streaming event order 已有回归覆盖；剩余扣分主要来自缺 live Brave smoke 和 degrade 语义说明。

## 3. RAG plan v2 core 验收

### 已完成

1. `ExecutePlanRequest` 已加 schema guard。
   - `#[serde(deny_unknown_fields)]`：`crates/common/src/rag_execute.rs:158-160`
   - validate 规则：`crates/common/src/rag_execute.rs:210-275`

2. validate 已覆盖：
   - `doc_scope` 非空
   - `items` 非空
   - 最多 4 个 item
   - item payload exactly one：`query` 或 `bm25_terms`
   - priority 必须 0..=1
   - budget 不能为 0
   - placeholder triplet 最多两个 placeholder
   - graph budget > 0 时必须有 `graph_hints` 或 `placeholder_triplets`

3. original query 注入 text dense 已落到 common helper，并在 GraphFlow 与 streaming RAG 都调用。
   - helper：`crates/common/src/rag_execute.rs:278-302`
   - GraphFlow non-stream：`crates/app/src/chat/graphflow_tasks_rag.rs:140-145`
   - streaming RAG：`crates/app/src/lib_impl/chat_streaming.rs:514-520`

4. raw planner output 先 validate，再 normalize。
   - `crates/app/src/main_agent/mod.rs:716-742`
   - 已有测试：`parse_rag_plan_rejects_raw_invalid_payload_before_normalize`、`parse_rag_plan_rejects_raw_doc_scope_mismatch_before_normalize`

5. graph retrieval 已收敛到结构化 hints/triplets。
   - placeholder 两空位映射：`crates/rag-core/src/runtime/execute.rs:254-268`
   - 无 structured graph input 时跳过 graph 并记录 degrade：`crates/rag-core/src/runtime/execute.rs:522-560`

6. `RagAgent` 不再返回 fake placeholder success，改为 fail-closed。
   - `crates/app/src/agents/rag_agent.rs:6-58`

### 剩余 gap

P1：RAG production answer 仍由 MainAgent planner/answer 承载。
- GraphFlow RAG synthesis 仍调用：
  - `answer_rag_with_main_agent(...)`
  - `MainAgent::build_rag_chat_response(...)`
- 证据：`crates/app/src/chat/graphflow_tasks_rag.rs:254-273`
- streaming RAG 也仍调用 MainAgent answer stream/build response。
  - `crates/app/src/lib_impl/chat_streaming.rs:596-655`

这不影响本轮 “rag-plan-v2-core” 的 correctness，但说明“RAG 作为 UnifiedAgentService 内的真实 RagAgent”还没完成。

P1：`RagAgent` 仍不是生产 RAG adapter。
- `UnifiedAgentService` 能路由 `AgentKind::Rag`，但 `RagAgent` 目前只 fail-closed。
- 这是比 fake success 更安全的阶段态，但还不是最终架构。

P2：Minsky 级 RAG E2E artifact 已补。
- 报告：`.hermes/reports/2026-04-30_140000-minsky-rag-e2e-agent-mechanisms.md`
- 运行记录：`.hermes/runs/e2e-minsky-agent-mech-1777528419-current.json`
- E2E 结论：文档实际是 Hyman Minsky《Stabilizing an Unstable Economy》，不是 Marvin Minsky《Society of Mind》；系统最终正确给出 evidence-boundary 拒答。
- Planner 本次只产出单 semantic query + `query_entities`，没有 `bm25_terms` / `graph_hints` / `placeholder_triplets`，因此 text dense 与 multimodal dense 有召回，BM25 与 graph-only 为 0。

### 评分

86/100。RAG v2 core + Minsky E2E artifact 基本达标；RAG agent 架构仍未完全收口。

## 4. 验证结果

本轮最终复验通过：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app
cargo test -p transport-http
cargo test -p common --test rag_execute_contract -- --nocapture
cargo test -p avrag-rag-core
cargo test -p avrag-search
git diff --check
python3 -m py_compile .hermes/scripts/e2e_minsky_agent_mechanisms.py \
  .hermes/scripts/continue_minsky_agent_mechanisms.py \
  .hermes/scripts/live_rag_e2e_minsky86.py
```

结果：

- `cargo test -p app`：通过，74 个 lib tests + integration/doc tests 通过。
- `cargo test -p transport-http`：通过，25 个 unit tests + `chat_stream_contract` / `rag_execute_plan_contract` / router/module tests。
- `cargo test -p common --test rag_execute_contract -- --nocapture`：通过，16 个 tests。
- `cargo test -p avrag-rag-core`：通过，25 个 tests。
- `cargo test -p avrag-search`：通过，21 个 unit tests + module surface，1 个 live Brave smoke 因需要真实 `SEARCH_API_KEY` 被 ignore。
- `git diff --check`：通过。
- `.hermes/scripts` 三个 Python E2E 脚本 `py_compile`：通过。

前端最终复验通过：

```bash
cd /home/chuan/context-osv6/frontend_next
pnpm test tests/workspace/stream.test.ts
pnpm test tests/workspace/ui-store.test.ts tests/workspace/workspace-chat-pane.test.tsx tests/workspace/workspace-right-rail.test.tsx
pnpm typecheck
```

结果：

- `stream.test.ts`：3 tests passed。
- `ui-store.test.ts` + `workspace-chat-pane.test.tsx` + `workspace-right-rail.test.tsx`：29 tests passed。
- `pnpm typecheck`：通过。

## 5. 安全与 diff hygiene

已检查：

- 前端 stream/ui/typecheck 本轮已通过。
- added-line secret scan 未发现真实 hardcoded secret；`.env.example` 中 `***` 占位和代码中的 config lookup 不按泄密计。
- 已移除/替换 `.hermes/scripts` 中的固定 E2E 测试密码字面量，改为环境变量或动态测试密码。
- `git diff --check` 通过。

风险：

- 当前 tracked changed 文件数：44。
- 当前 status entries：87。
- untracked entries：40。
- diff stat：2466 insertions / 744 deletions。
- `crates/ingestion/src/parser/mineru.rs` 单文件 1028 行改动，明显超出 Agent 主路径迁移范围，建议拆出或确认来源。
- `.hermes/runs`、`.hermes/reports`、`.hermes/scripts`、`.kilo/plans` 中有大量 untracked artifacts；不要无筛选 commit。

## 6. 建议修复顺序

1. 先清理/拆分 diff：
   - Agent migration 一组；
   - WebSearch Brave 一组；
   - RAG v2 contract/runtime 一组；
   - 前端 contract/UI 一组；
   - mineru/ingestion 大改单独处理；
   - `.hermes/runs` 和临时 E2E artifacts 默认不提交。

2. WebSearch 剩余验收：
   - live Brave LLM Context smoke test，如果 key/config 可用；
   - 明确记录 Brave context fetch buffered、answer synthesis streaming；
   - fallback/degrade 时在 trace/UX 中区分“证据列表”与“合成答案”。

3. RAG 剩余验收与架构收口：
   - Minsky query->planner->retrieval->answer E2E 已补，后续应把它固化为可重复 smoke 或 nightly；
   - 决定 `RagAgent` 是否作为 GraphFlow RAG adapter；
   - 把 MainAgent planner/answer 拆成 RAG-specific planner/synthesizer 或 agent stage；
   - 删除/降级 MainAgent legacy general/search 代码。

4. 前端已重跑通过，后续如继续改 streaming contract，再重跑：
   - stream parser tests；
   - UI mode tests；
   - chat pane/right rail tests；
   - `pnpm typecheck`。

## 最终判断

Kilo 这轮可以评为“实现质量不错、主路径推进明显、关键回归验证已补，但仍不是干净可合并状态”。

如果按开发 checkpoint：84/100。
如果按生产 merge gate：76/100。
主要扣分来自：diff 范围过大、缺 live Brave smoke、RAG agent 架构仍未完全收口、临时 artifact 未清理。