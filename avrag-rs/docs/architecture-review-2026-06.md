# RAG / Chat / WebSearch 策略架构评审与最终决策

> 基于真实 LLM（DeepSeek v4-pro/v4-flash + DashScope Embedding + Brave Search）E2E 测试结果
> 日期：2026-06-01
> 状态：决策已冻结，待按落地顺序执行

---

## 一、最终决策

本次架构调整保留 **Evidence Gate** 作为在线证据门控层，用于在进入生成阶段前判断当前检索结果是否具备回答条件。

RAG 与 WebSearch 继续采用"检索后直接 grounded answer"的主链路，**不再保留独立的 Evaluate 状态**，以避免同一批证据被重复读取和产生伪决策分裂。

`focus mode` 作为可选优化层保留，但仅在**上下文预算紧张**且**召回数量过多、评分集中度不高**时启用，用于对证据进行句段级压缩或筛选，从而降低噪音并控制 token 开销。

Chat 层不增加 `mode_recommendation` 结构化字段，保持为自然语言提示能力，由 chat agent 在回答中主动提醒用户可切换检索模式即可。

评测策略以人工 E2E 回归为主，不引入抽样 judge 作为常规机制，避免增加额外成本与不稳定因素。

---

## 二、设计原则

该方案遵循"**在线简化、离线保守、条件触发优化**"的原则：

- **在线路径尽量减少 LLM 重复读取同一证据的次数**，把模型能力集中在一次高质量 grounded 生成上。
- **离线验证优先依赖真实 E2E 场景和人工判断**，而不是把自动评测作为系统正确性的主要依据。
- **所有增强逻辑都应保持低耦合**，避免把模式识别、证据判断和生成行为混合进同一个 prompt 或状态机分支中。

---

## 三、启用规则

`focus mode` 的触发条件定义为：

- 上下文使用率接近上限。
- 召回结果数量超过预设阈值。
- 评分分布不集中，说明没有明显头部证据，噪音占比偏高。

当上述条件满足时，系统先对检索到的 chunk 做句段级提取或轻量压缩，再进入最终回答阶段。若条件不满足，则直接将通过 Evidence Gate 的证据送入生成器，不额外增加压缩步骤。

这样可以把压缩能力作为"预算保护手段"，而不是默认复杂度。

---

## 四、Evidence Gate 语义

Evidence Gate 是检索与生成之间的门控层，**不读 chunk 内容做语义判断**，只对**检索元数据**做硬性条件检查：

| 检查项 | 阈值 | 不通过时的行为 |
|--------|------|---------------|
| 召回数量 | ≥ 1 | 触发 Evidence Gate 失败：进入 EVIDENCE_TOPIC_MISMATCH 降级分支 |
| 评分集中度 | top-1 score ≥ 阈值（如 0.5） | 集中度不足：进入 focus mode 压缩分支 |
| 上下文预算 | token 使用 < 80% | 超预算：进入 focus mode 压缩分支 |
| 主题相关性 | 文档元数据主题与查询关键词重叠 | 主题不匹配：进入 EVIDENCE_TOPIC_MISMATCH 降级 |

**关键不变量**：Evidence Gate 不调用 LLM，纯代码判断。LLM 能力集中在一次 grounded 生成上。

---

## 五、落地顺序

| 步骤 | 内容 | 依赖 | 验收 |
|:----:|------|------|------|
| **第一步** | 实现 Evidence Gate，确保答复前的证据质量下限稳定 | 无 | 检索为空/主题不匹配时触发降级，不调用 LLM |
| **第二步** | 合并 RAG 与 WebSearch 的 Evaluate/Answer 路径，统一成单次 grounded answer 流程 | Evidence Gate | RAG/WebSearch 不再有独立 Evaluate 状态，chunk 只读一次 |
| **第三步** | 加入 `focus mode` 的条件触发规则，并先在少量高噪音场景验证其收益 | Evidence Gate | focus mode 触发时不破坏现有 grounding 质量 |
| **第四步** | 保留 Chat 的自然语言模式提醒，不增加额外字段或路由状态 | 无 | Chat 面对事实性问题时主动推荐 RAG/Search |
| **第五步** | 以人工 E2E 回归作为主要验收方式，覆盖典型检索、降级和错误路由场景 | 全部 | 18 个策略 E2E + 14 个产品 E2E 全部通过 |

