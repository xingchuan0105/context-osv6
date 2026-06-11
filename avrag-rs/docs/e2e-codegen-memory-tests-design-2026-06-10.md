# Product E2E 测试补充设计 — 复杂 codegen 多工具 + 多轮指代记忆读取/回写

> 日期：2026-06-10
> 目标读者：在新窗口照此实现的工程师（实现前请先通读「§2 决策」与「§5 前置改造（P0）」）
> 关联背景：`avrag-rs/docs/prompts-memory-doc-profile-optimization-2026-06-10.md`（本轮记忆三层重构 + `doc_profile` 档案分流 + `resolved_query` 一等列）

---

## 1. 目标

在 product E2E 套件（`crates/app/tests/product_e2e/`）补两组测试：

1. **复杂查询的 codegen 多工具调用**：覆盖「档案 vs 正文分流」——round0 `doc_profile` 取章节/作者档案 → round1 `chunk_fetch` 读正文 → round2 合成。
2. **多轮对话指代消解的记忆读取与回写**：turn1 建立实体 → turn2 用代词追问 → 断言①检索用的是消解后 query、②`chat_messages.resolved_query` 被写入且 ≠ 原始 query、③按需记忆工具 `conversation_history_load` / `user_profile_load` 可被调用并返回真实数据。

两层都做：**smoke（mock LLM，确定性，PR 级）** + **llm_real（真实 LLM，`#[ignore]`）**。

---

## 2. 决策（已与需求方确认）

| 维度 | 决策 |
|------|------|
| 测试层级 | smoke + llm_real 各一组 |
| codegen 多工具形态 | 多轮 + 档案分流：round0 `doc_profile` → round1 `chunk_fetch` → round2 合成 |
| 记忆回写断言粒度 | DB 级：断言 `chat_messages.resolved_query` 被写入且 ≠ 原始 query；并断言检索使用消解后 query |
| 按需记忆工具 | 纳入：smoke 在 mock 注入 `conversation_history_load` / `user_profile_load` 的 tool_call，断言返回真实历史/画像 |
| fixtures | 复用现有 `antifragile.txt` / `lindy.txt`，不新建 |

---

## 3. 现状锚点（实现必读的事实）

### 3.1 测试分层与运行命令

- 入口：`crates/app/tests/product_e2e.rs` → `product_e2e/mod.rs`
- 子模块：`smoke/`（mock，确定性，真 PG + 本地对象存储）、`integration/`（真实基础设施，`--features integration`）、`llm_real/`（真实 LLM，`#[ignore]`）、`failure/`、`tenants/`
- smoke 运行：`cargo test -p app --test product_e2e product_e2e::smoke -- --nocapture`
- llm_real 运行（串行，需 `.env` 凭据）：
  `cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture`

### 3.2 mock LLM 路由机制（`product_e2e/mock_servers.rs`）

核心在 `mock_llm_handler`（约 L621）+ `mock_native_tool_call`（L260），按消息形态分支：

1. **原生工具轮**（`!tool_names.is_empty() && !has_tool_results`，L644）：调 `mock_native_tool_call` 返回 tool_call；**当前只识别 `web_search`，其余返回 `None`**。
2. **工具结果后**（`!tool_names.is_empty() && has_tool_results`，L668）：返回空内容 → 结束循环进合成。
3. **RAG codegen 轮**（`tool_names.is_empty() && system_prompt.contains("检索 → 评估 → 合成")`，L674）：
   - 消息中**还没有** `<code_execution_result>` → 返回 `mock_rag_codegen_response()`（**单个** `client.dense_search(...)`，见 `format_mock_rag_codegen_response` L72）。
   - 已有 `<code_execution_result>` → 返回空 → 进合成。
   - 即：**当前只支持「单轮 codegen → 合成」，不支持多轮 codegen**。
