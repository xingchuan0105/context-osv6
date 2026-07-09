# ADR-0005: 统一智能体内核（AgentKernel）+ 三模式配置

| 项目 | 内容 |
|---|---|
| 状态 | 已采纳 |
| 决策日期 | 2026-06-06 |
| 提出者 | AI 助手（与用户共同决策） |
| 影响范围 | `crates/app/src/agents/`、`crates/app/src/chat/`、集成测试、前端接口 |

---

## 1. 背景与动机

### 1.1 当前问题

当前 `avrag-rs` 的 RAG / Chat / Search 三条链路各自拥有独立的策略实现（`ChatStrategy`、`RagStrategy`、`SearchStrategy`），每个策略内部维护一套状态机：

- `RagStrategy`：Plan → Execute → Eval → Answer（线性状态机，已完成 ADR-0004 改造为原生工具调用循环）
- `ChatStrategy`：单轮直接回答（无循环）
- `SearchStrategy`：Plan → WebSearch → Answer（简化循环）

**问题**：
1. **循环逻辑重复**：三条链路的"Plan→工具调用→合成答案"核心骨架高度相似，但代码分散在三处；
2. **模式切换成本高**：前端通过 `agent_type` 切 mode 时，后端必须换整条 pipeline，历史上下文无法自然继承；
3. **工具管控粒度粗**：每个策略的工具加载是硬编码的，无法按阶段（Plan vs Answer）动态调整；
4. **SSE 事件不统一**：三个策略各自发不同的事件名和 payload，前端需要三套解析逻辑。

### 1.2 目标

将三条独立策略的**循环骨架**合并为一个**统一的 AgentKernel**，差异点下沉为**模式配置**（Mode Configuration）。

**非目标**：
- 不替换 `avrag-llm` 的 LLM client（已 production-tested，48/48 测试覆盖）；
- 不引入 GraphFlow（当前无业务需求触发 interruptible execution / 持久化）；
- 不改变前端 `agent_type: "chat" | "rag" | "search"` 的显式切换语义。

---

## 2. 决策

### 2.1 核心决策

采用 **"统一 AgentKernel + 三 Mode 配置"** 架构：

- **AgentKernel**：一个统一的循环骨架，负责 Plan↔工具调用 的 0-N 轮迭代、Evidence Gate 判定、Answer 合成；
- **AgentMode trait**：定义 Mode 差异点的接口（工具目录、技能体、迭代预算、Evidence Gate 等）；
- **三个实现**：`ChatMode`、`RagMode`、`SearchMode`；
- **动态工具加载**：Plan 阶段和 Answer 阶段分别暴露不同的工具集合，由 Kernel 在每次 LLM 调用时**动态传入**。

### 2.2 关键边界决策

| 决策项 | 结论 | 理由 |
|---|---|---|
| **GraphFlow** | 不引入 | 当前业务无 interruptible / 人类介入 / 长时任务恢复需求；社区成熟度不足（v0.2.3，still in progress）；引入成本 > 收益 |
| **Rig** | 不引入 | Rig 的 `Agent` 在构造时静态绑定 tools，不支持"Plan 阶段检索工具 / Answer 阶段格式工具"的阶段级动态切换；avrag-llm 已满足需求 |
| **Agent 实例数** | 0（无实例概念） | 直接调 `avrag_llm::complete_with_tools(messages, tools)`，工具每次调用动态指定 |
| **Plan vs Answer 工具** | 同一 Mode，不同阶段，不同 tools | Plan 阶段暴露检索/计算工具；Answer 阶段暴露格式/输出工具；靠 skill body 约束 LLM 行为 |
| **跨 mode 历史** | 共享，只注入 user 角色 | 历史是跨 mode 协作的桥梁；agent 答案不进历史，避免污染 |
| **历史注入格式** | `[prior_user_query] <text>` 前缀 | 明确标记这是历史而非当前证据 |
| **Answer 阶段历史** | 只看本轮 loop_messages + evidence | Answer LLM 不需要重新理解用户历史背景 |
| **Fallback** | 仅硬 fallback（infra 失败） | LLM API 宕机、向量库宕机、搜索 API 超时；业务降级靠 skill prompt |
| **空检索处理** | RAG 硬降级（固定文案）；Search 找部分结果 | 由 skill body 驱动，不是代码路径 |

