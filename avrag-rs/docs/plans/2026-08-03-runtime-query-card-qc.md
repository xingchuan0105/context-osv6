# 2026-08-03 — 运行时动态质检：题型卡 + 分层埋点 + 证据闸 + salvage 重渲染

状态：已实施并验证（`cargo test -p agent-loop --lib` 全绿）。本文档归档计划与决策，供后续维护与回归归因。

## 决策（2026-08-02 讨论拍板）

1. 题型卡（query-card）由主 agent 填写，走 SDK 结构化 `complete_json_mode`（deepseek 真下发 `json_object`），禁自由文本多格式。
2. 不上语义裁判 / LLM judge；纯结构埋点（计数 Ok 回传）。
3. taxonomy v1 = 4+1 类：`calculation` / `rag_fact` / `table_count` / `chitchat` / `other`（serde 宽容解析，未知→`other`）。
4. 预算耗尽且无证据：放行 `DirectAnswer` + 宿主确定性追加披露行。
5. repair 再败：区分格式错 vs 真失败——有证据 → 证据池重渲染；无证据 → degraded 文案。

## 总体设计

```
  query → [L0 题型卡] pre-loop 一次 json_mode 调用（主 agent client）→ 校验/一次免费纠错轮
        → 卡注入 <query_card> 观察到每轮上下文
        → 检索 loop（L1.5/L2 已有观察不变）
        → DirectAnswer 接受点 [L2 证据闸 + L2.5 必做动作闸]：不过→第三人称观察+Continue（吃正常预算）
        → synthesis [L3]：S2 repair 失败→有证据=重渲染一遍 / 无证据=degraded-no-evidence
        → 预算耗尽且无证据：放行 + 宿主追加披露行
```

- 题型卡激活制：激活了没做到才拦，没激活不禁止；判错题型靠 149 回归监测。
- 全部新观察文案在 `prompts/loop/`（pipeline 卡 prompt 在 `prompts/pipeline/`），第三人称陈述，新标签先备案 `host_markers.rs`。
- 无证据披露行是宿主确定追加（user-visible copy，`prompts/loop/` 存文件），不交给模型写——防丢。

## Step 1：题型卡（query_card）机制

- 新增 `crates/agent-loop/src/react_loop/query_card.rs`：
  - `QueryCard { question_type: QuestionType, required_actions: Vec<String> }`，`QuestionType`：calculation/rag_fact/table_count/chitchat/`#[serde(other)] other`。放 agent-loop crate 内（不进 contracts、不加 typeshare）。
  - `validate()`：`required_actions` 过滤到 `contracts::sdk_primitives` 注册表 ∩ 当前 `mode.sdk_primitives` 已挂载（未挂载丢弃并记 telemetry）；未知动作丢弃。
  - `fetch_query_card(llm, mode, query) -> Option<QueryCard>`：`complete_json_mode` + system prompt `prompts/pipeline/query-card.system.md`；解析失败/必填缺失→一次「parse error 回贴」免费重试（heavytail 范式）；再失败→None（优雅降级）。
  - 调用点：`react_loop/mod.rs` `run_with_hooks`，prepare_run_request 后、run_retrieval_loop 前。usage 记 telemetry，不占迭代预算。
- `IterationState` 加 `query_card: Option<QueryCard>` 字段。
- 注入：`assembler.rs` `build_query_card_block` 渲染 `<query_card type=… required=…>`，`call_retrieve_llm` 尾部 user message（保 system 前缀稳定吃缓存）。
- `host_markers.rs` 登记 `<query_card>`（forbidden_in_final=true）；parity test 自动覆盖。

## Step 2：L2 证据闸 + L2.5 必做动作闸（DirectAnswer 接受点）

