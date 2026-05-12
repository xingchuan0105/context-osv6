# Agent Harness 三项升级设计 (2026-05-12)

> 状态：2026-05-12 草案，待审阅。
> 起源：2026-05-12 对照 `shareAI-lab/learn-claude-code` 通用 agent harness 与 COS6 现状,识别出三项**当前 agent 层欠账**(非可选优化),记录于此作为后续 P1/P2 实施依据。
> 关联文档：
> - `2026-05-09-runtime-tool-dispatch-architecture.md` — 工具目录/分发,本文 §1 在其上层
> - `2026-04-25-main-agent-memory-and-context-design.md` — 当前的两层记忆,本文 §2 取代其滑动窗口部分
> - `2026-05-10-codebase-gap-review.md` — 本文不重复 P0–P3 已识别项,只覆盖新增缺口

## 0. 总览

| # | 升级项 | 目的 | 优先级 |
|---|--------|------|--------|
| 1 | **Agent 进入真正的 tool-use 循环** | 让模型在单次对话内自主决定调多少次工具、何时停;打破当前"planner 一次出计划→runtime 跑→synthesizer 出答"的硬流水线 | P1 |
| 2 | **滑动窗口替换 `session_summary` 单点摘要** | 长会话(>30 轮)token 上限稳定性;消除单点摘要在重要细节上的信息丢失 | P1 |
| 3 | **Skill 按需加载** | 把当前一次性塞入 system prompt 的大段领域知识(answer 模板、引用规范、风格指南……)拆为模型可按需 `load_skill` 的小文件,降低 baseline token,支持新增领域指令而不改 prompt | P2 |

三项之间的依赖：**升级 1 是另外两项的载体**——只有进入 tool-use 循环,模型才能在中途调 `load_skill("citation-format")` 或 `compact_history(keep_recent=5)`。所以实施顺序固定为 1 → 2 → 3。

---

## 1. Agent 进入真正的 tool-use 循环

### 1.1 现状

```
[chat_agent.rs:55]
  ChatAgent::run → 单次 llm.complete[_stream] → Done
  (没有循环;不能调任何工具)

[rag_agent.rs:60]
  RagAgent::run
    ├─ call_planner (LLM) → RagPlanDecision { calls, next_step }
    ├─ execute_tools(dispatch_all)  ← 一次性并行所有 ToolCall
    ├─ evaluate_rag_iteration → EvalAdvice (recall/coverage 客观信号)
    └─ if EvalAdvice::Synthesize → synthesize_stream_text_from_tool_results
       else 最多 2 次 fallback 迭代 (LoopBudget::rag())
       (每次迭代再走一遍 plan→execute→evaluate)

[web_search_agent.rs] 同 RagAgent,LoopBudget = 2
```

**特征**：
- 模型只能在 `RagPlanDecision` 里**一次性列出**全部 ToolCall,不能在工具结果回来后决定"我再调一次别的"
- 是否继续由 **代码 evaluator** 决定,不是模型 `stop_reason`
- ChatAgent 没有工具能力,完全直答

### 1.2 目标

引入 **`AgentLoop`**——所有 agent kind 共享的 tool-calling 循环：

```rust
// 伪代码（最终接口由实现细化）
async fn agent_loop(
    state: &mut AgentState,
    ctx: &ReactContext<'_>,
) -> Result<AgentRunResult, AppError> {
    loop {
        ctx.check_cancelled()?;

        let response = state.llm.complete_with_tools(
            &state.messages,
            &state.tool_specs,           // 模型可见的工具目录
            state.temperature,
            ctx.cancel.clone(),
        ).await?;

        state.messages.push(assistant_message(&response));

        match response.stop_reason {
            StopReason::EndTurn | StopReason::StopSequence => return finalize(state),
            StopReason::ToolUse => {
                let tool_results = dispatch_all(
                    &response.tool_uses,
                    &state.tool_registry,
                    &state.auth,
                ).await?;
                state.messages.push(user_tool_results(&tool_results));
                state.budget.tick();
                if state.budget.exhausted() {
                    return degrade(state, DegradeReason::BudgetExhausted);
                }
            }
            StopReason::MaxTokens => return degrade(state, DegradeReason::ContextOverflow),
        }
    }
}
```