---

## 3. AgentMode 接口设计

```rust
/// 模式配置接口：定义 Chat / RAG / Search 三者的差异点。
/// Kernel 通过此接口获取"当前 mode 需要什么"，不负责"怎么执行"。
pub trait AgentMode: Send + Sync {
    /// 模式标识："chat" / "rag" / "search"
    fn id(&self) -> &'static str;

    /// Plan 阶段可用的工具目录（检索/计算类）
    fn plan_tools(&self) -> Vec<ToolSpec>;

    /// Answer 阶段可用的工具目录（格式/输出类）
    fn answer_tools(&self) -> Vec<ToolSpec>;

    /// Plan 阶段技能体（system prompt 片段）
    /// 职责：告诉 LLM "你是 planner，可用以下工具，目标是收集证据"
    fn plan_skill_body(&self) -> &str;

    /// Answer 阶段技能体（system prompt 片段）
    /// 职责：告诉 LLM "你是 answerer，已收集到证据，现在合成最终答案，禁止调检索工具，必须带引用"
    fn answer_skill_body(&self) -> &str;

    /// 最大迭代轮数（Plan↔工具调用循环的预算上限）
    fn max_iterations(&self) -> u8;

    /// Evidence Gate（仅 RAG/Search 需要；Chat 返回 None）
    fn evidence_gate(&self) -> Option<Box<dyn EvidenceGate>>;

    /// 空检索时的硬降级文案（仅 RAG 使用）
    fn empty_evidence_response(&self) -> Option<String>;

    /// 跨 mode 共享历史的过滤策略
    /// 默认：只保留 role = user 的消息，并加 `[prior_user_query]` 前缀
    fn filter_cross_mode_history(&self, history: Vec<ChatMessage>) -> Vec<ChatMessage>;
}
```

### 3.1 为什么 Plan 和 Answer 是同一 Mode 的两个阶段

用户明确提出的业务约束：
> "一个模式一个 agent。Plan 阶段是检索工具，Answer 阶段是一些格式工具而已。"

这意味着：
- **Mode 是业务身份**（Chat / RAG / Search），不是代码对象；
- **阶段是时间行为**（Plan 时收集证据，Answer 时格式化输出），不是独立实体；
- **工具按阶段动态暴露**，靠 skill body 约束 LLM "现在该调哪类工具"。

### 3.2 三个 Mode 的实现差异

| 维度 | ChatMode | RagMode | SearchMode |
|---|---|---|---|
| `id` | `"chat"` | `"rag"` | `"search"` |
| `plan_tools` | calculator, code_execution, web_fetch, weather_query | dense_retrieval, lexical_retrieval, hybrid_retrieval, rerank, chunk_fetch, focus_analysis, citation_lookup | web_search |
| `answer_tools` | （轻量格式工具或空） | html-renderer, ppt-generation, presentation-html | html-renderer, web_fetch |
| `plan_skill_body` | `prompts/skills/chat-plan/SKILL.md` | `prompts/skills/rag-plan/SKILL.md` | `prompts/skills/search-plan/SKILL.md` |
| `answer_skill_body` | `prompts/skills/chat/SKILL.md` | `prompts/skills/rag-answer/SKILL.md` | `prompts/skills/search-answer/SKILL.md` |
| `max_iterations` | 4 | 4 | 2 |
| `evidence_gate` | `None` | `DefaultEvidenceGate`（focus_cv_threshold=0.30, focus_score_gap=0.15, term_coverage） | `DefaultEvidenceGate`（配置可能不同） |
| `empty_evidence_response` | `None` | `"未找到相关文档..."` | `None` |

---

## 4. AgentKernel 循环设计

### 4.1 核心循环骨架

