# Agent migration review 后续收口计划

生成时间：2026-04-30 16:47 CST
工作区：/home/chuan/context-osv6/avrag-rs

## 当前判断

这轮不是功能失败，而是 merge gate 还不干净。

已通过的正向证据：

- 后端：`cargo test -p app`、`cargo test -p transport-http`、`cargo test -p common --test rag_execute_contract -- --nocapture`、`cargo test -p avrag-rag-core`、`cargo test -p avrag-search` 均已通过。
- 前端：workspace stream/ui tests 与 `pnpm typecheck` 已通过。
- Minsky RAG E2E artifact 已补，且暴露了正确的 evidence boundary：当前 PDF 是 Hyman Minsky，不是 Society of Mind。
- WebSearch Brave answer synthesis 与 streaming event order 回归测试已补。
- added-line secret scan 没看到真实 hardcoded secret。

仍不建议直接整包 merge：

- dirty worktree 太大：44 tracked changed files、87 status entries、40 untracked entries、约 2466 insertions / 744 deletions。
- `.hermes` / `.kilo` / `.serena` artifacts 很多，需要筛选。
- `crates/ingestion/src/parser/mineru.rs` 大改明显不是 agent migration 主线，必须单独确认是否保留。
- RAG production answer 仍由 `MainAgent` planner/answer 承载，`RagAgent` 还不是生产 adapter。
- Brave live smoke 仍未跑，因为需要真实 `SEARCH_API_KEY`。

## P0：先处理 auth error contract 语义

### 问题

独立 reviewer 指出 `anonymous_share_chat_requires_login_without_persisting_owner_session` 断言 `error = "unauthorized"`，但 `middleware.rs` 里有一段看起来想让 `/api/v1/chat` 返回 `login_required`：

```rust
"error": if path == "/api/v1/chat" { "login_required" } else { "unauthorized" }
```

当前实测该测试通过，说明实际路由/中间件路径语义和 reviewer 静态判断不完全一致。最可能原因是 `/api/v1` nest 之后 middleware 看到的 path 是 `/chat`，因此走了 `unauthorized`。

这不是红测，但它是 contract 语义歧义：

- 如果产品希望“查看共享内容不用登录，但提问需要登录”的专门错误码，则应该返回 `login_required`，并修 middleware/test。
- 如果统一 401 都用 `unauthorized`，则应删除或改写 middleware 里的 `/api/v1/chat -> login_required` 死分支/误导分支。

### 推荐

我建议选择第一种：保留产品语义，修成 `login_required`。

最小步骤：

1. 写/改一个明确测试，验证匿名 share chat 对 `/api/v1/chat` 返回：
   - HTTP 401
   - `error = "login_required"`
   - message 包含 “asking questions requires sign-in” 之类语义
   - `chat_sessions` 不新增
2. 运行该测试，先看它红。
3. 修 `request_context_middleware` 的 path 判断，避免被 `nest("/api/v1", ...)` 改写后的 path 干扰。
   - 可选方案 A：在 outer router 层保留 original uri/path 后再判断。
   - 可选方案 B：把 nested `/chat` 且 body/source_type=share/source_token 存在的匿名请求视为 login_required。
   - 只做最小修复，不改无关 auth 逻辑。
4. 跑：
   - `cargo test -p transport-http anonymous_share_chat_requires_login_without_persisting_owner_session -- --nocapture`
   - `cargo test -p transport-http`

如果决定统一用 `unauthorized`，则改文案/分支/报告，不要保留误导性 `login_required` branch。

## P1：拆分 merge 范围

目标：不要整包提交。

建议拆成 5 组：

1. Agent service / Chat/general 主路径
   - `crates/app/src/agents/*`
   - `crates/app/src/chat/service_modes.rs`
   - `crates/app/src/lib_impl/chat_streaming.rs`
   - 相关 config/state 接线

2. WebSearch Brave
   - `crates/search/*`
   - `crates/app/src/agents/web_search_agent.rs`
   - WebSearch tests

3. RAG plan v2 contract/runtime
   - `crates/common/src/rag_execute.rs`
   - `crates/rag-core/src/runtime/*`
   - `crates/app/src/chat/graphflow_tasks_rag.rs`
   - contract tests

4. Frontend stream/contract UI
   - `contracts/src/chat.rs`
   - `frontend_next/lib/workspace/stream.ts`
   - `frontend_next/lib/workspace/ui-store.ts`
   - workspace pane/right rail/tests

5. 单独评估：MinerU / ingestion
   - `crates/ingestion/src/parser/mineru.rs`
   - 这组不要混进 agent migration，除非明确说明它是 Minsky E2E 的必要修复。

`.hermes/runs` 默认不要提交；`.hermes/reports` 可以保留精选报告；`.hermes/scripts` 只保留可复用脚本。

## P1：RAG agent 架构收口

当前 RAG v2 core 是可用的，但生产架构仍不是最终形态。

下一步不要再补 UI 小东西，直接做架构收口：

1. 定义 `RagAgent` 生产职责：
   - planner
   - execute plan
   - retrieval bundle
   - answer synthesizer
   - SSE event sink
2. 把 `MainAgent::build_rag_chat_response` / `answer_rag_with_main_agent_stream` 从 GraphFlow RAG 主路径摘出来。
3. GraphFlow 只负责编排和持久化，不继续依赖 MainAgent 作为 RAG answer executor。
4. 保留 `MainAgent` 的地方必须标明 legacy/test-only，避免双主路径。

验证命令：

```bash
cargo test -p app rag -- --nocapture
cargo test -p common --test rag_execute_contract -- --nocapture
cargo test -p avrag-rag-core
cargo test -p transport-http --test chat_stream_contract -- --nocapture
```

## P2：WebSearch live smoke

如果有真实 Brave key：

```bash
SEARCH_API_KEY=[REDACTED] cargo test -p avrag-search brave_llm_context_live_smoke_returns_grounding_sources -- --ignored --nocapture
```

如果没有 key，至少把当前状态明确写进验收报告：unit/parser/provider + fake LLM synthesis 已覆盖，live API shape 未验证。

## P2：更新报告与最后 gate

最后合并前再跑：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app
cargo test -p transport-http
cargo test -p common --test rag_execute_contract -- --nocapture
cargo test -p avrag-rag-core
cargo test -p avrag-search
git diff --check
python3 -m py_compile .hermes/scripts/e2e_minsky_agent_mechanisms.py .hermes/scripts/continue_minsky_agent_mechanisms.py .hermes/scripts/live_rag_e2e_minsky86.py

cd /home/chuan/context-osv6/frontend_next
pnpm test tests/workspace/stream.test.ts
pnpm test tests/workspace/ui-store.test.ts tests/workspace/workspace-chat-pane.test.tsx tests/workspace/workspace-right-rail.test.tsx
pnpm typecheck
```

再做 added-line secret scan。任何报告中出现真实 token/key/password/连接串都必须替换成 `[REDACTED]`。

## 推荐执行顺序

1. P0：定 auth error contract：`login_required` vs `unauthorized`，并修测试/代码一致。
2. P1：清理/拆分 dirty diff，尤其先把 `.hermes/runs` 和 `mineru.rs` 从 agent migration 主提交中拿出来。
3. P1：收口 RAG production path，让 `RagAgent` 真正承载 RAG answer，减少 MainAgent 残留。
4. P2：有 key 时跑 live Brave smoke；无 key则保留为已知 gap。
5. P2：最终全量验证 + secret scan + 更新验收报告。