- codegen 注入参数的 setter：`set_mock_rag_codegen_query` / `set_mock_rag_codegen_chunk_id(s)` / `set_mock_rag_skip_codegen`（L43–65）。
- 路由可被 header `x-mock-route` 或 system_prompt 标记覆盖（L715+，`MockLlmRoute`）。

### 3.3 ⚠️ 关键交互（P0 风险）：tool_pool 非空已使 mock RAG 路由失配

本轮重构把 `modes/rag.yaml`、`modes/chat.yaml` 的 `tool_pool` 改为含 `conversation_history_load` / `user_profile_load`。证据：

- `config.rs:448 tools_for_retrieve` 在 `tool_pool` 非空时返回这些工具。
- `assembler.rs` 测试 `rag_round_zero_discloses_codegen_bundle` 断言 round0 **同时**含 codegen 提示（`dense_search`）**且** `ctx.tools.len() == 2`。
- `iteration.rs:199` 用 `complete_with_tools(&round_messages, &assembled.tools, ...)` 把 tools 发给 LLM。

后果：RAG retrieve round0 的请求里 `tools` 非空 → `mock_tool_names` 非空 → mock 进入「原生工具轮」分支（L644），`mock_native_tool_call` 对 RAG 返回 `None` → 不返回 codegen；而 codegen 分支门控 `tool_names.is_empty()` 此时为假 → 被跳过。**结果是 RAG 不再产出 codegen，`assert_codegen_bridge_dense_retrieval` 等现有 `rag_smoke` 断言会挂。**

> 实现第一步：先跑现有 `product_e2e::smoke::rag_smoke`，确认是否已红。无论红否，§5.1 的 mock 路由改造都是本设计的前置依赖。

### 3.4 bridge 方法 → 工具名 映射（`rag-core/src/runtime/bridge.rs`）

断言 `ChatResponse.tool_results[].tool` 时按下表（client 方法名 ≠ tool_results 里的 tool 名）：

| Python/bridge 方法 | `tool_results[].tool` |
|--------------------|------------------------|
| `dense_search` | `dense_retrieval` |
| `lexical_search` | `lexical_retrieval` |
| `graph_search` | `graph_retrieval` |
| `chunk_fetch` | `index_lookup` |
| `doc_summary` | `doc_summary` |
| `doc_profile` | `doc_profile` |

### 3.5 断言库与 DB 访问

- 断言库：`product_e2e/assertions.rs`。可复用：`assert_http_ok` / `assert_has_citations` / `assert_codegen_bridge_dense_retrieval` / `assert_citations_use_document_chunks` / `assert_answer_substantive`。
- bridge 工具断言范式：`resp.tool_results.iter().any(|r| r.tool == "X" && r.status == ToolStatus::Ok)`。
- **DB 直查范式**（`test_context.rs` 已有先例，约 L1083+）：

```rust
let pool = sqlx::PgPool::connect(&self.pg_url).await?;
let row: (i32,) = sqlx::query_as("SELECT chunk_count FROM documents WHERE id = $1")
    .bind(doc_uuid)
    .fetch_one(&pool)
    .await?;
```

→ 新增 `resolved_query` 查询助手即照此写（§5.4）。

### 3.6 记忆链路（被测对象）

- `resolved_query` 写入：`service_postprocess.rs` 从 `execution.query_resolution.resolved_query` 取值，经 `append_chat_turn(..., user_resolved_query)` 写入 `chat_messages.resolved_query`（migration `0040_chat_resolved_query`）。
- 按需工具：`conversation_history_load` / `user_profile_load`（`skills/builtin/conversation_history.rs`），实际查询在 `skills/memory_dispatch.rs`（`load_history_by_tags` / `get_user_profile`）。生产派发链 `iteration.rs:133 dispatch_skill_tool` → `dispatch_atomic_tool_with_enforcement` → `ExecutionContext::with_memory`。
- `doc_profile` 工具：`rag-core/src/runtime/tools/doc_profile.rs`，输出数组，每元素含 `doc_id` / `name` / `author` / `publication_date` / `domain` / ... / `sections[]`（`sections[].chunk_id` 可用于后续 `chunk_fetch`）。