---

## 六、跨策略共性问题（背景）

### 6.1 策略隔离导致规则漂移

**现状**：`rag-answer` 和 `web-grounded-answer` 是两个独立 skill，在 citation 规则、evidence strength 分级、uncertainty handling 上有大量重复文本。

**问题**：改了 RAG 的 citation 格式（如 `[[cite:CHUNK_ID]]` 改成 `[^1]`），Search 没同步 → 前端解析不一致。

**解决方案**：提取公共规则到 `prompts/skills/grounded-answer/`（通用 grounded answer 基础规则：citation 格式、evidence strength、uncertainty 表达）。RAG 和 WebSearch 分别引用并叠加策略特定规则（RAG 的 chunk ID 格式、Search 的 URL 引用格式）。

### 6.2 `degrade_trace` 是自由字符串

**现状**：`DegradeTraceItem.reason` 是 `String`。

**问题**：测试中硬编码匹配 `"budget_exhausted"`、`"NoResultsAfterAllFallbacks"`、`"escalate_to_search: ..."`。产品代码改了措辞，测试断裂。

**解决方案**：定义 `DegradeReason` 强枚举：
```rust
enum DegradeReason {
    BudgetExhausted,
    NoResultsAfterAllFallbacks,
    EvidenceInsufficient,
    EvidenceTopicMismatch,
    ContentGuardBlocked,
    EscalateToSearch(String),
    Other(String),
}
```

### 6.3 共享基础设施的数据污染

**现状**：E2E 测试连接本地持久化 Milvus，所有测试共享同一个 collection。

**问题**：`rag_empty_document_degrades_gracefully` 测试时，Milvus 里已有历史文档（《反脆弱性》《系统架构》等），检索召回 23 条无关结果，测试结果不可复现。

**解决方案**：E2E 测试中每个用例使用独立 collection（`MILVUS_COLLECTION_PREFIX={test_name}_{timestamp}`），测试结束后 `drop_collection`。

---

## 七、附录：各策略详细问题分析

### 7.1 RAG 策略

#### 7.1.1 现状

当前 RAG 采用 ReAct 式 4 步循环：Plan → ExecuteRetrieve → Evaluate → Answer。

- **Plan**：LLM 生成 sub-query 和工具调用计划
- **ExecuteRetrieve**：执行 dense/lexical/graph 检索，召回 chunk
- **Evaluate**：LLM 读 chunk 全文，判定 sufficient / insufficient / give_up
- **Answer**：LLM **再读同一批 chunk 全文**，生成带 citation 的回答

代码上，`evaluate_retrieval_strategy` 函数把 chunk 全文（最多 15 条）塞进 prompt，调用 LLM 做评估。评估通过后，answer synthesizer 再次读取同一批 chunk 生成回答。

#### 7.1.2 问题描述

**同一批 chunk 被 LLM 读两次，费用翻倍。**

Mock 测试中不敏感（mock 不花钱），真实 LLM 环境下：
- Evaluate 一次 ≈ 4000 token（15 个 chunk × 平均 200 字 + system prompt + sub-query 列表）
- Answer 一次 ≈ 6000 token（同一批 chunk + answer SKILL + 用户问题）
- 一次 RAG 问答，chunk 内容被付费读取两次

更严重的是**决策分裂**：
- 评估器判定 sufficient → 系统进入 Answer 状态
- 回答器发现 chunk 其实不够（或主题不相关）→ 只能声明"证据不足"但仍生成回答
- 系统最终标记 `Synthesized`，用户误以为答案来自文档

真实测试案例：用户问"量子纠缠是什么"，上传的文档是《面包烘焙指南》。共享 Milvus 召回 23 条历史文档（反脆弱性、系统架构等）。评估器数到 23 条 → 误判 sufficient。回答器发现问题 → 声明"文档里没有量子物理内容"，但仍用通用知识回答。**系统标记 Synthesized，用户被误导。**

