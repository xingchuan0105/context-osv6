# 全量黄金集回归诊断报告（2026-08-02）

> 触发：prompt 体系重构（WP1-WP5，commit 707004a7..25f7da0b）验收通过后，用户要求跑一轮全量黄金集（realistic_corpus_full_eval，149 题）。
> 性质：重构后首次真实 LLM 全量跑，暴露系统性回归；已定位根因、修复（commit 6b1ea710）并经 4 题探针验证。
> 复核要点：§1 症状、§2 根因证据链、§3 修复、§4 验证、§5 遗留与未决。

---

## 1. 症状

### 1.1 运行配置

- 用例：`realistic_corpus_full_eval`（`crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs:992`）
- corpus：`crates/app/tests/rag_quality/golden_set_realistic.json`，21 个 subset 合计 **149 题**
- 模型：deepseek-v4-flash（判分 judge 同模型）
- 命令：
  ```bash
  cd avrag-rs && set -a && source .env && set +a
  export E2E_MODE=nightly RAG_EVAL_V2=1 RAG_EVAL_V2_ONLY=1
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
    --features product-e2e -- --ignored --test-threads=1 --nocapture
  ```
- harness 路径：`ctx.chat(query, workspace_id, &doc_scope)`（`rag_quality_prod.rs:797`）→ `test_context/http.rs:232` `post_rag_chat` → POST 真实 `/api/v1/chat`（agent_type=rag, stream=false）→ 真实产品 agent 路径（`mode_debug.agent_kind=chat`）。**模型收到完整装配上下文，无 harness 捷径。**

### 1.2 失败模式（前 18 题全挂后中断）

- 全部 `eval_bridge_miss` / `RETRIEVAL_MISS`，`chunks=0`，v2 judge correctness 多为 0。
- `eval_bridge_miss` 判定（`rag_quality_prod.rs:1449-1471`）：rag 且 `expected_should_answer` 且 `source_chunks` 非空时，`chat.tool_results` 必须含 status=Ok 且 tool ∈ `RETRIEVAL_TOOLS`（dense_retrieval/lexical_retrieval/graph_retrieval/index_lookup/doc_grep/doc_read_lines/doc_summary/struct_query）的检索层 tool_result，否则 FAIL。
- **模型每题的 tool_calls 是原生工具名而非 `<code>` 块**：`conversation_history_load`×N、`user_profile_load`、`invoke_skill`、`invoke_kb`、`invoke_retrieval`、`invoke_code`、`invoke_unknown`、`__knowledge_base__search`×5、`noop`；q3 写了 4 个 `code` 调用。
- `tool_trace`（q003.json）全部 status=NotImplemented：`[invoke_skill, code, code, code, code]`；`activity_counts={accept:understand:1, budget_exhausted:1}`。
- q1 答案自述「本轮执行额度已用尽」拒答；q3 答案自述「沙箱返回了执行环境错误」。
- **结论性症状：模型完全不使用 `<code language="python">` codegen 协议，检索从未发生。**

---

## 2. 根因证据链

### 2.1 排除的假设