- 证据闸：`requires_evidence(mode)`（sdk_primitives 含 rag 组或 search 组）&& `!has_retrieval_observation(...)` && 预算未耗尽 → 不接受 DirectAnswer，注入 `prompts/loop/evidence-missing.nudge.md`，`IterationControl::Continue`，`exit_reason=evidence_missing_continue`。弹回吃正常迭代预算。
- 必做动作闸：`state.query_card.required_actions` 任一动作无对应 Ok ToolResult（按 tool 名匹配 alias 表）且预算未耗尽 → 同上，注入 `prompts/loop/required-action-missing.tmpl.md`（`{action}` 占位），`exit_reason=required_action_missing_continue`。
- 预算耗尽放行：`budget_exhausted = iteration+1 >= max_iterations`（就地判定，rounds-only；tokens 由 loop-top 处理）。耗尽时两闸都放行；证据闸放行时后续统一在 finish 路径追加披露。
- 边界（content_dispatch.rs 注释更新）：`require_evidence` 仍是 skill-owned；新闸是结构计数闸（零 Ok 回传）。

## Step 3：synthesis salvage 重渲染 + 真失败区分 + 披露行

- 穿参：`SynthesisPhase::run` 的 `tool_results` 传入 `run_prose_stream`；`has_evidence`（`has_retrieval_observation`）一并传入。
- 重渲染：repair 再败处分叉——
  - 有证据：第三遍 `stream_prose_to_sink`，messages = synthesis_messages + assistant(repaired) + `append_tool_results_observation` 重放 + user(`prompts/loop/synthesis-rerender.tmpl.md`，保留 SELECTED: #n / `[[web:n]]` 指引)。重渲染产出再过 `check_final_answer`；过→采用；再败→`contract_violation_fallback`。
  - 无证据：跳过重渲染，落 `degraded-no-evidence-{rag,search,default}.md`（重新启用现成 loader）。
- 披露行（决策 ④）：finish 路径，若 `requires_evidence(mode) && !has_evidence` → 最终 answer 末尾追加 `prompts/loop/evidence-missing-disclosure.md`（宿主确定追加，不经模型）。覆盖两条来路：预算耗尽放行的 DirectAnswer、无证据 synthesis。
- citations 无需重接线：`build_run_result` 对最终串自动重算。
- 流式：重渲染遍与现有一致走 stream；「静默缓冲+结尾校正」留作后续可选项，本期不做。

## Step 4：文案、文档、治理

- `prompts/loop/README.md` 文件表加 4 行；占位符登记 `{action}`。
- `prompts/pipeline/query-card.system.md`：题型定义第三人称事实化，taxonomy 只写抽象类。
- AGENTS.md stop-decision 表改写：宿主不跑语义完备性闸，但运行结构证据闸（零 Ok 回传 + rag/web 挂载 → DirectAnswer 不收，预算耗尽放行+披露）；`require_evidence` 语义仍归 skill。
- 本文件归档计划+决策。
- 可选项（默认不做）：`prompts/capabilities/knowledge-base/reference/how-to-read-tables.md` 误读对照加「问有多少个 X 且一名多行→数行」条目。

## Step 5：收尾验证

1. `cargo test -p agent-loop --lib`（核心，含 host_markers parity test）✅
2. `cargo test -p app-chat --lib`（caps/mode 组装面）
3. `bash scripts/test-l1.sh`（wave 末）
4. 真实 LLM 149 回归：用户决定何时跑（本期不跑）；跑后新增 `exit_reason`——`query_card_feedback`/`evidence_missing_continue`/`required_action_missing_continue`——可直接从 telemetry 归因埋点命中率。

## 明确不做

- 不上语义裁判 / LLM judge（决策②）。
- 不重开 native tool surface 做 tool_choice 强制填卡（D11 已关，侵入过大；json_mode 已满足「走 SDK 避免多格式」）。
- 不动 eval 侧 golden_set / 评分器（markitdown 重新 ingest 后再说）。
- 不改流式多遍直播行为（重渲染静默缓冲留作后续可选项）。
- 表格计数口径的 prompt 补强默认不动（见 Step 4 可选项）。

## 新 exit_reason / 观察标签一览

| exit_reason | 触发 | 观察文件 |
|-------------|------|----------|
| `evidence_missing_continue` | 证据闸（零 Ok 回传 + rag/web 挂载）| `prompts/loop/evidence-missing.nudge.md` |
| `required_action_missing_continue` | 必做动作闸 | `prompts/loop/required-action-missing.tmpl.md` |
| （synthesis 内部） | 有证据重渲染 / 无证据 degraded / 披露行 | `prompts/loop/synthesis-rerender.tmpl.md`、`degraded-no-evidence-*.md`、`evidence-missing-disclosure.md` |