#### 7.1.3 成因

**RAG 被强行套进了 ReAct 的"计划-执行-评估-回答"框架，但 RAG 的任务特性不需要这种硬分层。**

| 任务类型 | 适合分层评估 | 原因 |
|---------|:----------:|------|
| 复杂推理（数学证明、多步分析） | ✅ | 每步需要验证，错了回退 |
| 工具链编排（查天气→查路线） | ✅ | 有明确的中间状态 |
| **RAG 文档问答** | **❌** | 就一步"搜到东西→回答问题"，没有中间状态值得独立验证 |

RAG 的"评估"不是真正的中间验证，它只是**在回答前再读一遍 chunk**。评估器没有独立的判断依据（它看的内容和回答器完全一样），却要在回答器之前做一个二元决策。这个决策要么是冗余的（评估器和回答器结论一致），要么是错误的（评估器被数量迷惑，回答器才发现真相）。

另外，`rag-eval` prompt 明确说 "You do NOT inspect chunk text"，但代码把 chunk 全文塞进 prompt。LLM 收到矛盾指令，行为不可预测。

#### 7.1.4 解决方案（与最终决策一致）

**Evidence Gate + 移除独立 Evaluate 状态。**

- **Evidence Gate**（纯代码）检查召回数量、评分集中度、文档主题元数据，不调 LLM
- 通过则直接进入 grounded answer（单次 LLM 调用）
- 不通过则进入降级分支（EVIDENCE_TOPIC_MISMATCH / EVIDENCE_INSUFFICIENT）
- focus mode 作为可选项，在召回过多且评分分散时触发 chunk 压缩

---

### 7.2 Chat 策略

#### 7.2.1 现状

Chat 策略是最简单的两步循环：Plan → Answer。

- **Plan**：`chat-plan` 分析用户意图，输出 `action`（answer / clarify）和可选的 `calls`（calculator 等工具）
- **Answer**：`chat` skill 生成对话回答

`chat-plan` 只有两个输出分支：直接回答 或 要求澄清。没有模式路由或推荐。`format_hint` 是 HTTP 层参数，直接注入 answer prompt，planner 不感知。

#### 7.2.2 问题描述

**Chat 没有"导医台"功能：该推荐 RAG/Search 的时候不推。**

用户问："我们公司上季度的营收是多少？"
- Chat 直接用自己的训练数据胡诌，或者说"我不知道"
- 用户不知道产品的 RAG（查上传文档）和 WebSearch（搜公开财报）可以帮他
- 产品最值钱的 grounded 检索能力被闲置

**Chat 听不懂格式意图。**

用户说"给我做个 PPT 总结"，Chat 应该自动识别这是 `presentation-html` 格式需求。但 `chat-plan` 不感知格式关键词，`chat` answer skill 也没有格式检测规则。格式输出完全依赖 HTTP 层的 `format_hint` 参数硬注入。

#### 7.2.3 成因

Chat 被设计为"通用对话助手"，prompt 中虽然有 *"when the user's goal fits RAG or Web Search better, briefly say so"*，但这只是 answer agent 的弱提示，不是 planner 的强路由。系统没有机制让 Chat 主动识别"这个问题需要查文档"或"这个问题需要上网搜"。

格式技能注入是 HTTP 层和 answer prompt 的硬编码耦合，planner 不参与决策。如果用户通过对话口头要求格式（而非 API 参数），系统无法响应。

#### 7.2.4 解决方案（与最终决策一致）

**保持 Chat 为自然语言提示，不增加结构化字段。**

- 在 `chat-plan` 和 `chat` answer skill 的 prompt 中增强"模式边界意识"：面对需要 RAG/Search 的事实性问题，主动说"我没有实时数据/文档权限，要不要我帮你搜？"
- 格式关键词检测放在 `chat` answer skill 中（"PPT"→presentation-html，"teach me"→step-by-step-tutor）
- **不改代码**，所有改动都在 prompt 层

---

### 7.3 WebSearch 策略