```rust
pub struct AgentKernel<M: AgentMode> {
    mode: M,
    llm: Arc<LlmClient>,
}

impl<M: AgentMode> AgentKernel<M> {
    pub async fn run(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let mut loop_messages: Vec<ChatMessage> = vec![];
        let mut iteration = 0;
        let max_iter = self.mode.max_iterations();

        // 1. 构造 Plan 阶段初始 messages
        let mut plan_messages = vec![
            ChatMessage::system(self.mode.plan_skill_body()),
        ];
        
        // 2. 注入跨 mode 共享历史（只取 user 角色）
        let cross_history = self.mode.filter_cross_mode_history(request.history);
        plan_messages.extend(cross_history);
        
        // 3. 当前 user query
        plan_messages.push(ChatMessage::user(request.query.clone()));

        // === Plan↔工具调用 循环 ===
        loop {
            if iteration >= max_iter {
                // 预算耗尽：直接进 Answer（用已有 evidence）
                break;
            }

            // 4. 调 LLM（Plan 阶段工具目录）
            let plan_response = self.llm
                .complete_with_tools(&plan_messages, &self.mode.plan_tools(), Some(0.7))
                .await?;

            // 5. LLM 决定调工具还是直接答
            match plan_response.tool_calls {
                Some(tool_calls) => {
                    // 5a. 执行工具
                    let tool_results = execute_tools(tool_calls).await?;
                    
                    // 5b. Evidence Gate（RAG/Search 专属）
                    if let Some(gate) = self.mode.evidence_gate() {
                        match gate.check(&tool_results) {
                            EvidenceGateResult::Degrade => {
                                // RAG: 返回固定文案；Search: 理论上不会触发（靠 skill prompt）
                                if let Some(degrade_text) = self.mode.empty_evidence_response() {
                                    return Ok(AgentResponse::text(degrade_text));
                                }
                            }
                            EvidenceGateResult::Pass => {
                                // 证据充分 → 进 Answer
                                break;
                            }
                            EvidenceGateResult::NeedsFocus { chunk_count, mean_score, cv } => {
                                // 证据不充分 → 继续迭代
                                // loop_messages 保留，下一轮 Plan 会看到这些 tool results
                            }
                        }
                    }

                    // 5c. 把 tool_call + tool_result 加入 loop_messages
                    loop_messages.push(build_assistant_message_with_tool_calls(&plan_response));
                    loop_messages.push(build_tool_message(&tool_results));

                    // 5d. 下一轮 Plan messages = 历史 + loop_messages
                    plan_messages = vec![
                        ChatMessage::system(self.mode.plan_skill_body()),
                    ];
                    plan_messages.extend(cross_history.clone());
                    plan_messages.extend(loop_messages.clone());
                    plan_messages.push(ChatMessage::user(request.query.clone()));
                }
                None => {
                    // 5e. LLM 直接给出内容 → 视为已有足够信息，进 Answer
                    if let Some(content) = plan_response.content {
                        // 如果 LLM 在 Plan 阶段就直接给了答案（Chat 常见）
                        // 对于 RAG/Search，skill body 应约束 LLM 不要这么做
                        // 但兜底：直接返回
                        return Ok(AgentResponse::text(content));
                    }
                    break;
                }
            }

            iteration += 1;
        }

        // === Answer 阶段 ===
        let mut answer_messages = vec![
            ChatMessage::system(self.mode.answer_skill_body()),
        ];
        
        // Answer 阶段只看本轮 loop_messages（ReAct 内部历史）+ evidence
        // 不看跨 mode 历史——Plan 阶段已经消化了历史背景
        answer_messages.extend(loop_messages);
        answer_messages.push(ChatMessage::user(
            "基于上述收集到的证据和工具结果，合成最终答案。"
        ));

        // 6. 调 LLM（Answer 阶段工具目录——格式工具）
        let answer_response = self.llm
            .complete_with_tools(&answer_messages, &self.mode.answer_tools(), Some(0.7))
            .await?;

        // 7. 如果 Answer 阶段 LLM 又调了格式工具，执行后继续合成
        // （理论上 answer_skill_body 会约束"先调格式工具再输出"，但 Kernel 支持多轮 Answer 工具调用）
        let final_answer = if let Some(tool_calls) = answer_response.tool_calls {
            let tool_results = execute_tools(tool_calls).await?;
            answer_messages.push(build_assistant_message_with_tool_calls(&answer_response));
            answer_messages.push(build_tool_message(&tool_results));
            answer_messages.push(ChatMessage::user("请基于格式工具的结果，输出最终答案。"));
            
            let final_response = self.llm
                .complete(&answer_messages, Some(0.7))
                .await?;
            final_response.content.unwrap_or_default()
        } else {
            answer_response.content.unwrap_or_default()
        };

        Ok(AgentResponse::text(final_answer))
    }
}
```