### 1.3 关键决策

| # | 决策 | 选项 | 选择 | 理由 |
|---|------|------|------|------|
| 1 | 谁判定终止 | 模型 `stop_reason` / 代码 evaluator / 两者并存 | **两者并存,模型优先** | 模型 `EndTurn` 立即终止;`ToolUse` 后再用现有 evaluator 检查客观信号(`recall_count`/`max_score`/`term_coverage`),信号不足时**在下一轮 tool prompt 里附加 "fallback hint"**,但**不替模型决策**。保留客观信号的可观测性与回归测试价值,同时还回模型主导权。 |
| 2 | 工具集如何对模型可见 | 全局统一 / 按 AgentKind 过滤 | **按 AgentKind 过滤** | ChatAgent 看见 `[load_skill, compact_history]`;RagAgent 在此之上看见 6 个 RAG 工具 + `search_web`(允许 RAG→Search 兜底);Search 看见 `[brave_search, fetch_full_page, load_skill]`。avoid 让 chat 误调 `dense_retrieval`。 |
| 3 | 工具调度复用 | 复用 `rag-core/runtime/execute.rs` 的 `dispatch_all` / 重写 | **复用** | `dispatch_all` 已经做了 schema 校验、并行、超时、ToolResult 收集。升级只需把它包成 `AgentToolRegistry::execute(name, args)` 即可被 agent_loop 调用。 |
| 4 | `LoopBudget` 是否保留 | 删除 / 保留作硬上限 | **保留为硬上限** | 模型 `stop_reason` 失控时(无限调工具)的护栏。RAG 默认 6、Search 默认 4、Chat 默认 3——比当前 plan-迭代数翻倍,因为每次 tool_use 才 tick 一次而不是整轮。 |
| 5 | Cancellation 语义 | 不变 | **不变** | `ctx.check_cancelled()` 仍在每一次 `loop` 入口检查;`llm.complete_with_tools` 必须接受 `CancellationToken`。 |
| 6 | 与 Rig 的关系 | 自实现 / 走 Rig | **走 Rig** | `rig_adapter.rs:19` 已是项目认定的"唯一新流式 runtime";rig 0.4+ 已支持 multi-turn tool-calling,直接复用其 `CompletionRequestBuilder::tools(...)` API,避免重造。 |
| 7 | Streaming 兼容 | tool_use 段不 stream / 全程 stream | **全程 stream,tool_use 段以 Activity 事件代替 MessageDelta** | 用户体验上需要看到"正在检索文档..."而不是停顿。`AgentEventSink` 的 `Activity{stage:"tool_use", message:"dense_retrieval(query=...)"}` 已经支持这种语义。 |

### 1.4 影响面

| 文件 | 变更 |
|------|------|
| `crates/app/src/agents/agent_loop.rs` (新增) | 共享循环驱动 |
| `crates/app/src/agents/tool_registry.rs` (新增) | `AgentToolRegistry` —— 把 6 个 rag-core 工具 + `load_skill` + `compact_history` + `search_web` 包成 agent 视角的工具表 |
| `crates/app/src/agents/chat_agent.rs` | 改为通过 `agent_loop` 跑,工具集 = `[load_skill, compact_history]` |
| `crates/app/src/agents/rag_agent.rs` | 删除 plan→execute→evaluate 显式三阶段;evaluator 改为**循环间 hint 注入器**而非决策者;`LoopBudget::rag() = 6` |
| `crates/app/src/agents/web_search_agent.rs` | 同上,`LoopBudget::search() = 4` |
| `crates/llm/src/...` | `LlmClient` 增加 `complete_with_tools` / `complete_stream_with_tools`,基于 rig 实现 |
| `prompts/{chat,rag,web_search}_agent_system.txt` | 重写:从"输出一个 JSON 计划"改为"工具会在需要时由系统注入。回答完毕直接结束本轮" |
| `crates/common/src/tool_call.rs` | `ToolCall` / `ToolResult` 已有,新增 `ToolSpec`(模型可见的工具描述)与 `StopReason` enum |