---

## 4. 现有可复用资产

- `llm_real/multi_turn.rs::real_llm_multi_turn_rag_follow_up_remembers_context`：已有 turn1（antifragility）→ turn2（"Who wrote the book about it?"）续聊，但**只断言答案含 "taleb"**，未断言 `resolved_query` 回写、未触达按需工具。→ llm_real 用例在此扩展，不另起炉灶。
- `llm_real/mod.rs`：`chat_with_retry` / `chat_with_session_retry`（带重试，串行）、`new_with_real_llm()`、artifact 落盘助手。
- `smoke/rag_smoke.rs`：smoke RAG happy path 范式（上传 → 等 ingest → 查 chunk → chat → 断言）。

---

## 5. 前置改造（P0，测试用例的依赖）

### 5.1 修复 + 扩展 mock RAG 路由以支持「多工具非空 tool_pool + 多轮 codegen」

`mock_servers.rs::mock_llm_handler` 改造要点：

1. **codegen 优先于原生工具**：当 `system_prompt.contains("检索 → 评估 → 合成")`（RAG retrieve）时，**不论 `tool_names` 是否非空**，都走 codegen 分支；仅当 mock 被显式要求注入记忆工具调用时（见 5.2 toggle）才在该轮返回 tool_call。
   - 即把 codegen 分支门控从 `tool_names.is_empty()` 改为「是 RAG retrieve 提示 且 未被要求注入记忆工具」。
2. **多轮 codegen 驱动**：按消息中 `<code_execution_result>` 出现次数决定本轮 codegen 内容：
   - 出现 0 次 → round0 codegen（`doc_profile`）。
   - 出现 1 次 → round1 codegen（`chunk_fetch`，chunk_id 取自 §5.3 注入值）。
   - 出现 ≥2 次 → 返回空内容 → 进合成。
   - 用一个测试 toggle（如 `set_mock_rag_multiround_profile(true)`）开启此多轮脚本，**默认关闭**以不影响现有单轮 smoke。

辅助函数（新增）：

```rust
fn count_code_execution_results(messages: &[serde_json::Value]) -> usize {
    messages.iter().filter(|m| {
        m.get("content").and_then(|c| c.as_str())
            .is_some_and(|c| c.contains("<code_execution_result>"))
    }).count()
}

// round0：doc_profile 取档案（含 sections + chunk_id）
fn format_mock_rag_doc_profile_codegen(doc_id: &str) -> String { /* <code language="python"> profile = await client.doc_profile(doc_ids=[doc_id]) ; print(json.dumps(profile)) </code> */ }

// round1：chunk_fetch 取正文（chunk_id 来自注入）
fn format_mock_rag_chunk_fetch_codegen(chunk_id: &str) -> String { /* client.chunk_fetch(chunk_id=...) */ }
```

> 注意：`doc_profile` 在 bridge 的 `format_data` 里走 `"doc_summary" | "doc_profile" => json!({"chunks": data})`，与 dense 一样能被 sandbox stdout/bridge 捕获并进入 `tool_results`。

### 5.2 mock 注入按需记忆工具调用

扩展 `mock_native_tool_call`（或新增并联函数 + toggle）：

- 新增 toggle：`set_mock_emit_memory_tool(Some("conversation_history_load"))` / `Some("user_profile_load")`。
- 当 toggle 命中且 `tool_names` 含该工具名时，返回对应 tool_call：

```rust
json!({
  "id": "call_mem_0", "type": "function",
  "function": { "name": "conversation_history_load",
                "arguments": "{\"limit\":20}" }
})
// user_profile_load: "arguments": "{}"
```

- 该工具结果回灌后，下一轮按既有「has_tool_results → 空内容 → 合成」收尾即可。

### 5.3 mock codegen chunk_id / doc_id 注入

