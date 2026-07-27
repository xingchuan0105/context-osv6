# Agent 主路径深化 — 开发文档（A 组整合）

> 来源：2026-06-09 架构 grilling（`/improve-codebase-architecture`），聚焦 main agent 路径 / ReAct loop / 渐进披露。
> 本文整合原 `disclosure-deepening-*.md` + `agent-mainpath-deepening-*.md` 两份，**按落地顺序重排**为单一开发文档。
> 关联（实现时对照）：
> - `CONTEXT.md` — 已加 ContextAssembler / Disclosure Phase / Disclosure Trigger / ClusterIndex·SkillBody 词条
> - `avrag-rs/docs/adr/0007-react-phased-context-disclosure.md` — 顶部「决策更新 2026-06-09」：round_idx → phase+trigger
> - B 组散点清理见 `cleanup-backlog-2026-06-09.md`（独立，勿混）
> **状态：✅ 已实现（2026-06-09 落地 + 2026-06-09 review 修复闭环）**

---

## 落地摘要（review 对照）

| 阶段 | 状态 | 关键交付 |
|------|------|----------|
| A2 skill_request | ✅ | `loop/skill_request.rs`；整段 JSON 协议；启发式全删；orchestrator 三件套对齐 |
| 候选 1 披露轴 | ✅ | `DisclosurePlanner` + `DisclosureRenderer`；删 `DisclosureConfig`/round 轴；`inject_retrieval_query` + `mandatory.retrieve` |
| A3 run_iteration | ✅ | `loop/iteration.rs`；`apply_llm_output` 可注入假 LLM；6 单轮 outcome 测试 |
| Review 修复 | ✅ | dep 渲染对齐；chat skill_request 护栏；tools_for_retrieve 注释/参数清理；synthesis hint 顺序恢复 |

**已知偏差（可接受 / 已文档化）**

- **JSON 嵌散文**：不支持（仅整段 trim 后合法 JSON）；与 orchestrator「纯 JSON 暗号」一致；有 `embedded_json_in_prose_is_unsupported` 测试。
- **skill_request 解析点**：仅在 `apply_llm_output` 写入 `last_skill_request`（1 处）；`assemble_retrieve` 只读 `disclosed.last_skill_request`。
- **DisclosureRenderer**：prompt 仍走 `PromptRegistry::standard_cached()` 全局单例（capability 已注入；prompt 注入待后续打磨）。
- **IterationOutcome**：sandbox 连续报错仍 `record: None` + 轮内遥测（保留旧行为）。
- **D2 范围外**：`mod.rs:174` loop query 选择、`loop_exit_for_mode` 等 `mode.id` 分支属 backlog，非本次披露 D2 欠债。

**验证**：`cargo test -p app --lib`（480 passed）；`cargo test -p app --test agent_catalog_contract --test unified_agent_contract` 全绿。

---

## 0. 总览

### 0.1 三项深化与依赖
| 阶段 | 候选 | 一句话 |
|---|---|---|
| 一 | **A2 — skill_request 协议** | 模型"要手册"改用单一权威 JSON 暗号，删掉连蒙带猜的启发式 |
| 二 | **候选 1 — 披露轴重构** | 废 `round_idx` 轴，改 phase+trigger；mode 特例下沉为数据；拆纯决策器 + 取件器 |
| 三 | **A3 — run god-method** | 抽「单轮 step」深模块，run() 收缩为编排；记账事件统一外发 |

依赖（决定顺序）：
- A2 产出的**干净请求清单** → 喂候选 1 的 `DisclosurePlanner`。
- A3 的 `run_iteration` 内通过 `apply_llm_output` **只写一次** `last_skill_request`。

> **落地顺序：A2 → 候选 1 → A3**（自底向上，每步行为等价、独立可验证）。

### 0.2 主要改动文件
- `avrag-rs/crates/app/src/agents/loop/config.rs`
- `avrag-rs/crates/app/src/agents/loop/assembler.rs`
- `avrag-rs/crates/app/src/agents/loop/mod.rs`
- `avrag-rs/crates/app/src/agents/loop/skill_request.rs`
- `avrag-rs/crates/app/src/agents/loop/disclosure_plan.rs`
- `avrag-rs/crates/app/src/agents/loop/iteration.rs`
- `avrag-rs/modes/{rag,search,chat}.yaml`
- `avrag-rs/prompts/orchestrators/*.md`（A2 暗号说明）

### 0.3 跨阶段注意
B 组「与 ADR-7 矛盾的陈旧契约测试」已处理：`chat_conversation_history_tools_in_catalog` 已删，`agent_catalog_contract` 通过。

---

## 阶段一 · A2 — `parse_skill_request` 脆弱协议 → 单一权威暗号

### 问题
LLM 请求能力簇正文（SKILL.md body）靠在自由文本里"暗示"，解析器叠了三层启发式（子串拼接 / 写死簇名 / 中英文短语）。

### 决策（固定暗号 + 直接删启发式）
- **单一权威协议**：assistant 在 content 中输出 `{"skill_request": ["<cluster_id>", ...]}`，解析器**只认整段 JSON**（trim 后 `serde_json::from_str`）。
- **删除**：子串拼接、写死簇名、中英文短语匹配——全部移除。
- **校验下沉**：`validate_skill_request` 用 `skill_catalog.cluster_by_id` 过滤。
- **orchestrator 提示词**：三份 `*.md` 已对齐 JSON 暗号。

### 接口
```rust
// loop/skill_request.rs
pub fn parse_skill_request(content: &str) -> Vec<String>;
pub fn validate_skill_request(mode: &ModeConfig, content: &str) -> Vec<String>;
pub fn is_skill_request_message(content: &str) -> bool;
```

### 测试
纯 JSON / 多 id / 未知 id / 无请求 / 畸形 / **嵌散文→空** / validate 过滤 / chat 护栏（`skill_request_json_in_chat_is_not_direct_answer`）。

---

## 阶段二 · 候选 1 — 披露轴重构（round_idx → phase + trigger）

### 决策
**D1** — phase+trigger 轴；删 `round_idx` / `DisclosureLoad`。

**D2** — 披露相关 mode 特例下沉：`inject_retrieval_query`、`mandatory.retrieve`；assembler 无 `mode.id==` 披露分支。

**D3** — `DisclosurePlanner`（纯）+ `DisclosureRenderer`（唯一 `DisclosedState` 变更点）。

### 实现注记
- `DisclosureSlice::MandatorySkillBody` 为 synthesis 强制项合理增项。
- `render_cluster_body` 依赖 skill 正文已对齐 `render_skill_body_with_deps`（review 顺修）。

---

## 阶段三 · A3 — `ReActLoop::run` god-method → 抽「单轮 step」

### 实现
- `loop/iteration.rs`：`run_iteration` + `apply_llm_output` + `dispatch_tool_call`
- `IterationState` / `IterationControl` / `IterationOutcome`
- `run()` 循环 ~110 行；TurnEnd / evaluation telemetry 从 `outcome.record` 统一外发
- **单轮测试矩阵**（`apply_llm_output` + 假 `LlmResponse`）：
  - native tool → Continue
  - code 成功 → Continue
  - 连续 sandbox 错 → BreakToSynthesis
  - chat content → DirectAnswer
  - rag 无证据 → content_blocked
  - skill_request JSON → Continue（非 DirectAnswer）

---

## 附录 A4 — `UnifiedAgent` 死 seam ✅ 已解决

`unified/mod.rs` 已无 `llm_provider` / 死 `temperature`。**无需动作**。