### 1.5 风险与回滚

- **R1**：模型在没有显式上限下"调爆"工具。**缓解**：`LoopBudget` 硬上限 + 每个工具自身的成本计费上限(rag-core 已有)。
- **R2**：模型选错工具(用 `dense_retrieval` 答闲聊)。**缓解**：tool prompt 描述里强约束适用场景;客观信号 evaluator 在迭代间反馈"上一轮 recall=0,建议改用 X 工具"。
- **R3**：rig 的 tool-calling 兼容性在某些 provider(尤其 DeepSeek)上不稳定。**缓解**：单测覆盖 DeepSeek / DashScope / Gemini-DMXAPI 三家;不稳定时回退到当前 `RagPlanDecision` 路径(保留 feature flag `AGENT_TOOL_LOOP_ENABLED`)。
- **回滚**:整个升级在 feature flag 后面;关掉即回到当前 plan→execute→synthesize 流水线。

### 1.6 验收

1. `chat_agent` 能成功调用 `load_skill("citation-format")` 后回答 RAG 风格问题。
2. `rag_agent` 在 doc_scope 命中率低时**自主**调 `search_web` 兜底,而不是依赖 evaluator 强制 escalate。
3. `web_search_agent` 在第一次 brave 结果空时自主再调一次(不同 vertical),不再走 `EscalateVertical` 硬编码分支。
4. 长链路 telemetry:每次 `tool_use` → `Activity` 事件;`iterations` 字段记录每次 tool_use 的 stage + signals。
5. e2e:对 100 条历史回归 query 跑 A/B,模型路径在 recall@5 不劣化于当前流水线。

---

## 2. 滑动窗口替换 `session_summary` 单点摘要

### 2.1 现状

```
[lib_impl/memory_helpers.rs]
  每轮对话后台调 MEMORY_LLM 生成 session_summary (单一字符串)
  AgentRequest.session_summary 在 build_chat_messages / RagAgent 里
    被拼到 system prompt 顶部

[AgentRequest.messages]
  仍是全量近期消息(由 chat service 在拼装时按数量截断)
```

**问题**：
- **单点摘要**：所有历史压成一个段落,无法保留"第 5 轮用户给过具体型号 XR-450"这种关键事实
- **没有真正的滑动**：`messages` 截断是按硬阈值,超过即丢失;`session_summary` 是事后异步生成,不一定能在下一轮请求前更新到位
- **token 浪费**:短会话也带满 summary;长会话又因 messages 截断丢上下文

### 2.2 目标:三层滑动压缩

借鉴 learn-claude-code s06 三层结构,落地为 COS6 的 PG 存储模式：

```
Layer 1 (Hot, 原文)   : 最近 N 轮 (默认 N=8) — 完整保留
Layer 2 (Warm, 摘要段): 上一段窗口(轮 N+1 ~ 2N) — 按 chunk 摘要,4–6 句
Layer 3 (Cold, 卷起)  : 更早 — 单段长摘要,只保留"用户偏好/已确认事实/未解决问题"三类

每次新增一轮 → 触发 promote(layer1→layer2→layer3) 检查
```

### 2.3 关键决策