复用/扩展现有 setter，让 round0 `doc_profile` 的 `doc_id` 与 round1 `chunk_fetch` 的 `chunk_id` 取自测试用真实值：

- doc_id：测试已知（upload 返回）。
- chunk_id：测试通过 `ctx.query_document_chunk_ids(doc_id)` 拿到真实 chunk_id，注入 `set_mock_rag_codegen_chunk_id(...)`，round1 codegen 用它 `chunk_fetch`。

### 5.4 新增 DB 断言助手（`test_context.rs`）

```rust
impl TestContext {
    /// 读取某会话中最近一条 user 消息的 resolved_query（用于回写断言）。
    pub async fn query_latest_user_resolved_query(
        &self, session_id: &str,
    ) -> anyhow::Result<(String, Option<String>)> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let sid = uuid::Uuid::parse_str(session_id)?;
        let row: (String, Option<String>) = sqlx::query_as(
            "SELECT content, resolved_query FROM chat_messages \
             WHERE session_id = $1 AND role = 'user' \
             ORDER BY created_at DESC LIMIT 1"
        ).bind(sid).fetch_one(&pool).await?;
        Ok(row) // (原始 content, resolved_query)
    }
}
```

> 实现前确认 `chat_messages` 的列名（`session_id` / `role` / `content` / `created_at` / `resolved_query`）与实际 schema 一致（参考 `storage-pg/src/chat.rs` 与 migration 0040）。

---

## 6. 测试用例清单

### Smoke（mock，确定性）— 新增文件 `product_e2e/smoke/rag_codegen_multitool_smoke.rs`、`product_e2e/smoke/memory_multiturn_smoke.rs`，在 `smoke/mod.rs` 注册

**Case S1 — 多轮档案分流 codegen（doc_profile → chunk_fetch → 合成）**
1. 上传 `antifragile.txt`，等 ingest 完成，取 `doc_id` 与真实 `chunk_ids`。
2. `set_mock_rag_multiround_profile(true)` + `set_mock_rag_codegen_chunk_id(chunk_ids[0])`。
3. 复杂查询（如「这本书第 X 章讲了什么？」）走 RAG。
4. 断言：
   - `resp.tool_results` 同时含 `tool == "doc_profile"`（Ok）与 `tool == "index_lookup"`（Ok，即 chunk_fetch）。
   - `resp.degrade_trace.is_empty()`（happy path）。
   - `assert_has_citations` + `assert_citations_use_document_chunks(resp, &chunk_ids)`。
   - （可选）round 顺序：doc_profile 先于 index_lookup（按 tool_results 顺序或 trace）。

**Case S2 — 指代消解 + resolved_query DB 回写**
1. 上传文档；turn1：「What is antifragility?」（建立实体），记 `session_id`。
2. turn2（同 session）：「Who wrote the book about **it**?」。
3. 断言：
   - `query_latest_user_resolved_query(session_id)` 返回的 `resolved_query` 为 `Some(..)` 且 ≠ 原始 content（即代词被展开）。
   - turn2 答案非空、无 degrade。
   - （强化）检索用的是消解后 query：可断言 mock 收到的 `dense_search`/`doc_profile` query 含被消解实体（通过 mock 记录的 query cell，或 trace `prompt_snapshot`）。

> mock 侧需保证 Pre-Loop 指代消解真实运行并写出 `query_resolution.resolved_query`；若 mock LLM 不产出消解，需为「指代消解 LLM 调用」加一条 mock 路由返回固定展开结果（确认消解走哪个 LLM client / 提示标记，再加路由）。

**Case S3 — 按需记忆工具读取**
1. 同 session 造 ≥2 轮历史。
2. `set_mock_emit_memory_tool(Some("conversation_history_load"))`，发一轮触发记忆工具的查询。
3. 断言：`resp.tool_results` 含 `tool == "conversation_history_load"` 且 `data.message_count > 0`、`data.messages` 非空（证明 `memory_dispatch` 真查到 PG 历史，而非空壳）。
4. 另一子用例：`Some("user_profile_load")` → 断言返回 `structured_profile` 等字段结构存在（无画像时为空对象也算通过结构断言；如要断言非空，需先在 PG 预置一条 user_profile）。