| 假设 | 结论 | 证据 |
|------|------|------|
| harness 不走真实产品路径 | **排除** | `ctx.chat` → `post_rag_chat` → POST 真实 `/api/v1/chat`，`agent_kind=chat`；模型确实收到完整装配上下文 |
| `<code>` 块解析丢失 | **排除** | `parse.rs` 顺序：①`tool_calls` 非空→NativeToolCalls；②`<code>`→CodeBlocks（codegen）；③```围栏→CodeBlocks；④Content。codegen 块记录 tool="code_gen"。tool_trace 里的 `code` NotImplemented 来自 **NativeToolCalls 路径**（dispatch_tool → ToolCatalog 未命中 → NotImplemented "unknown tool"），不是 codegen |
| 提示词本身缺 codegen 教学 | **排除** | `agent-base.md`（v1.3）无条件段：唯一执行入口 `<code language="python">`、首块约束、并行扇出 + gather 示例、基础原语、证据判定，均正确；round-0 披露测试绿 |
| 模型/LLM 能力问题 | **排除** | 重构前 v2 基线同模型 recall=1.0（见 2.3） |

### 2.2 根因链（重构引入）

1. WP2 实现 D8：memory cluster 改为**全模式 mandatory 每轮披露**。
2. `assembler.rs`（重构前即存在）在 memory 披露时把两个原生记忆工具 `conversation_history_load`/`user_profile_load` 附加进检索轮工具列表。
3. 由于 memory 现在每轮披露 → **round 0 模型即收到 `tools=[conversation_history_load, user_profile_load]`**。重构前 memory 仅在 skill_request 时披露 → round 0 `tools=[]`。
4. deepseek-v4-flash 是函数调用模型：看到 tools schema 即走原生 `tool_calls` 路径，不再写 `<code>` 块。
5. 模型发明的工具名（`invoke_skill`/`code`/`__knowledge_base__search`…）在 `tool_registry.rs:138-146` dispatch_tool → ToolCatalog 未命中 → 全部 NotImplemented → 烧光预算 → `budget_exhausted` 拒答。
6. 检索从未发生 → `chunks=0` → `eval_bridge_miss`/`RETRIEVAL_MISS` 全挂。

关键代码位置：
- `agent-loop/src/react_loop/assembler.rs`（原 :91-103 附加 memory 工具，现已删除）
- `agent-loop/src/react_loop/policy/config/mode_loader.rs:95-103` `tools_for_retrieve`（tool_pool 查询）
- `agent-loop/src/react_loop/iteration/assemble.rs:106` `complete_with_tools(&round_messages, &assembled.tools, ...)` → llm `build_llm_request.with_tools`（真正发给 provider）
- `agent-tools/src/tool_registry.rs:138-146` dispatch_tool NotImplemented

### 2.3 重构前基线（关键对照）

v2 判分产物 `crates/app/tests/e2e_output/rag_eval_v2/v2_20260801-153129/`（17 行）与 `v2_20260801-154331/`（15 行），均早于 08-02 重构 commit：

- thesis_factual/numeric/adversarial、cross_adr、ipd_table、baiyao_pdf、cross_document、rag_search_joint、rag_codegen_channels、memory_coreference、option_d_pure_chat/dual_source 等**大部分 PASS，recall=1.0，correctness=1.0**，检索正常。
- 仅零星 2 SELECTION_MISS / 2 RETRIEVAL_MISS / 1 PARTIAL。

**结论：重构前同模型（deepseek-v4-flash）SaC codegen 检索工作正常 → 本轮全挂是重构引入的真实回归，非模型问题。**

---

## 3. 修复（commit 6b1ea710）

`agent-loop/src/react_loop/assembler.rs`：

- `assemble_retrieve` 的 tools 计算删除 memory 披露时的原生工具 merge（原条件 `memory_cluster_disclosed(disclosed)`），改为恒定：
  ```rust
  let tools = mode.tools_for_retrieve(registry);  // rag/search tool_pool 空 → tools=[]
  ```
- 删除被孤立的私有函数 `memory_cluster_disclosed` 与 `dedupe_tools`。
- 更新 3 个相关单测（`rag_retrieve_tools_always_from_tool_pool_only` / `rag_round_zero_discloses_codegen_bundle` tools 空断言 / `rag_retrieve_stays_tool_free_after_memory_skill_request`）。
- 注释说明 D8 自洽：memory 每轮散文披露、教学 `client.history`/`client.user_profile` 沙箱基础原语；原生记忆工具是 legacy 点选式残留。

保留的合法引用（未动）：
- `deps.rs:28/390/403/412/422/430`：SacHostBridge 的 `call_history`/`call_user_profile` = 沙箱 `client.history`/`client.user_profile` 路径。
- `progress/mod.rs:303` 与 `labels.rs:25`：进度标签防御性映射。

计划文档 `§8` 追加 WP7 偏差记录（#14 根因 / #15 修复 / #16 验证）。

---

## 4. 验证

### 4.1 单测

- `cargo test -p agent-loop --lib -- --test-threads=1` = **279 passed, 0 failed**。

### 4.2 真实 LLM 探针（thesis_factual Q1-Q4，E2E_QUESTIONS=1,2,3,4）

| Q | 结果 | 证据 |
|---|------|------|
| 速冻机使食品的中心温度需要达到多少度？ | **PASS** | recall@15=100% (1/1), correctness=1, faithfulness=1, chunks=2 |
| Y冷冻设备公司的前身是在哪个城市成立的？ | eval_bridge_miss | 模型写出 code 块，但 `asyncio.run()` 与沙箱运行中事件循环冲突执行失败，tools=[] |
| Y冷冻设备公司的主要产品有哪两种速冻机？ | eval_bridge_miss | 写出 code 块 + sandbox_error=1 + synthesis_code_answer_repair/violation，tools=[] |
| （Q4） | **PASS** | recall@15=100% (1/1) |

- **检索模式已恢复**：2/4 PASS（recall=1.0），2 个 miss 已不再是「不写 code 块」的系统性失败，而是模型行为层波动（asyncio 误用、synthesis 格式），与重构前基线零星 miss 同量级。
- 后续请求的全量 149 后台跑被用户中断（推进至 2/149），未产生新结论。

### 4.3 fail6 子集探针（E2E_QUESTIONS=65,86,88,105,106,121，commit 382117bc 修复 asyncio 示例后）

| Q | subset | 结果 | 证据 |
|---|--------|------|------|
| 65 | consulting_factual | UNGROUNDED | recall=1.00, chunks=4, judge correctness=1（SELECTION_MISS）|
| 86 | ipd_table | **PASS** | recall=1.00, chunks=55（raw recall@15=0 但 v2 judge PASS）|
| 88 | ipd_table | **PASS** | recall=1.00, chunks=33 |
| 105 | cross_document | PARTIAL | recall=1.00, chunks=41, correctness=0.8（SYNTHESIS_CONTRACT）|
| 106 | cross_document | **PASS** | recall=0.50 (1/2), chunks=48 |
| 121 | rag_search_joint | **FAIL eval_bridge_miss** | tools=web_search×6 全 Ok（=沙箱 client.web 记录名）, web=23, doc=0, judge correctness=0.9（v2 label=PASS 但 harness 硬闸 FAIL）|

- **修复验证成立**：asyncio + tools 两修复后，6/6 都发生了真实检索/判分；5 题 judge PASS 或 PARTIAL。
- **q121 定性（第三个问题，判定为模型取舍 miss 而非回归）**：dual 模式下模型全程用沙箱 `client.web`（act:search_web=46，纯 web 高质答案 correctness=0.9），但从未调用 `client.dense`（doc=0）——联合题本应同时取文内证据（golden doc 侧 source_chunks 非空），harness 因此按 RAG 侧无检索结果硬判 FAIL。与重构前基线 rag_search_joint 子集同样出现过 RETRIEVAL_MISS（1 次）一致，属模型对联合题的取舍波动，非确定性回归。

---

## 5. 遗留与未决

1. **全量 149 未重跑**：修复后的完整黄金集数字待跑（预计 ~2h）。fail6 子集（含跨子集代表性：consulting_factual/ipd_table/cross_document/rag_search_joint）已验证修复成立；建议先跑 `E2E_QUESTIONS=116,117,118,119,120,121`（rag_search_joint 全子集）确认 q121 频度，再全量。
2. **`asyncio.run()` 误用（q002）——WP3 引入的确定性失败模式（复核确认后从"可选教学补充"升级为必修，已修 commit 382117bc）**：agent-base.md v1.3 的 gather 示例写成 `async def main()` + `asyncio.run(main())`；而沙箱 wrapper（`code-interpreter/src/bridge.rs:170-176`）把用户代码缩进包进 `async def __avrag_main()` 再 `asyncio.run(__avrag_main())`——用户代码已在运行中的事件循环里，顶层 `await` 天然合法，`asyncio.run()` 必然报错。q002 的 answer（"上一轮因 asyncio.run() 与沙箱已有运行中的事件循环冲突而执行失败"）即模型照抄示例的结果：每次模仿基座示例首块必死、烧一轮预算自纠。**全量重跑前必须修**。
   - **已修（commit 382117bc）**：agent-base.md 示例改顶层 `await` 形态 + 新增观察式事实「沙箱在已启动的事件循环中执行代码块；异步调用直接写顶层 `await`（`asyncio.run()` 会与运行中的循环冲突）」；**KB SKILL.md:57-73 的 gather 示例同样含有 `async def main()` + `asyncio.run(main())`（复核方以为它已正确，实际 line 61 只是 `await` 在 `main()` 内），一并改顶层 `await`**；全 prompts 复查无 `asyncio.run(main())` 残留（唯一 `asyncio.run` 命中是教学句本身）。
3. **q003 synthesis_code_answer_repair**：模型产出含 code 的终答触发修复，待全量跑确认频度（归为待观察项）。
4. **q121 rag_search_joint 联合题模型取舍 miss（非回归）**：dual 模式模型全走 `client.web` 未调 `client.dense`（见 §4.3）。属模型行为波动，重构前基线同子集亦有此 miss；若全量跑频度偏高再考虑在教学层强化「联合题双源证据」观察式提示。

---

## 6. 复核自检表

- [x] 症状与我运行日志一致（前 18 题全挂、chunks=0、tool_trace NotImplemented）
- [x] 根因链每环有代码位置佐证（§2.2）
- [x] 重构前基线 recall=1.0 对照成立（§2.3）
- [x] 修复改动面最小、无行为外溢（§3）
- [x] 4 题探针结论（2 PASS / 2 模型波动）可复现 —— **复核修正：q002 不是模型波动，是 agent-base v1.3 示例（asyncio.run 形态）确定性误导，已在 §5.2 修正并修复；KB SKILL 同病一并修**
- [x] 单测 279 全绿
- [x] 修复 agent-base 示例后：全 prompts 无 `asyncio.run(main())` 残留（commit 6b1ea710 之后的第二个修复，commit 382117bc）
- [x] fail6 子集探针（65/86/88/105/106/121）检索恢复 6/6，5 题 judge PASS 或 PARTIAL；唯一硬失败 q121 为联合题模型取舍 miss（§4.3、§5.4），非回归