| # | 决策 | 选项 | 选择 | 理由 |
|---|------|------|------|------|
| 1 | 数据模型 | 新表 / 沿用 `messages` 加列 | **`messages` 加 `layer` 枚举列 + `summary_text` 可空列** | 避免跨表事务;Layer 2/3 直接是同一行的摘要替换。需要 migration。 |
| 2 | promote 触发时机 | 每次 user turn / 后台 cron / agent 内 tool | **agent 内 tool `compact_history(keep_recent=N)`** | 让模型自己判断"现在上下文压力大了,我自己 compact";同时保留兜底:`user turn` 入口检查 layer1 数量 > 2N 时强制 demote。 |
| 3 | 摘要写谁 | `MEMORY_LLM`(DeepSeek)/ `INGESTION_LLM`(Gemini) | **保留 `MEMORY_LLM`** | 现有 KeyVault 已 wire,摘要质量在 DeepSeek 上已验证。 |
| 4 | Layer 3 何时回写 | 每轮 / 跨会话 boundary | **跨会话 boundary 后台合并** | 单轮内不影响延迟;`sessions.rs` 在 session inactive > 10 min 时触发后台 finalize。 |
| 5 | 注入策略 | system prompt 拼接 / user 工具结果 | **system prompt 中 layer3 + assistant/user 交替注入 layer2 + 原文 layer1** | layer3 是"事实背景",放 system 合理;layer2 是"近期对话的脱水版",必须放回对话流以保持语义连贯,作为合成的 `assistant`(对应原助手轮)与 `user`(对应原用户轮)交替消息。 |
| 6 | 阈值 N 可调 | 硬编码 / env var / per-session | **env var `MEMORY_HOT_WINDOW` 默认 8** | 与现有 `RAG_MIN_MAX_SCORE` 等 evaluator 阈值同风格。 |

### 2.4 影响面

| 文件 | 变更 |
|------|------|
| `migrations/00XX_messages_layer.sql` | 加 `layer` (smallint default 1) + `summary_text` (text null) 列;索引 `(session_id, layer, created_at)` |
| `crates/storage-pg/src/repositories/messages.rs` | `select_layered(session_id) → LayeredHistory { layer1: Vec<Msg>, layer2: Vec<Msg>, layer3: Option<String> }` |
| `crates/app/src/lib_impl/memory_helpers.rs` | 删除 `session_summary` 单点合成;新增 `promote_layer / build_layered_messages_for_request` |
| `crates/app/src/agents/runtime.rs` | `AgentRequest.session_summary` 废弃(保留字段一个 release 但不再写入),替换为 `AgentRequest.layered_history: LayeredHistory` |
| `crates/app/src/agents/{chat,rag,web_search}_agent.rs` | 拼 messages 时用 `layered_history.into_chat_messages()` 替换当前 system+messages 拼接 |
| `crates/app/src/agents/tool_registry.rs` | 注册 `compact_history` 工具:输入 `{keep_recent: u8}`,执行 promote 并返回新层数 |
| `prompts/session_summary_system.txt` | 拆为 `layer2_summary_system.txt`(短窗口压缩) + `layer3_finalize_system.txt`(跨会话 finalize),后者复用现有逻辑 |

### 2.5 风险与回滚

- **R1**:摘要遗漏关键事实(用户型号、未完成订单 ID)。**缓解**:layer2 摘要 prompt 强制"保留所有专有名词、数字、ID"白名单;layer3 finalize 增加"用户偏好/已确认事实/未解决问题"三类抽取槽位。
- **R2**:模型滥用 `compact_history` 工具频繁触发摘要,token 反而增加。**缓解**:工具描述里写明"通常无需手动调用;系统会在 hot 窗口溢出时自动 demote"。设硬上限"每会话每分钟最多 1 次"。
- **R3**:迁移期老数据没有 `layer` 字段。**缓解**:migration 默认值 `layer = 1`;启动期后台 backfill 把 N 轮之前的 demote。
- **回滚**:feature flag `MEMORY_LAYERED_ENABLED`;关掉则 `AgentRequest` 仍按当前 `session_summary` 路径拼接。

### 2.6 验收

1. 50 轮会话下,token 占用稳定在(单轮)< 12k(当前会无限增长直到截断)。
2. 在第 30 轮提问"我刚才提到的设备型号是什么",模型能从 layer2/layer3 找回(当前 session_summary 路径丢失率 > 30%)。
3. promote 单测:每 N+1 轮触发一次 demote,layer1 永远 ≤ N。
4. `compact_history` 工具被 agent 主动调用时,事件流出现 `Activity{stage:"compacting_history"}`。

---

## 3. Skill 按需加载

### 3.1 现状

```
[prompts/chat_agent_system.txt]       — 编译期 include_str! 嵌入,固定一份
[prompts/rag_plan_system.txt]         — 同上
[prompts/rag_answer_system.txt]       — 同上
[prompts/web_search_system.txt]       — 同上
...
```