### 4.2 关键设计点

#### 4.2.1 为什么 Plan 和 Answer 都调 `complete_with_tools`

因为 **Answer 阶段也需要调格式工具**（html-renderer, ppt-generation 等）。如果 Answer 阶段调 `complete`（无工具），格式工具无法被调用。

**约束**：`answer_skill_body` 必须明确写入：
> "你现在可以调用以下格式工具来美化输出。如果你不需要格式工具，直接输出最终答案。"

#### 4.2.2 为什么 Evidence Gate 在 `NeedsFocus` 时继续循环而不是直接 Answer

ADR-0004 §2.1 要求 "Pass→Answer, NeedsFocus→继续迭代"。当前实现遵循此语义：
- `Pass`：证据充分 → 跳出循环进 Answer；
- `NeedsFocus`：证据不充分 → loop_messages 保留，下一轮 Plan 基于现有证据继续；
- `Degrade`：证据为 0 或质量极差 → RAG 返回固定降级文案。

#### 4.2.3 为什么 `loop_messages` 只在本轮有效

`loop_messages` 是 **AgentKernel 内部的 ReAct 轨迹**，生命周期 = 本次请求。它包含：
- assistant 的 tool_call 决策
- tool 的执行结果

**不**进持久化历史。跨请求的历史只保留 `user` 角色的 query（见 4.2.4）。

#### 4.2.4 跨 mode 共享历史注入策略

```rust
fn filter_cross_mode_history(&self, history: Vec<ChatMessage>) -> Vec<ChatMessage> {
    history.into_iter()
        .filter(|m| m.role == "user")
        .map(|m| ChatMessage {
            role: "user".to_string(),
            content: format!("[prior_user_query] {}", m.content),
            ..m
        })
        .collect()
}
```

- **只取 user 角色**：agent 的推理过程和答案**不**进历史，避免污染；
- **`[prior_user_query]` 前缀**：明确告诉 LLM "这是历史查询，不是当前证据"；
- **跨 mode 共享**：Chat 模式的用户 query 对 RAG 模式也有参考价值（"你之前问过 X，这次要 Y"）。

---

## 5. SSE 事件协议统一

AgentKernel 统一发出以下 SSE 事件，前端**一套解析逻辑**适配所有 mode：

| 事件名 | 触发时机 | payload |
|---|---|---|
| `plan.start` | Plan 阶段开始 | `{mode, iteration}` |
| `plan.thinking` | 收到 LLM 的 reasoning content | `{text}` |
| `tool.call` | LLM 决定调工具 | `{tool_name, arguments}` |
| `tool.result` | 工具执行完成 | `{tool_name, status, summary}` |
| `evidence.gate` | Evidence Gate 判定 | `{result: "Pass" | "NeedsFocus" | "Degrade", details?}` |
| `answer.start` | Answer 阶段开始 | `{mode}` |
| `answer.chunk` | Answer 流式输出 | `{text}` |
| `answer.format_tool` | Answer 阶段调格式工具 | `{tool_name}` |
| `complete` | 整个请求完成 | `{mode, duration_ms}` |
| `error` | 任何 infra 错误 | `{code, message}` |

**注意**：`answer.chunk` 在 Answer 阶段启用流式输出（如果 `LlmClient` 支持 streaming）。Plan 阶段**不**流式（LLM 输出的是结构化 tool_call，不需要逐字显示）。

---

## 6. 错误处理边界

### 6.1 硬 Fallback（infra 失败）

