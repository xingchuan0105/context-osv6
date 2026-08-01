# 问答场景缓存与压缩补齐（①-⑤）验收指示

> 本文件由落地方生成，供**另一个 agent 窗口**独立验收。验收者没有本会话上下文，
> 请按下列步骤逐项执行；每项给出 PASS/FAIL 与证据，最后汇总结论。
>
> **验收边界**：以**静态检查 + 编译 + 单元测试 + 代码审查 + 纪律合规**为主；
> 不要求真实 LLM 调用（`responses_live` 等 `#[ignore]` 集成测试需要 live API key，
> 属可选；如你判断必须跑，先征得用户同意）。

## 0. 验收对象

- **提交**：`1b2407e7` — `feat(llm,cache): RAG/chat/websearch 问答场景缓存与压缩补齐（①-⑤）`
  （40 文件，+1340/-378）
- **前置提交（先于本次存在，验收时已在历史中）**：`e488c406`（OpenAI Responses 协议）、
  `55a6696a`（ProviderPool routing 层，另一窗口的工作）
- **目的**：针对 RAG/chat/websearch 问答场景，补齐与 Reasonix 对齐的缓存/压缩能力：
  ① tool result 剪裁 + synthesis 重放收敛；② ingestion 结果级缓存；③ session summary
  历史压缩；④ websearch 搜索结果缓存；⑤ reasoning tokens 入账。
- **当前状态声明**：实现完成、编译通过、单元测试通过；未做真实 E2E 验收。
- **相关纪律**：仓库 `AGENTS.md`（prompts-in-md、solo 本地 trunk、graphify update、
  不混入他人未提交工作）。

## 1. 提交完整性检查

```bash
cd /home/chuan/context-osv6
git log --oneline -1 1b2407e7
git show --name-only --format= 1b2407e7 | wc -l
git show --name-only --format= 1b2407e7 | grep -E '^(avrag-rs/crates/(llm|agent-loop|rag-core-ports|search|app-bootstrap|app-billing|app-core|billing|app-chat|struct-supervision)|avrag-rs/bins/worker|avrag-rs/prompts/pipeline/session-summary|avrag-rs/migrations/0062)' | wc -l
```

**预期**：第一条显示 `feat(llm,cache): RAG/chat/websearch 问答场景缓存与压缩补齐（①-⑤）`；
文件数 = **40**；第三条 = 40（全部在预期路径内，无越界）。

**PASS 条件**：40 个文件且全部落在上述路径范围内。
注意：工作区可能有**既有的未提交开发改动**（`git status` 大量 M 文件，含另一窗口的
ProviderPool 后续迭代）——验收者只确认 **1b2407e7 本身**，不要被工作区噪音干扰。

## 2. 全量编译（0 error 门禁）

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo check --workspace 2>&1 | grep -cE '^error'
```

**预期**：`0`。

**PASS 条件**：输出 `0`。若 >0：把 error 列表贴进结论并 FAIL（不要自行修改）。

## 3. 单元测试（三 crate 门禁）

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p avrag-llm --lib 2>&1 | tail -1
cargo test -p agent-loop --lib 2>&1 | tail -1
```

**预期**：
- `avrag-llm`: `test result: ok. 107 passed; 0 failed; 1 ignored`
- `agent-loop`: `test result: ok. 276 passed; 0 failed; 0 ignored`

**PASS 条件**：两条 `ok` 且无 failed。

**可选补充**（app-chat 有 1 个已知外部失败，验收豁免）：
```bash
cargo test -p app-chat --lib 2>&1 | tail -1
```
预期 `231 passed; 1 failed`——唯一失败 `chat::pipeline_tests::tests::inject_assembled_metadata_dual_roundtrips_mode_config`
（`pipeline_tests.rs:307`，`system_prompt_parts` 数量断言）由**外部未提交的
`mode_assemble.rs` 改动**引起，与本次提交无关（本次未触碰 mode_assemble/pipeline_tests）。
验收者确认失败名与该说明一致即可，不算本次 FAIL。