所有"系统侧领域知识"——回答格式约定、citation 规范、风格指南、敏感话题降级模板、产品功能限制说明——一次性写在 system prompt 里,**不管这一轮用不用得上**。

### 3.2 目标

把领域知识拆为 `skills/<skill-name>.md` 文件,system prompt 仅保留**通用 agent 协议**(工具列表、输出契约、basic role)。模型按需调 `load_skill(name)` 工具,把对应 SKILL 内容以 `tool_result` 形式注入对话流。

```
prompts/
  agent_protocol.txt                 ← 新的精简 system prompt (所有 agent 共用)
skills/
  citation-format.md                 ← 引用格式约定
  refusal-templates.md               ← 拒答模板
  rag-answer-style.md                ← RAG 回答风格
  web-search-synthesis.md            ← 多源融合规范
  doc-summary-style.md
  ...
```

### 3.3 关键决策

| # | 决策 | 选项 | 选择 | 理由 |
|---|------|------|------|------|
| 1 | 加载粒度 | 文件级 / section 级 | **文件级** | 实现简单;section 级需要再做索引,收益不显著。单个 SKILL 文件目标 < 2k tokens。 |
| 2 | 加载时机 | 模型主动 `load_skill` / 系统启发式预加载 | **两种并行** | 主动:`load_skill("rag-answer-style")` 工具;启发式:agent 启动时根据 `AgentKind` 自动 inject 默认 skill(如 RAG agent 默认带 `rag-answer-style`),避免模型每次都要先 load。 |
| 3 | 缓存策略 | 不缓存 / per-session / 全局 | **全局 + ETag** | SKILL 文件在 runtime 极少变;`SkillRegistry` 启动期载入,响应里塞 ETag,模型同一会话再次 `load_skill` 同名时返回 `{status:"unchanged", etag}` 提示已加载,避免重复 inject。 |
| 4 | 存放位置 | `prompts/` / 新 `skills/` 目录 / `crates/app/skills/` | **`avrag-rs/skills/`** | 与 `prompts/` 平级,符合 `.claude/skills/` 的 superpowers 习惯。`include_dir!` 编译期嵌入,运维不需挂卷。 |
| 5 | SKILL 元数据 | 无 / frontmatter | **YAML frontmatter** | 与 `.claude/skills/` 习惯一致:`name`、`description`、`applicable_when`、`agent_kinds`。`SkillRegistry::list()` 把 description 拼成"工具目录",作为 `load_skill` 工具的可选 enum。 |
| 6 | 与 i18n 关系 | 多语言 SKILL 各一份 / 模板渲染 | **多语言各一份** | 与现有 `i18n.rs` 风格一致;`load_skill(name, lang)` 按 `AgentRequest.language` 路由。 |

### 3.4 SKILL 文件格式

```markdown
---
name: rag-answer-style
description: COS6 RAG 答案的回答风格、citation 规范、降级模板
applicable_when: agent_kind == "rag" && tool_results.contains_documents
agent_kinds: [rag]
languages: [zh, en]
---

# RAG 答案风格

## 引用规范
- 每条来源都要 [^cite:doc_id:chunk_id] 格式
- ...

## 拒答情形
- ...
```

### 3.5 影响面

| 文件 | 变更 |
|------|------|
| `avrag-rs/skills/*.md` (新增) | 初始拆出 6–10 个 skill,迁移自现 prompts |
| `crates/app/src/skills/mod.rs` (新增) | `SkillRegistry` + `Skill` 结构;`include_dir!` 嵌入 |
| `crates/app/src/agents/tool_registry.rs` | 注册 `load_skill(name: enum, lang?: string)` 工具 |
| `prompts/agent_protocol.txt` (新增) | 通用 system prompt,所有 agent 共用 |
| `prompts/{chat_agent_system,rag_plan_system,rag_answer_system,web_search_system}.txt` | 大幅瘦身,把领域知识迁出到 skills/ |
| `crates/app/src/agents/agent_loop.rs` | 启动时按 AgentKind 预 inject 默认 skill(放第一条 `user` tool_result) |