| 失败场景 | 行为 |
|---|---|
| LLM API 超时/5xx | 返回 `error` SSE → 前端提示 "服务暂时不可用，请重试" |
| 向量库连接失败 | `dense_retrieval` 工具返回错误 → LLM 收到 error message → 可能继续或降级 |
| 搜索 API 失败 | `web_search` 工具返回错误 → 同上 |

### 6.2 业务降级（非 infra 失败）

| 场景 | 降级方式 |
|---|---|
| Evidence Gate `Degrade` | RAG 返回固定文案（`empty_evidence_response`） |
| 工具执行返回空结果 | 由 skill body 告诉 LLM "结果是空的，请基于已有信息回答或告知用户" |
| LLM 在 Plan 阶段直接给答案（未调工具） | Chat 模式下**接受**；RAG/Search 模式下 skill body 应约束避免 |
| LLM 在 Answer 阶段调检索工具 | `answer_tools` 里**不包含**检索工具，LLM 物理上无法调；如果强行尝试，会收到 "tool not found" error |

**原则**：业务降级**不走代码分支**，靠 skill body 的措辞引导 LLM 自我修复。

---

## 7. 迁移路径

### 阶段 1：建立 AgentKernel 骨架（2 周）

- [ ] 新建 `crates/app/src/agents/kernel/mod.rs` —— `AgentKernel` 结构 + `run()` 方法
- [ ] 新建 `crates/app/src/agents/kernel/mode.rs` —— `AgentMode` trait 定义
- [ ] 实现 `ChatMode` 作为第一个 Mode（最简单，无 Evidence Gate）
- [ ] 集成测试：`test_chat_agent_loop_basic`、`test_chat_agent_loop_tool_call`
- [ ] 不删除旧 `ChatStrategy`，并行存在

**验证标准**：`cargo test -p app --test kernel_chat` 通过。

### 阶段 2：迁移 RAG 和 Search（2 周）

- [ ] 实现 `RagMode`（含 Evidence Gate、7 Plan tools、3 Answer tools）
- [ ] 实现 `SearchMode`（含 web_search、Answer 格式工具）
- [ ] 将 `pipeline_steps::dispatch_mode` 改为走 `AgentKernel`
- [ ] 集成测试覆盖：
  - 多轮迭代
  - 预算耗尽
  - Evidence Gate 三种结果
  - 跨 mode 历史共享
  - Answer 阶段格式工具调用
- [ ] 旧 `RagStrategy` / `SearchStrategy` 标记 `#[deprecated]`

**验证标准**：现有 442 lib tests 不回归 + 新增 kernel 测试全绿。

### 阶段 3：删除旧 Strategy（1 周）

- [ ] 删除 `ChatStrategy` / `RagStrategy` / `SearchStrategy` 的循环骨架代码
- [ ] 保留 `RagContext` / `ChatContext` / `SearchContext` 的字段映射到 `KernelContext`
- [ ] 更新 E2E 测试到走 kernel 路径
- [ ] 更新文档

**验证标准**：`cargo test -p app --lib` 全绿 + E2E 测试（需环境变量）通过。

---

## 8. 测试策略

### 8.1 Mock LLM 层

复用 ADR-0004 的 `ScriptedLlmProvider`：
- 预编排 LLM 响应序列（第 1 轮返回 tool_calls，第 2 轮返回 content）；
- 验证 Kernel 是否正确迭代、正确追加 messages。

### 8.2 Mock DataPlane 层

复用 ADR-0004 的 `ScriptedDataPlane`：
- 模拟 `dense_retrieval` 返回 0 / 3 / 10 个 chunk；
- 验证 Evidence Gate 在不同 chunk 分布下的行为。

### 8.3 切片测试（Slice Tests）

| 切片 | 测试目标 |
|---|---|
| Slice A | Plan 阶段调 1 个工具 → Answer 阶段直接输出 |
| Slice B | Plan 阶段调 2 轮工具 → Evidence Gate `Pass` → Answer |
| Slice C | Plan 阶段调 N 轮 → 预算耗尽 → Answer 用已有 evidence |
| Slice D | Evidence Gate `Degrade` → 硬降级文案 |
| Slice E | Answer 阶段调格式工具 → 最终输出 |
| Slice F | 跨 mode 切 Chat→RAG → 历史 user query 被注入 |