## 4. 功能单元测试针对性确认

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p avrag-llm --lib completion_cache 2>&1 | tail -1
cargo test -p agent-loop --lib trim_json message_format 2>&1 | tail -1
cargo test -p agent-loop --lib synthesis 2>&1 | tail -1
cargo test -p avrag-llm --lib openai_responses 2>&1 | tail -1
```

**预期**：四条全部 `ok`。分别覆盖：
- `completion_cache`：roundtrip / key 区分（model/prompt_version/messages 任一不同即 miss）/ kill switch 禁用
- `trim_json / message_format`：结构化裁剪保持 JSON 有效、字节预算、tool 消息 24k 上限
- `synthesis`：重放去重（同 tool+data 只留一条）、48k 总预算、最新优先
- `openai_responses`：非流式/流式/工具调用解析、`reasoning_tokens` 断言（=5）

**PASS 条件**：四条 `ok`，且 `openai_responses` 输出中包含
`non_streaming_response_absorbs_text_usage_and_tool_call ... ok`（其断言
`response.usage.reasoning_tokens == 5`，验证 ⑤ 的协议层提取）。

## 5. 代码审查点（逐项核验，给证据）

### ① tool result 剪裁 + synthesis 重放收敛
- `crates/agent-loop/src/react_loop/message_format.rs`：
  - `trim_json_for_context` / `trim_json_inner`：预算按**字节**（`to_string().len()`）；
    String 分支 `floor_char_boundary` 截断；Object 分支超预算时**保底保留序列化最大的字段**
    （fallback 逻辑），Array 分支保底保留首项——不会返回空容器。
  - `build_tool_message` 对 `data` 做 `TOOL_MESSAGE_MAX_CHARS = 24_000` 裁剪。
- `crates/agent-loop/src/react_loop/synthesis.rs`：
  - `trim_tool_results_for_synthesis`：`(tool, data_json)` 去重、从最新往旧保留、
    `SYNTHESIS_TOOL_RESULTS_MAX_CHARS = 48_000` 总预算；**超预算且 kept 为空时保底 push
    最新一条**（`if kept.is_empty() { kept.push(entry); }`）——证据集永不为 `[]`。

### ② ingestion 结果级缓存
- `crates/llm/src/completion_cache.rs`：
  - key = `llm_result:v1:{sha256(model || prompt_version || messages_json)}`，
    `completion_cache_key` 返回 `Option`（序列化失败则跳过缓存，不塌缩到单 key）；
    TTL 7 天；`INGESTION_LLM_RESULT_CACHE=0/false` kill switch。
  - 命中返回 `LlmUsage::zeroed()`——下游 `merge_usage`/`accumulate` 是 saturating_add，
    0 累加无副作用（已验证）。
- 接入点：`section_index.rs` / `summary.rs` 的 `complete_cached`（温度 0.1/0.3/0.2 固定）；
  `bins/worker/src/pipeline/triplet_extraction.rs` 的 `complete_triplet_extraction`
  （cache 参数，worker `LlmDeps.completion_cache` 与 document lock 共享 Redis 连接）。

### ③ session summary
- `crates/app-chat/src/agent_runtime.rs` `resolve_agent_messages`：历史 user 消息
  > `MAX_PROMPT_HISTORY_TURNS`(2) 时，超窗部分调 `summarize_older_turns`
  （MEMORY_LLM `memory_client()`）压缩为摘要，注入为置顶 user 消息
  `[早前对话摘要]\n...`；`memory_client()` 为 None 或调用失败 → `ok()?` 静默降级
  为原 2 条截断行为（与旧行为逐字节一致）。
- `prompts/pipeline/session-summary.system.md`：**LLM 提示词在 prompts/ 下**，代码只
  `include_str!`——符合 prompts-in-md 纪律。

### ④ websearch 搜索结果缓存
- `crates/search/src/executor.rs`：`SearchExecutor.cache: Option<Arc<dyn CachePort>>` +
  `with_cache`；`execute_search` 按 `search:brave:v1:[vertical:]query` 查/存，
  `SEARCH_CACHE_TTL_SECS = 30*60`；命中反序列化后清 `llm_usage = None`。
- `crates/app-bootstrap/src/lib.rs`：bootstrap 注入 `cache_store`（Redis 未配置时
  不注入，executor 无缓存，不 panic）。

### ⑤ reasoning tokens 入账
- `crates/rag-core-ports/src/llm_types.rs`：`LlmUsage.reasoning_tokens: u32`
  （`#[serde(default)]`，向后兼容反序列化）。
- 协议层：仅 `openai_responses/types.rs::to_llm_usage` 真实提取
  `output_tokens_details.reasoning_tokens`；openai_chat/gemini/anthropic 恒 0
  （**已知限制**：o1/o3 `thoughts`、Gemini `thoughtsTokenCount` 未提取，验收记为
  已知缺口即可，非本次 FAIL）。
- 链路：`LlmUsage` → `client/mod.rs::record_completion_success(reasoning_tokens)` →
  `ChatUsageRecord` → `UsageLimitUsageRecord` → `pg_usage_limit_store.rs` INSERT
  （19 列 / $1..$19 / 19 个 bind，列序对齐）→ `llm_usage_events.reasoning_tokens`。
- `migrations/0062_reasoning_tokens.{up,down}.sql`：up/down 对称
  （ADD/DROP COLUMN IF EXISTS，`BIGINT NOT NULL DEFAULT 0`）。

## 6. 纪律合规检查

```bash
cd /home/chuan/context-osv6
git show 1b2407e7 --stat | grep -E 'graphify-out|\.env' || echo "无 graphify-out / .env 混入"
ls avrag-rs/graphify-out 2>/dev/null | head -1 || echo "graphify-out 未入库（或已在 .gitignore）"
```

**预期**：`1b2407e7` 不含 `graphify-out/`、不含 `.env`。
提示：结构改动后 `graphify update .` 已在本会话执行（graph 已重建），验收者无需重跑；
如需重跑确认：`cd /home/chuan/context-osv6 && graphify update .`（可选，耗时约 2-3 分钟）。

## 7. 结论模板

```
## 验收结论
- 提交完整性：PASS/FAIL（证据：文件数、路径清单）
- 编译门禁：PASS/FAIL（证据：cargo check --workspace 输出）
- 单元测试门禁：PASS/FAIL（证据：avrag-llm / agent-loop 输出）
- 功能测试针对性：PASS/FAIL（证据：四条 cargo test 输出）
- 代码审查 ①-⑤：PASS/FAIL（证据：每项文件:行核验）
- 纪律合规：PASS/FAIL（证据：git show --stat）
- 已知限制（不计 FAIL）：app-chat 外部失败 1 个；reasoning_tokens 仅 responses 协议提取
- 总体：通过 / 不通过（列出阻塞项）
```