### 3.6 风险与回滚

- **R1**:模型不知道 skill 存在,从来不调 `load_skill`。**缓解**:`agent_protocol.txt` 显式列出 SkillRegistry 目录(name + description),与工具表一起注入;每个 AgentKind 的默认 skill 走预 inject 不依赖模型主动。
- **R2**:SKILL 文件膨胀(每个 > 5k tokens),`load_skill` 反而吃 token。**缓解**:CI lint 限制单 SKILL 文件 < 2k tokens;> 2k 时强制拆分。
- **R3**:多语言不同步(`zh` 改了 `en` 没改)。**缓解**:`SkillRegistry` 启动期校验同 name 下 zh/en 都存在,缺失启动失败。
- **回滚**:feature flag `SKILL_REGISTRY_ENABLED`;关掉则 `prompts/*.txt` 保留原内容(灰度期不删 prompts),agent_loop 走老路径。

### 3.7 验收

1. baseline system prompt token 数:`chat_agent_system.txt` 从当前 ~1.8k 降到 < 400。
2. 单轮 token 减少:同样问题在 chat 模式下,prompt_tokens 降幅 ≥ 30%(简单闲聊问题不需要 load 任何 skill)。
3. RAG/Search agent 自动 inject 对应默认 skill,iterations[0] 的 plan 中可见 `inject_skill: rag-answer-style` 记录。
4. 新增 skill(如 `compliance-notes.md`)无需重启,只需重 build(`include_dir!` 在编译期);文档更新 = PR + skill 文件改动,不动 prompt。

---

## 4. 实施顺序与里程碑

| Phase | 内容 | 依赖 | 估算 |
|-------|------|------|------|
| **Phase A** | 工具表 + LlmClient.complete_with_tools + ChatAgent 跑通 tool_use 循环(只挂 `load_skill` / `compact_history` 两个占位工具) | rig 升级 | 1 周 |
| **Phase B** | RagAgent / WebSearchAgent 迁移到 agent_loop,evaluator 改为 hint 注入器 | Phase A | 1–2 周 |
| **Phase C** | messages 表 migration + LayeredHistory + `compact_history` 工具落地 | Phase A(为了让模型能调) | 1 周 |
| **Phase D** | skills/ 目录初始化 + SkillRegistry + 默认 skill 预 inject | Phase A | 3 天 |
| **Phase E** | feature flag 默认开启 + 老路径 deprecation 一个 release 周期后删除 | Phase B+C+D | 2 周 |

> 总计约 5–7 周。Phase A 走通即可在 dev 环境对照新老两条路径做 A/B,后续阶段都可独立合入 main 不阻塞其他工作。

## 5. 与 `learn-claude-code` 的对应关系

| 本文 § | LCC session | 差异保留点 |
|--------|-------------|-----------|
| §1 tool-use loop | s01 + s02 | 保留 `LoopBudget` 硬上限、客观信号 hint;不放弃 evaluator |
| §2 滑动窗口 | s06 | 三层结构相同;数据存 PG 而非 JSON 文件;模型主动 `compact_history` 工具触发 |
| §3 Skill | s05 | SKILL 文件结构基本照搬;增加 i18n / `applicable_when` 元数据 |

明确**不抄**的部分:LCC s07–s12(任务图、团队、worktree、自主认领)对应 CLI 多 agent 范式,COS6 是 HTTP 服务,不适配。

## 6. 开放问题

- Q1:Phase A 里 `load_skill` / `compact_history` 当占位工具时,SkillRegistry 是空,工具调用怎么返回?**初步答**:返回 `{status: "noop", reason: "skill_registry_not_initialized"}`,Phase D 落地后自然生效。
- Q2:rig 在 DeepSeek 上的 tool-calling 兼容性还需测。建议 Phase A 第一周做 spike,失败则推迟整盘并评估 fallback。
- Q3:`session_summary` 字段是直接删除还是保留 deprecation 期?**初步答**:保留一个 release,`AgentRequest` 同时写 `session_summary` 和 `layered_history`,下个 release 删 `session_summary`。
