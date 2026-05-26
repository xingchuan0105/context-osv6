# 写作风格库 + Brainstorming 技能 + Agent 自治多轮对话记忆 设计文档

> Date: 2026-05-26
> Status: Design Approved

## 目录

- [1. 写作风格库 (Writing Style Library)](#1-写作风格库-writing-style-library)
  - [1.1 目标](#11-目标)
  - [1.2 核心设计](#12-核心设计)
  - [1.3 目录结构](#13-目录结构)
  - [1.4 SKILL.md 格式](#14-skillmd-格式)
  - [1.5 系统改动](#15-系统改动)
  - [1.6 检测逻辑](#16-检测逻辑)
- [2. Brainstorming 技能](#2-brainstorming-技能)
  - [2.1 核心定位](#21-核心定位)
  - [2.2 触发机制](#22-触发机制)
  - [2.3 交互流程](#23-交互流程)
  - [2.4 退出条件](#24-退出条件)
- [3. Agent 自治多轮对话记忆](#3-agent-自治多轮对话记忆)
  - [3.1 核心理念](#31-核心理念)
  - [3.2 数据库层](#32-数据库层)
  - [3.3 新工具定义](#33-新工具定义)
  - [3.4 交互流程](#34-交互流程)
  - [3.5 与现有 10 轮窗口的关系](#35-与现有-10-轮窗口的关系)
  - [3.6 潜在问题与缓解](#36-潜在问题与缓解)
- [4. 三个设计的协同关系](#4-三个设计的协同关系)
- [5. 实现优先级](#5-实现优先级)

---

## 1. 写作风格库 (Writing Style Library)

### 1.1 目标

让 Agent 在 Answer 阶段能够根据用户查询的语境，自主选择并应用一种或多种写作风格，从而生成符合用户预期的回复。写作风格与 Format Skill 的区别在于：Format Skill 控制**输出格式**（PPT JSON、HTML 代码块），而写作风格控制**语言风格**（简洁、学术、讲故事）。

**核心原则：Agent 自治。** 不硬编码关键词匹配规则，由 Plan LLM 自主判断需要加载哪些写作风格。

### 1.2 核心设计

- **复用现有 Skill 系统**：写作风格是 Skill 的一种，通过 `category: "writing-style"` 与现有 Skill 区分。
- **三模式通用**：Chat、RAG、Search 的 Answer 阶段均支持。
- **可多选叠加**：用户可以同时激活"简洁+学术"两种风格（与 Format Skill 的单选不同）。
- **内容结构**：每个风格 Skill 包含 `NO-LIST`（禁止做的事）、`YES-LIST`（必须做的事）、`references/` 目录下的 few-shot 示例。

### 1.3 目录结构

```
prompts/skills/
├── chat-plan/                    (现有)
├── chat/                         (现有)
├── rag-plan/                     (现有)
├── ...
├── ppt-generation/               (现有 format skill)
├── html-renderer/                (现有 format skill)
├── teaching/                     (现有 format skill)
├── framework-extraction/         (现有 format skill)
├── concise-writing/              ← 新增
│   ├── SKILL.md
│   └── references/
│       ├── few-shot-1.md
│       └── few-shot-2.md
├── academic-writing/             ← 新增
│   ├── SKILL.md
│   └── references/
│       └── few-shot-1.md
└── storytelling/                 ← 新增
    ├── SKILL.md
    └── references/
        └── few-shot-1.md
```

### 1.4 SKILL.md 格式

以 `concise-writing` 为例：

```markdown
---
name: concise-writing
description: "Load when the user prefers brief, direct answers without fluff"
version: "1.0"
depends: []
category: "writing-style"           ← 新增字段，与 "standard" / "format" / "behavior" 区分
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
---

You must write in a concise, direct style. Follow these rules:

## NO-LIST (Never do these)
- Do NOT use filler phrases like "It is important to note that..."
- Do NOT repeat the same point in different words
- Do NOT include unnecessary background unless explicitly asked
- Do NOT use more than 3 sentences per paragraph unless the topic demands depth

## YES-LIST (Always do these)
- Start with the answer, then explain if needed
- Use bullet points for lists of 3+ items
- One idea per sentence

## Few-shot Examples
{{ref:few-shot-1}}
{{ref:few-shot-2}}
```

### 1.5 系统改动

#### 1.5.1 Skill Frontmatter 解析器

**文件**: `crates/app/src/agents/progressive/skill_frontmatter.rs`

- 新增 `category` 字段（可选，默认 `"standard"`）。
- 现有 Skill 不受影响。

#### 1.5.2 CapabilityRegistry

**文件**: `crates/app/src/agents/capability/registry.rs`

新增方法：

```rust
pub fn answer_writing_styles(&self, strategy: &str) -> Vec<&SkillMetadata> {
    self.skills
        .iter()
        .filter(|s| s.category == "writing-style")
        .filter(|s| s.applicable_strategies.contains(strategy))
        .collect()
}
```

#### 1.5.3 PromptBuilder

**文件**: `crates/app/src/agents/strategy/prompts.rs`

`build_answer_system_prompt` 增加 `selected_writing_styles` 参数：

```rust
pub fn build_answer_system_prompt(
    answer_skill_id: &str,
    strategy: &str,
    selected_format_skills: &[String],
    selected_writing_styles: &[String],   // ← 新增
) -> String {
    // 1. answer skill body（基底）
    // 2. format skills 目录（Index tier）
    // 3. 选中的 format skill 全文（Load tier）
    // 4. 写作风格 skill 全文（Load tier） ← 新增
    for style_id in selected_writing_styles {
        if let Some(skill) = registry.skill(style_id) {
            parts.push(skill.system_prompt().to_string());
        }
    }
}
```

#### 1.5.4 PlannerOutput 扩展

**文件**: 各策略的 Planner Output 结构

```rust
pub struct PlannerOutput {
    pub decision: PlannerDecision,
    pub skills: Vec<String>,                  // format skills
    pub writing_styles: Vec<String>,          // ← 新增
    pub behavior_mode: Option<String>,        // ← 新增（Brainstorming 用）
    pub calls: Vec<ToolCall>,
}
```

### 1.6 检测逻辑

**Agent 自治**：Plan LLM 根据用户查询自主判断是否需要加载写作风格，以及加载哪些。

Plan 阶段的 system prompt 中增加说明：

```
Available writing styles: [concise-writing, academic-writing, storytelling, ...]
If the user's request implies a preferred writing style, include the corresponding style IDs
in the `writing_styles` field. Multiple styles can be combined.
```

**示例**：
- 用户："简单说一下" → Plan LLM 输出 `writing_styles: ["concise-writing"]`
- 用户："用学术论文的格式详细分析" → Plan LLM 输出 `writing_styles: ["academic-writing"]`
- 用户："讲个故事" → Plan LLM 输出 `writing_styles: ["storytelling"]`

---

## 2. Brainstorming 技能

### 2.1 核心定位

不是独立 Strategy，而是**行为模式 Skill**。当 Agent 检测到用户输入模糊、不完整、或包含探索性意图时，加载此 Skill，改变 Answer 阶段的行为：从"直接给答案"变为"一步一步澄清需求"。

### 2.2 SKILL.md 结构

```markdown
---
name: brainstorming
description: "Load when the user's request is vague, underspecified, or exploratory"
version: "1.0"
depends: []
category: "behavior"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
---

You are in brainstorming mode. The user has asked something vague or exploratory.
Your job is NOT to give a final answer immediately. Instead, follow this protocol:

## Protocol

### Step 1: Identify what's missing
Analyze the user's request and identify:
- What goal are they trying to achieve? (state your understanding)
- What constraints or preferences are unstated?
- What scope decisions need to be made?

### Step 2: Ask clarifying questions (max 2 per turn)
Present 1-2 focused questions that would most reduce ambiguity. Each question should:
- Be multiple-choice when possible (easier for user to answer)
- Cover the most consequential uncertainty first
- Avoid asking everything at once

### Step 3: Synthesize and confirm
After the user answers, restate your understanding in this format:
```
Based on what you've told me:
- Goal: [summarized goal]
- Constraints: [summarized constraints]
- Approach I'm considering: [your proposed approach]

Does this look right? If yes, I'll proceed. If not, tell me what to adjust.
```

### Step 4: Exit brainstorming
Only after explicit user confirmation ("yes", "looks good", "proceed") do you switch
back to normal answer mode and provide the actual solution.

## NO-LIST
- Do NOT give a full answer while in brainstorming mode
- Do NOT ask more than 2 questions in one turn
- Do NOT assume preferences that the user hasn't stated
- Do NOT exit brainstorming without explicit user confirmation

## Examples
{{ref:example-vague-request}}
{{ref:example-clarification-flow}}
```

### 2.3 触发机制

**Agent 自治**：Plan LLM 自主判断是否需要进入 Brainstorming 模式。

Plan LLM 的输出增加 `behavior_mode` 字段：

```rust
pub struct PlannerOutput {
    pub decision: PlannerDecision,
    pub skills: Vec<String>,
    pub writing_styles: Vec<String>,
    pub behavior_mode: Option<String>,   // "brainstorming" or None
    pub calls: Vec<ToolCall>,
}
```

触发信号（由 Plan LLM 自主识别，非硬编码）：
- 用户查询缺少关键信息
- 用户表达模糊（"我想做个东西"、"帮我看看这个"）
- 用户主动要求探索（"帮我 brainstorm 一下"）

### 2.4 交互流程

```
User: "我想做个东西" (模糊)
  ↓
Plan LLM 判断 → behavior_mode: "brainstorming"
  ↓
Answer 加载 brainstorming skill
  ↓
Agent: "听起来你想启动一个项目。为了帮你更好地规划，请确认两点：
        1. 这是编程项目还是内容创作？ [编程/内容/其他]
        2. 你更关注快速原型还是长期可维护性？ [速度/质量/平衡]"
  ↓
User: "编程，速度优先"
  ↓
Plan LLM 判断 → behavior_mode: "brainstorming" (继续)
  ↓
Agent: "确认：你要做一个编程项目，优先快速原型。我建议用 Python + 快速脚手架。
        对吗？对的话我开始写代码。"
  ↓
User: "对的"
  ↓
Plan LLM 判断 → behavior_mode: None
  ↓
Answer 恢复正常模式，提供实际解决方案
```

### 2.5 退出条件

**Agent 自治**：Plan LLM 每次根据对话上下文决定 `behavior_mode` 是继续 `"brainstorming"` 还是 `None`。

没有硬编码规则（如"用户说'对'就退出"），Agent 自己读上下文判断。通常当以下情况满足时退出：
- 用户目标已明确
- 关键约束已确认
- 用户显式要求继续（"对的"/" proceed"）

---

## 3. Agent 自治多轮对话记忆

### 3.1 核心理念

外围系统只做两件事：
1. **提供读取接口** — 按标签过滤返回历史消息
2. **提供写入接口** — 执行 Agent 输出的标签操作

**其他一切由 Agent 决定**：何时读、读多少、如何分类、标签叫什么、何时改标签。

**不做 embedding / 向量检索**。按需检索，Agent 决定何时读取历史。

### 3.2 数据库层

现有 `chat_messages` 表**不变**。新增独立标签表：

```sql
CREATE TABLE message_tags (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGINT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(message_id, tag)
);

CREATE INDEX idx_message_tags_tag ON message_tags(tag);
CREATE INDEX idx_message_tags_message_id ON message_tags(message_id);
```

**设计意图**：标签与消息解耦。一个消息可以有多个标签，标签可以增删改，不影响消息本身。

### 3.3 新工具定义

注册到 CapabilityRegistry，Plan 阶段可用。

#### 3.3.1 `conversation_history_load`

```json
{
  "name": "conversation_history_load",
  "description": "Load previous messages from this session. Use without tags to load all messages for initial analysis. Use with tags to recall specific topics.",
  "parameters": {
    "tags": {
      "type": "array",
      "items": "string",
      "optional": true
    },
    "limit": {
      "type": "integer",
      "optional": true,
      "default": 20,
      "description": "Max messages to load"
    }
  }
}
```

#### 3.3.2 `conversation_history_tag`

```json
{
  "name": "conversation_history_tag",
  "description": "Tag messages with descriptive, specific labels. Every loaded message should receive at least one tag. Tags should distinguish topics clearly (e.g., 'Rust并发模型分析' not just '技术').",
  "parameters": {
    "operations": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "message_id": "integer",
          "action": "enum: add | remove | replace",
          "tags": ["array of string"]
        }
      }
    }
  }
}
```

### 3.4 交互流程

#### 3.4.1 首次分析（Session 有未分类历史）

```
Turn N: 用户查询需要历史上下文
  ↓
Plan LLM 判断 → 输出 tool call:
  conversation_history_load(tags: null, limit: 20)
  ↓
Execute: 返回该 session 的全部历史（最多 20 条，带现有标签）
  ↓
Plan LLM 读取历史内容
  ↓
Plan LLM 输出 tool call:
  conversation_history_tag(
    operations: [
      {message_id: 1, action: "add", tags: ["Rust并发写作", "文章大纲规划"]},
      {message_id: 2, action: "add", tags: ["Rust并发写作", "技术细节展开"]},
      {message_id: 3, action: "add", tags: ["Rust并发写作", "示例代码"]},
      {message_id: 4, action: "add", tags: ["天气查询", "深圳今日天气"]},
      {message_id: 5, action: "add", tags: ["闲聊", "问候"]},
    ]
  )
  ↓
Execute: 写入 message_tags 表
  ↓
Plan LLM 继续正常规划（基于已加载的历史上下文）
```

#### 3.4.2 后续按需加载（标签已存在）

```
Turn M: 用户："回到文章继续写"
  ↓
Plan LLM 判断 → 输出 tool call:
  conversation_history_load(tags: ["Rust并发写作"], limit: 10)
  ↓
Execute: 返回 message 1, 2, 3（匹配标签）
  ↓
Plan LLM 读取这些历史，继续规划
  ↓
（如果新消息未分类，Plan LLM 可能再输出 tag 操作）
```

#### 3.4.3 标签更新（Agent 发现分类不准）

```
Turn K: Agent 重读历史后发现 message_4 不只是天气，还涉及出行规划
  ↓
Plan LLM 输出:
  conversation_history_tag(
    operations: [
      {message_id: 4, action: "replace", tags: ["天气查询", "深圳今日天气", "出行规划"]}
    ]
  )
  ↓
Execute: 更新标签
```

### 3.5 与现有 10 轮窗口的关系

**方案：完全替代。**

取消 `MAX_PROMPT_HISTORY_TURNS = 10` 的硬编码注入。`AgentRequest.messages` 只包含当前轮次的用户输入。所有历史上下文由 Agent 通过 `conversation_history_load` 按需加载。

**原因**：10 轮窗口是"系统预设 Agent 需要什么"，与 Agent 自治理念冲突。让 Agent 自己决定加载哪些历史，比系统硬塞最近 10 轮更智能。

**Fallback**：如果 Agent 从未调用 `load_history`，prompt 中只有当前查询（相当于没有历史）。这对新 session 是合理的。

### 3.6 潜在问题与缓解

| 问题 | 缓解 |
|------|------|
| 首次加载 20 条可能超 token | `limit` 参数由 Agent 控制，默认 20，Agent 可调小 |
| Agent 忘记打标签 | Plan prompt 明确要求 "Every loaded message should receive at least one tag" |
| 标签膨胀（太多标签） | Agent 自主合并相似标签，系统不限制数量 |
| 全量加载 latency | 首次加载是异步的，不影响用户体验；后续按标签加载很快 |
| 与现有 E2E 测试冲突 | 10 轮窗口是测试依赖，替换后需要更新测试 |
| Agent 误分类 | Agent 可在后续轮次重新读取并修正标签（`replace` 操作） |

---

## 4. 三个设计的协同关系

```
User Request
    ↓
Plan 阶段
    ├── 检测 behavior_mode → "brainstorming"? → 加载 Brainstorming Skill
    ├── 检测 writing_styles → 哪些写作风格? → 注入 Answer Prompt
    ├── 检测需要历史? → conversation_history_load → Agent 读取 + 打标签
    └── 输出 PlannerDecision (tools, state transition)
Execute 阶段
    └── 执行工具调用（含 conversation_history_tag 写入标签）
Evaluate 阶段 (RAG/Search)
    └── 评估结果
Answer 阶段
    ├── 注入 answer skill body
    ├── 注入 selected format skills
    ├── 注入 selected writing styles ← 新增
    └── 生成回复
```

**统一原则：Agent 自治。**
- 写作风格：Plan LLM 自己选
- Brainstorming：Plan LLM 自己判断何时进入/退出
- 多轮记忆：Plan LLM 自己决定读什么、怎么分类

---

## 5. 实现优先级

| 优先级 | 功能 | 理由 |
|--------|------|------|
| P0 | 写作风格库 | 改动最小，复用 Skill 系统，15 行代码 + 新增 Skill 目录 |
| P1 | Brainstorming 技能 | 独立 Skill，不影响现有流程，风险低 |
| P2 | Agent 自治多轮记忆 | 涉及数据库迁移、工具注册、10轮窗口替换，改动面大 |

建议按 P0 → P1 → P2 的顺序实现，每完成一个验证后再做下一个。