### llm_real（真实 LLM，`#[ignore]`，串行）— 扩展 `llm_real/multi_turn.rs`，必要时加 `llm_real/rag_real.rs`

**Case R1 — 复杂查询多工具（非确定性，宽松断言）**
- 用能触发多工具的复杂问题（如「先告诉我这本书作者和章节结构，再解释第二章的核心论点」）。
- 断言（宽松）：`resp.tool_results` 中**不同 tool 名 ≥ 2 个**且至少含一个检索类（`dense_retrieval`/`index_lookup`/`doc_profile` 任一）；答案 substantive；落 artifact 便于离线核查。
- 不强断言具体 tool 组合（真实 LLM 非确定）。

**Case R2 — 多轮指代 + resolved_query 回写**
- 在现有 `real_llm_multi_turn_rag_follow_up_remembers_context` 之上追加：turn2 后调用 `query_latest_user_resolved_query(session_id)`，断言 `resolved_query` 为 `Some` 且 ≠ 原文。
- 保留既有「答案含 Taleb」行为断言。

---

## 7. 新增 / 修改文件清单

**修改**
- `crates/app/tests/product_e2e/mock_servers.rs`：codegen 优先级修复 + 多轮 codegen 脚本 + 记忆工具注入 + 新 setter/toggle（§5.1–5.3）。
- `crates/app/tests/product_e2e/test_context.rs`：新增 `query_latest_user_resolved_query`（§5.4）。
- `crates/app/tests/product_e2e/smoke/mod.rs`：注册两个新 smoke 文件。
- `crates/app/tests/product_e2e/llm_real/multi_turn.rs`：追加 R1/R2 断言。

**新增**
- `crates/app/tests/product_e2e/smoke/rag_codegen_multitool_smoke.rs`（Case S1）。
- `crates/app/tests/product_e2e/smoke/memory_multiturn_smoke.rs`（Case S2 + S3）。

**可能修改（断言库）**
- `crates/app/tests/product_e2e/assertions.rs`：如需可加 `assert_tool_result_ok(resp, "doc_profile")`、`assert_distinct_tool_count(resp, >=2)` 等小助手（按 §3.5 范式）。

---

## 8. 验收标准

- smoke：`cargo test -p app --test product_e2e product_e2e::smoke -- --nocapture` 全绿（含**既有** `rag_smoke`，证明 §5.1 mock 修复未回归单轮路径）。
- 编译/clippy：`cargo check -p app --tests` 无错。
- llm_real（手动，需凭据）：`cargo test -p app --test product_e2e llm_real::multi_turn -- --ignored --test-threads=1 --nocapture` 通过并产出 artifact。

---

## 9. 风险与未决（实现时确认）

1. **mock RAG 路由失配（P0）**：§3.3 所述，tool_pool 非空使现有 codegen 门控失效。**先验证现有 rag_smoke 是否已红**，§5.1 修复同时服务于既有用例与新用例。
2. **指代消解在 smoke 下是否真实产出 `resolved_query`**：需确认 Pre-Loop 消解走哪个 LLM client 及其提示标记，给 mock 补一条「返回固定展开 query」的路由；否则 S2 的 DB 回写断言拿不到值。
3. **`user_profile_load` 真值断言**：画像由每日一次的 dream layer 产出，E2E 难自然触发。S3 若要断言非空画像，需在测试 setup 直接往 PG 预置一条 user_profile 行（确认表名/列）。
4. **多轮 codegen 的 round 上限**：确认 loop `max_iterations` 足够容纳 round0+round1+合成（mode yaml / 默认值），否则多轮脚本会被提前截断。
5. **chat_messages 列名**：§5.4 SQL 依赖实际 schema，落地前对照 migration 0040 与 `storage-pg/src/chat.rs`。