---

## 9. 替代方案（已否决）

### 9.1 方案 A：引入 Rig 适配层

- **为什么否决**：Rig 的 `Agent` 在构造时静态绑定 tools，不支持"Plan 阶段检索工具 / Answer 阶段格式工具"的阶段级动态切换；avrag-llm 已覆盖需求；引入 Rig 增加 breaking change 风险。

### 9.2 方案 B：引入 GraphFlow 编排

- **为什么否决**：GraphFlow 的 tick 粒度是 Task 级，看不到 AgentKernel 内部每轮 LLM/tool 行为；当前业务无 interruptible / 人类介入需求；社区成熟度不足。

### 9.3 方案 C：6 个 Agent 实例（Plan Agent + Answer Agent × 3 Mode）

- **为什么否决**：用户明确反对"一个模式拆两个 agent"；工具差异应由阶段控制，不是实例分裂；6 实例增加管理复杂度且无业务收益。

---

## 10. 开放问题

### 10.1 GraphFlow 触发条件（何时重新评估）

当前不引入 GraphFlow。若未来出现以下任一需求，重新评估：

1. **人类介入**：AI 流程跑一半，用户需要修改 query / 提供反馈 / 审批后继续；
2. **长时任务恢复**：AI 流程运行 > 1 小时，需要可中断 + 持久化 + 恢复；
3. **Audit & 回放**：需要逐 tick 回放 AI 决策过程，用于合规审计；
4. **多步审批流**：AI 输出需经人工审批后才能进入下一阶段。

### 10.2 Rig 触发条件（何时重新评估）

当前不引入 Rig。若未来出现以下情况，重新评估：

1. Rig 支持**调用时动态切换 tools**（非构造时静态绑定）；
2. Rig 生态出现**我们必须依赖的 provider/tool**（avrag-llm 未覆盖）；
3. Rig 达到稳定 LTS（长期支持），breaking change 风险消除。

### 10.3 新增第四 Mode

当前设计支持对称扩展：新增 enum 变体 + 实现 `AgentMode` trait + 配置 tools/skills。无需修改 Kernel 循环骨架。

---

## 11. 影响与后果

### 11.1 正面影响

- **代码复用**：三条链路的核心循环合并为一处，维护成本降低 ~60%；
- **模式切换零成本**：前端切 `agent_type` 时，后端只换 `AgentMode` 实现，历史上下文自动继承；
- **工具管控精确**：Plan / Answer 阶段分别暴露不同工具，LLM 行为更可预测；
- **SSE 统一**：前端一套事件解析逻辑适配所有 mode；
- **测试覆盖提升**：Mock 测试集中在 Kernel 一处，覆盖率更高。

### 11.2 负面影响

- **初期迁移成本**：阶段 1-3 需 ~5 周，期间新旧代码并存；
- **skill body 责任加重**："LLM 什么时候调什么工具"完全靠 skill body 约束，对 prompt 质量要求更高；
- **Rig 生态错过**：短期内无法使用 Rig 的 provider 抽象、内置 tools、社区示例。

### 11.3 兼容性

- **前端接口不变**：`POST /chat` 的 `agent_type` 字段语义不变；
- **数据库 schema 不变**：`chat_messages` / `message_tags` / `session_summary` 表不变；
- **LLM API 不变**：继续调 `avrag_llm::complete_with_tools`（OpenAI 协议）。

---

## 12. 参考文档

- ADR-0004: RAG Agent Loop with Native Tool Calling
- `crates/llm/src/client.rs` —— `complete_with_tools` 实现
- `crates/rag-core/src/evidence_gate.rs` —— Evidence Gate 逻辑
- `crates/app/src/agents/strategy/rag.rs` —— 当前 RagStrategy 实现
- `crates/app/tests/strategy_rag_agent_loop.rs` —— ADR-0004 集成测试

---

*本文档由 AI 助手与用户共同决策生成，经多轮 Rig/GraphFlow 框架调研后收敛至此方案。*