#### 7.3.1 现状

WebSearch 采用和 RAG 相同的 ReAct 式 4 步循环：Plan → ExecuteSearch → Evaluate → Answer。

- **Plan**：`web-search-planner` 生成 1-3 个 sub-query
- **ExecuteSearch**：调用 Brave Search API，获取 web 结果
- **Evaluate**：`web-search-coverage-eval` 读搜索结果，判定 sufficient / insufficient
- **Answer**：`web-grounded-answer` **再读同一批结果**，生成带 citation 的回答

`LoopBudget` 只记录迭代次数，不做 token 级预算分配。`web-search-planner` prompt 完全不感知 budget。

#### 7.3.2 问题描述

**Budget 爆炸：搜索-评估-replan 循环失控。**

用户问："今天最新的 AI 新闻"
1. planner 生成 sub-query
2. Brave 搜索返回结果
3. evaluator 判定 insufficient（结果不够全面）
4. replan → 新 sub-query
5. evaluator 又判定 insufficient
6. 想触发第三轮 → **budget 耗尽** → 系统报错降级，用户什么都没得到

Mock 测试不会暴露（mock 不花钱），真实 LLM 环境下：planner + evaluator + replan 每次都要调 LLM，三次循环 token 费用就吃光预算。

**同一批搜索结果被读两次**，和 RAG 一样的浪费。

**Evaluator 只看数量，不看 snippet 质量。** `web-search-coverage-eval` 的设计和 `rag-eval` 一样，只看 sub-query 和 result count。但 web 结果天然碎片化，一个 sub-query 返回 10 条结果，可能 3 条相关、7 条噪音。数量够了，质量不够。

#### 7.3.3 成因

**和 RAG 同一个病根：强行把"评估"和"回答"拆成两步。**

WebSearch 的任务特性比 RAG 更简单 — 没有多工具编排，就一次 Brave API 调用。但系统给它套了同样的 4 步循环：
- Evaluate 读一遍搜索结果（付费）
- Answer 再读一遍（再付费）
- 中间还多了一次 replan（再付费）

**没有预算保护机制**是第二病根。`LoopBudget` 只数"迭代了几轮"，不估算"还剩多少 token"。系统不知道"我已经花了 80% 预算在评估上，剩下的只够生成一句话"。

#### 7.3.4 解决方案（与最终决策一致）

**Evidence Gate + 移除独立 Evaluate 状态 + 搜索最多 2 轮止损。**

- **Evidence Gate** 检查 Brave 返回结果的数量、评分集中度、URL 域名可信度
- 通过则直接进入 grounded answer（单次 LLM 调用）
- 不通过则触发降级（DEGRADED_BRAVE_EMPTY_RESULT）
- **搜索最多 2 轮**硬约束：超过 2 轮即使结果不完美也合成答案给用户
- focus mode 在 web 结果过多时压缩 snippet

---

## 八、执行优先级（按落地顺序）

| 步骤 | 策略 | 改动类型 | 内容 |
|:----:|------|---------|------|
| **1** | RAG | 代码 | 实现 Evidence Gate（纯代码，不调 LLM） |
| **2** | RAG + WebSearch | 代码 + prompt | 移除独立 Evaluate 状态，合并为 grounded answer |
| **3** | RAG + WebSearch | 代码 | focus mode 条件触发逻辑 |
| **4** | Chat | prompt | 自然语言模式提醒 + 格式关键词检测 |
| **5** | 全策略 | 验收 | 18 个策略 E2E + 14 个产品 E2E 全部通过 |

跨策略 P3 项（不阻塞主流程）：
- `DegradeReason` 强枚举
- E2E 独立 Milvus collection
- `grounded-answer` 公共规则提取

---

## 九、下一步

1. ✅ 文档已记录（含最终决策、设计原则、启用规则、落地顺序）
2. ⬜ 按落地顺序执行：Evidence Gate → 合并 Evaluate/Answer → focus mode → Chat prompt
3. ⬜ 每个改动补充对应的 E2E 测试覆盖
4. ⬜ 第五步人工 E2E 回归作为最终验收
