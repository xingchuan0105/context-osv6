# 编排 / 能力 提示词工程优化（2026-07-20）

> **SUPERSEDED** — 本文描述的 orchestrator / worker / brief / handoff 多 agent 架构已被取代：2026-07-30 起产品路径改为单 agent（SaC 设计，见根 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），orchestrator 代码已于 2026-08-01 物理删除（commit `7f2d182d`）。本文仅作历史记录。（横幅添加于 2026-08-02 文档体系梳理）

> **Runtime 结构已被 Option D 取代（2026-07-20 hard cutover）。**  
> 有检索时的「独立 Chat exit 子 agent」终态 → **同一 Product Agent 运行时 Answer 相位换装**。  
> 本文 **P0–P3 提示词装配结论仍有效**（chat-base、capability 派发节、积木、pure-chat 入口绕过）；  
> 运行时形态与验收以 [`2026-07-20-unified-product-agent-option-d.md`](./2026-07-20-unified-product-agent-option-d.md) 与  
> [`2026-07-20-option-d-test-gap-and-drift.md`](./2026-07-20-option-d-test-gap-and-drift.md) 为准。下文若写「Chat exit」请读作 **Answer phase**。

| 项目 | 内容 |
|------|------|
| 状态 | **P0–P3 + 删扁平兼容路径已落地（2026-07-20）**；**runtime 见 Option D cutover** |
| 范围 | 何时走编排链路、Chat 提示词如何拼接、编排器 system、capability 多选装配、agent-base 去留 |
| 不在范围 | CDS cluster 路由、ingestion pipeline 提示词、改 tool schema 语义 |
| 关联 | `prompts/README.md`、`docs/agents/cds-v1.1.md`、`mode_assemble.rs`、`orchestrator/brain.rs`、`orchestrator/host.rs`、`chat/pipeline_steps.rs`、Option D 设计稿 |
| 前置共识 | 产品可勾选「工作区检索 / 公网检索」；有检索时走「派活 → 检索同事 → 写答案」；纯聊天入口不进编排 |
| 已拍板 | **A. 纯聊天入口级绕过编排**（2026-07-20） |

---

## 0. 何时启用编排（已拍板）

用产品勾选决定路径，而不是「全局开关一开、所有请求都进编排壳子」。

| 用户勾选 | 走哪条路 |
|----------|----------|
| **都不勾**（纯聊天） | **入口直接 Chat**。不创建编排 agent、不跑检索同事。 |
| **只勾工作区** / **只勾公网** / **两个都勾** | **永远编排链路**：协调者写任务 → 检索同事 → Chat 写最终回答（可选 V2 多跳脑）。 |

```text
dispatch 入口
  │
  ├─ 未勾选任何检索  ──►  pure chat（chat-base only）
  │
  └─ 勾选了 rag 和/或 search  ──►  编排 host（worker + chat exit；V2 则多跳）
```

**已删除：** `AGENT_ORCHESTRATOR_V1=0` 时的扁平单 agent 旁路（`run_rag_mode` / `run_search_mode`）。该 flag 不再决定产品路径，仅保留 env 探测兼容。

**单选也要编排：** 只要勾了任一检索，就「检索同事 + 写答案」拆开；不是只有双选才开编排。

---

## 0.1 Chat 提示词怎么拼（白话 + 已认方向）

### 两种上班方式

1. **用户没开检索** → 只要 **聊天系统提示词**。  
2. **前面查过资料，轮到你写最终答案** → 聊天系统 + **按本轮材料类型选的作答规则**（见下，**不能**三种场景共用一份大杂烩）。

### 为什么不能「一份根据材料写答案」打天下

单 RAG、单 Web、RAG+Web 在作答规则上**不一样**：

| | 仅工作区 | 仅公网 | 工作区 + 公网 |
|--|----------|--------|----------------|
| 材料从哪来 | 文档段落 | 网页 | 两类都有 |
| 引用长什么样 | 文档/chunk 类标记 | 网页序号/URL 类标记 | **两套都要会，且不能混用** |
| 冲突时怎么写 | 以文档为准即可 | 以网页为准、注意时效 | 必须说明 **文档说什么 / 网页说什么**，冲突显式写 |
| 没查到时 | 「库里没有」 | 「网上没检到」 | 分侧说清，禁止用一侧顶另一侧 |
| 多余条款 | 若塞进公网规则 → **冗余且干扰** | 若塞进文档规则 → 同左 | 若只用单侧模板 → **说不清** |

结论：**一种万能「根据材料写答案」要么冗余，要么说不清楚。** 必须按场景选模板，或由小块组合出三种场景。

### 推荐：小块组合（三种场景 = 不同积木）

| 积木 | 何时拼进 Chat | 写什么（白话） |
|------|----------------|----------------|
| **聊天系统** | 永远 | 你是谁、怎么说话 |
| **共用：听写作要求** | 凡「根据材料写最终答案」 | 有协调者写作说明则遵守口径；不要自己改题意；不要假装再去检索 |
| **工作区作答** | 本轮材料里**有**工作区证据 | 只用文档材料；文档引用格式；库没有就说没有 |
| **公网作答** | 本轮材料里**有**公网证据 | 只用网页材料；网页引用格式；没检到就说没检到 |
| **双源对照** | **同时**有工作区 + 公网材料 | 分类陈述；冲突并陈；禁止用网页冒充文档或反之 |

拼出来的三种完整形态：

```text
仅工作区写答案：
  聊天系统 + 听写作要求 + 工作区作答

仅公网写答案：
  聊天系统 + 听写作要求 + 公网作答

工作区+公网写答案：
  聊天系统 + 听写作要求 + 工作区作答 + 公网作答 + 双源对照

纯聊天 / 本轮决定不查直接聊：
  仅 聊天系统
```

**选型依据（重要）：** 以 **本轮实际带给 Chat 的材料种类** 为准（有没有工作区条、有没有网页条），不要只看用户勾选。  
例：用户勾了双开，但公网整轮失败、只有文档材料 → 按「仅工作区」拼，不要塞公网条款。

可选实现：

| 做法 | 说明 |
|------|------|
| **A. 四文件组合（推荐）** | `chat-system` + `answer-from-workspace` + `answer-from-web` + `answer-dual-source`；「听写作要求」可并入各 answer 头或单独 `answer-follow-brief` 极短块 |
| B. 三份完整模板 | `answer-rag-only` / `answer-web-only` / `answer-rag-web` 各写全 | 三种场景最清楚，但共用句会复制三份，改一处易漏 |
| C. 一份大文件分三章 | 运行时按场景只注入对应章 | 单文件维护，切分要稳 |

**默认采用 A。** 与现有 `prompts/synthesis/rag-answer.md`、`search-answer.md` 的关系：可收敛为上述积木，或由积木替代「Chat exit 阶段」的注入内容；避免 Chat exit 再误用「单 agent 自检索自答」的整份 monomode 提示词。

协调者**这一题的具体**写作要求（怎么理解题、比哪几维）仍是 **本轮动态附文**，不是固定文件。

### 不要塞进 Chat 的

- 检索同事怎么查库、怎么上网（检索同事自己的提示词）  
- 协调者怎么决定再派一轮（Chat 不能派活）

---

## 1. 问题陈述

当前提示词**文件在、接线错位**，导致：

1. **编排器不知道能力是什么**——`capability-rag.md` / `capability-search.md` 只注入 worker，**从不**进入 orchestrator system。
2. **`orchestrator-base.md` 写错角色**——写成「决定分配范式 + 产品黑话」，而不是「接 query → 写具体 brief → 够了再交 Chat」的工作过程。
3. **`agent-base.md` 对有能力路径多余**——与 capability 头几行重复；纯 chat 仍需要自包含底座，不能无替代地删。
4. **双轨 system 源**——`modes/*.yaml` 的 `system_prompt_base`（`rag-system` / `search-system` / `chat-system`）与产品 `system_prompt_parts`（`agent-base` + `capability-*`）并存，文档与心智模型易混。

### 1.1 现状接线（代码事实）

```text
Orchestrator V2 brain
  system = orchestrator-base.md
         + 运行时「本轮状态」（通道名 / 源文档 / 预算 / 已派发 / 证据条数）
  ✗ 不加载 capability-*

RAG / Search worker（host.run_channel）
  system_prompt_parts = agent-base + capability-{rag|search}
                      + 内联 Task brief / handoff 约束

Chat exit（host.run_chat）
  system_prompt_parts = agent-base   # pure chat assemble
  + synthesis skill（chat 等）在 ReAct 合成阶段注入

扁平路径（无编排 / assemble_mode）
  同 worker：agent-base + 可选 capability-*
```

关键实现：

| 位置 | 行为 |
|------|------|
| `app-chat/src/orchestrator/brain.rs` → `render_system_message` | 只拼 `base_prompt` + 本轮状态 |
| `app-chat/src/mode_assemble.rs` | `system_prompt_parts = [agent-base] + capability-*` |
| `agent-loop/.../assembler.rs` → `load_assembled_system_base` | **优先** `metadata.system_prompt_parts`，否则 `mode.system_prompt_base` |
| `modes/rag.yaml` 等 | `system_prompt_base: rag-system.md` 等，主路径常被 parts 覆盖 |

### 1.2 错误心智模型（已纠正）

| 旧理解 | 实际 |
|--------|------|
| capability 注入**编排器** | 只注入 **worker / 扁平 agent** |
| `rag-system` / `search-system` = 当前 subagent 主提示词 | monomode / fallback；主路径是 **agent-base + capability-*** |
| `chat-system` = 编排后合成答案专用提示词 | chat **mode** monomode 底座；合成契约在 `prompts/synthesis/*` |
| orchestrator 只选「范式」 | 代码与 tool 是在**分配具体任务**（`goal` / `instruction`） |

---

## 2. 设计原则

1. **零产品黑话默认**：提示词不得假设模型认识 `orchestrator`、`物化`、`Chat exit`、`O1/O2`、`capability` 等内部词；若必须出现，先用自然语言定义一次。
2. **角色 = 工作过程**：每个 agent 的 system 用「你会收到什么 → 逐步做什么 → 产出什么」书写，而不是「你是 X 架构组件」。
3. **谁写 brief，谁要懂通道**：编排器必须看到**派发视角**的能力说明；worker 看到**执行视角**的协议。两者可同源分节，不可只给 worker。
4. **自包含优先于薄拼接**：能力说明文件应可独立作为 worker system；避免「空 base + 半份能力」叠床架屋。
5. **信息分层，避免灌爆**：编排器不需要 codegen SDK 全文；只要「能查什么 / 不能查什么 / brief 必填什么 / 返回长什么样」。
6. **单源真值**：同一规则不在 orchestrator-base、capability、cluster 三处各写一遍（Perplexity / 既有 prompts 优化纪要同一原则）。

---

## 3. 目标装配

```text
用户 query
    │
    ▼
┌─ 编排 agent（V2 brain）────────────────────────────────┐
│ system =                                               │
│   orchestrator-base.md     # 工作流 + 工具用法（无黑话）  │
│   + 【派发视角】已开启能力说明（按物化通道）              │
│   + 运行时本轮状态（文档列表、预算、证据计数…）           │
│ tools = delegate_rag / delegate_search /               │
│         evidence_fetch / delegate_chat / memory?       │
└───────────────┬────────────────────────────────────────┘
                │ goal / instruction
     ┌──────────┴──────────┐
     ▼                     ▼
┌─ RAG worker ──┐   ┌─ Search worker ─┐
│ capability-   │   │ capability-     │
│ rag.md 全文   │   │ search.md 全文  │
│ （执行协议）  │   │ （执行协议）    │
└───────┬───────┘   └────────┬────────┘
        │ handoff            │
        └──────────┬─────────┘
                   ▼
        ┌─ Chat exit ─────────────────────┐
        │ chat 自包含底座                  │
        │ + synthesis/*                   │
        │ + handoff（口径 + 证据列表）     │
        └─────────────────────────────────┘
```

### 3.1 文件职责（目标）

| 文件 | 读者 | 职责 |
|------|------|------|
| `orchestrator-base.md` | 编排 agent | 工作过程；何时派谁；brief / instruction 写法；结束条件；禁止越权检索与写用户长文 |
| `capability-rag.md` | **双读者** | **§ 派发视角**（给编排）+ **§ 执行协议**（给 worker）；或正文执行 + 独立摘要节由代码抽给编排 |
| `capability-search.md` | 同上 | 同上 |
| `chat` 底座 | Chat exit / pure chat | 自包含对话角色（可从现 `agent-base` + `chat-system` 收敛）；**不**依赖 agent-base 拼接 |
| `synthesis/*` | 合成阶段 | 输出契约与 cite 形态；不复述检索协议 |
| `rag-system.md` / `search-system.md` / `chat-system.md` | 过渡 | **收敛为 deprecated 或与 capability 合并**；主路径不再依赖 |
| `agent-base.md` | 过渡 | **删除或降为 0 行**；身份句并入各自包含文件 |

### 3.2 编排器 system 必须包含的「能力说明」最小集

对每个**已物化**通道，注入（摘自 capability，非 tool description 三言两语）：

| 维度 | RAG 示例 | Search 示例 |
|------|----------|-------------|
| 能做什么 | 工作区内已入库文档的检索与抽取 | 公网检索与抓取 |
| 不能做什么 | 不能当互联网；未见 observation 不当事实 | 不能当工作区文档库 |
| brief 必写 | 文档身份/结构线索 + 要抽什么（自包含） | 可独立成立的检索主题（默认中英） |
| 返回形态 | handoff：`summary` / `key_facts` / `coverage` / `gaps` | 同 schema |
| 何时再派 | `coverage≠full` 或 `gaps` 非空且预算允许 | 同左；空结果可换 goal 一次 |

**禁止**：把 worker 的整份 codegen skill、完整 cite 细则灌进编排器每轮 system（token 与噪声）。

### 3.3 `orchestrator-base.md` 目标文案结构（纲要）

```markdown
## 你是谁
用用户的话理解并拆任务的协调者。你不自己查文档、不自己上网、不写给用户看的最终长文。

## 你会收到
- 用户问题
- 本轮可用的检索通道说明（见后附）
- 工作区里有哪些文档（若有）
- 每轮刷新的进度：已派发结果、证据条数、剩余轮次

## 工作过程
1. 读懂用户问题；指代不清时先用记忆工具（若有），再写任务。
2. 给「工作区检索」和/或「公网检索」写**自包含任务说明**并派发（一次可只派一步，看结果再走）。
3. 根据返回的覆盖度与缺口决定：再派、换写法、或结束检索。
4. 证据够了或轮次将尽：给「写答案的同事」写写作说明（必须写清：你怎么理解原问题、证据怎么组织、哪些维度已覆盖/未覆盖），然后移交。

## 任务说明怎么写
- 禁止把用户原话原样转发；必须消解「这篇/该/它」。
- 工作区任务：文档是谁、结构从哪来、要抽出什么。
- 公网任务：脱离工作区也能成立的查询主题。

## 禁止
- 编造检索结果；未派发的通道当已查过。
- 在未对每个可用检索通道至少派发一次前结束（由运行时校验，你仍应按此规划）。
```

（落地时以中文精炼版为准；上表为结构约束。）

---

## 4. `agent-base` 去留决议

| 选项 | 做法 | 推荐 |
|------|------|------|
| **A. 删除 agent-base** | capability / chat 底座各自自包含首段身份；`assemble_mode` 的 parts 不再以 base 打头 | **推荐** |
| B. 保留一行身份 | `agent-base` 缩成 1–2 句 brand，无协议 | 可接受过渡 |
| C. 维持现状 | base + capability | **不接受**（本优化明确否决） |

**Pure chat / Chat exit**：采用 A 时，parts = `[chat-system 收敛版]` 或新建 `chat-base.md`（自包含），**不再**用空 agent-base。

**Worker**：parts = `[capability-rag.md]` 或 `[capability-search.md]` only（可加 brief 内联块，与现 host 一致）。

---

## 5. 编排器注入 capability 的实现选择

| 方案 | 描述 | 取舍 |
|------|------|------|
| **A. capability 内双节** | 文件含 `## 给任务分配者` + `## 给执行者`；brain 只 load 前节，worker load 全文或后节 | **推荐**：单文件真值，代码用 frontmatter/`<!-- orch -->` 或固定标题切分 |
| B. 独立 `capability-*-dispatch.md` | 编排专用短文，与 worker 文件并列 | 双文件易漂移 |
| C. 整份 capability 拼进编排器 | 实现最快 | 噪声大、与原则 5 冲突 |

**默认选 A。** 切分失败时 fallback：拼全文并打 warn（开发期可见）。

运行时伪代码：

```text
fn render_system_message(...) {
  s = load(orchestrator-base)
  for ch in materialized_channels {
    s += "\n\n" + load_dispatch_section(capability_path(ch))
  }
  s += runtime_state_block(...)
}
```

---

## 6. 与 monomode `*-system.md` 的关系

| 阶段 | 动作 |
|------|------|
| 本优化 P0–P1 | 主路径完全不依赖 `rag-system` / `search-system` / `chat-system` 作为 system 来源 |
| P2 | 内容 diff：把 `*-system` 中仍有价值、capability 缺失的句子迁入 capability 或 synthesis；然后标记 deprecated |
| P3 | 测试 / guardrail 泄漏样本改为 capability 文本；YAML `system_prompt_base` 指向 capability 或删除 monomode |

避免同时维护三套（system + capability + cluster 复述）。

---

## 7. 落地波次

### P0 — 文档与编排可见性（本文件 + 最小代码）

1. 本设计文档入库（本文件）；§0 启用矩阵 + §0.1 Chat 拼接已认。
2. `dispatch_agent_mode`：**纯聊天入口级绕过**编排（拍板 A）。
3. 重写 `orchestrator-base.md`（§3.3 结构）。
4. `brain.rs`：按已开启的检索通道注入 capability **给派活的人看的那一节**。
5. capability-rag / search 增加「给派活的人」小节。

**验收**：纯聊天不进入编排模块；`rag` 开启时编排 system 含工作区检索说明；`search` 同理。

### P1 — 去掉多余 agent-base

1. `mode_assemble`：parts 不再含 `agent-base`；worker = capability only；chat = 自包含 chat 底座。
2. 删除或 stub `agent-base.md`；更新 `prompts/README.md` 装配说明。
3. 回归：纯 chat、rag-only、search-only、dual、orchestrator e2e smoke。

### P2 — monomode 收敛

1. 审计 `rag-system` / `search-system` / `chat-system` 与 capability / synthesis 的 diff。
2. 迁移独有条款；deprecated 目录或 README 标明「非主路径」。
3. guardrail / token_budget 样例对齐。

### P3 — 质量飞轮（可选）

1. golden / smoke 中抽查 orchestrator brief 是否自包含（人工或 LLM-as-judge）。
2. 空 handoff / `coverage=partial` 时是否再派的行为探针。

**P3 落地说明（2026-07-20）：**

- 探针已入 `brain.rs` 测试：`partial_coverage_is_observable_and_redispatch_allowed`（gaps 进观察、进 chat handoff，新 goal 再派放行）、`empty_first_result_does_not_block_rag_redispatch`（Empty 不锁 rag 再派；search 连续空结果收敛由既有 `search_exhausted` 测试覆盖）。
- brief 自包含走**人工抽查**（LLM-as-judge 留待真机波次），抽查入口：编排 round 日志与 `assistant_turn_metadata.dispatches` 里的 `goal` / `instruction` 字段。抽查问题清单：
  1. 每条 brief 脱离对话历史是否仍读得懂（无「这篇 / 该 / 它」残留指代）？
  2. 工作区 brief 是否含文档身份 / 结构线索 + 要抽取什么？
  3. 公网 brief 是否脱离工作区也能独立成立（默认中英双语）？
  4. `delegate_chat.instruction` 是否写明理解口径 + 证据组织 + 已覆盖 / 未覆盖？
  5. 同一通道的再派是否换了角度（非原样重发）？

---

## 8. 明确非目标（本轮不做）

- 改 Orchestrator finish-gate / 强制首波 dispatch 的 Rust 策略（除非 prompt 无法表达）。
- 合并 RAG+Search 为一个 worker。
- 重写全部 cluster（codegen/writing）正文。
- 为编排器启用 JSON Output mode 或换模型。
- 解决 baiyao 等文档 `{"triplets":[]}` 空图（ingestion 侧，与本提示词优化无关）。

---

## 9. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 编排 system 变长导致多跳贵 | 派发节硬上限（建议 ≤400–600 中文字 / 通道）；禁止粘贴 codegen |
| 双节维护时只改执行节忘派发节 | code review 清单；可选单测 assert 两节标题存在 |
| 去掉 agent-base 后 pure chat 变空 | P1 与 chat 底座改动同 PR |
| 与 O1 结构首波文案冲突 | base 写「运行时可能已强制各通道先跑一轮；你仍根据结果决定是否再派」 |

---

## 10. 验收清单（优化完成时勾选）

- [x] 纯聊天入口不进编排；有 rag/search **永远**走编排（已删 V1=0 扁平旁路）  
- [x] 编排 system 在 rag 开启时含「给派活的人」的工作区说明；search 同理  
- [x] Chat 作答规则按材料种类组合：仅库 / 仅网 / 双源 三套形态，禁止一份万能材料作答模板（`answer-follow-brief` + `answer-from-workspace` / `answer-from-web` / `answer-dual-source`，P2 落地）  
- [x] 纯聊天仅聊天系统；双开但只有一侧有料时按实际有料侧拼接（`run_chat` 按 handoff.listings 实际材料选块，DocProfile 定向段不算材料）  
- [x] `orchestrator-base` 用工作过程书写，少内部黑话  
- [x] worker system 无强制依赖 `agent-base`  
- [x] `prompts/README.md` 装配图与代码一致  
- [x] 针对性测试不因装配回归（app-chat 151 / agent-tools 151 / avrag-guardrails 41 全绿；agent-loop 183 串行全绿——并行全量下 `codegen_without_print…bridge_has_chunks` 为预存时序抖动，恢复旧文件布局亦复现，与本优化无关）；product e2e smoke 留波次末真机跑  

---

## 11. 附录：关键路径速查（落地后）

| 路径 | 说明 |
|------|------|
| `prompts/orchestrators/orchestrator-base.md` | 协调者工作过程（已重写 v2） |
| `prompts/orchestrators/chat-base.md` | 纯聊天 / Chat exit 底座（替代已删 `agent-base`） |
| `prompts/orchestrators/capability-rag.md` | worker 全文 + `## 给任务分配者` |
| `prompts/orchestrators/capability-search.md` | 同上 |
| `prompts/orchestrators/answer-follow-brief.md` | Chat：听写作要求 |
| `prompts/orchestrators/answer-from-workspace.md` | Chat：仅工作区材料 |
| `prompts/orchestrators/answer-from-web.md` | Chat：仅公网材料 |
| `prompts/orchestrators/answer-dual-source.md` | Chat：双源对照 |
| `prompts/deprecated/monomode-system/*` | 已退役 monomode system（非主路径） |
| `prompts/synthesis/*.md` | 合成 skill；Chat exit 以 answer-* 积木为主 |
| `crates/app-chat/src/chat/pipeline_steps.rs` | 入口：纯聊 / 编排二分 |
| `crates/app-chat/src/orchestrator/brain.rs` | 编排 system 注入派发节 |
| `crates/app-chat/src/orchestrator/host.rs` | worker / Chat exit parts |
| `crates/app-chat/src/mode_assemble.rs` | pure chat / capability parts |

### 11.1 验收报告「第 5 点」白话说明（已澄清）

验收时曾写过一条残留担心：

> Chat 选作答规则时只看「材料清单 listings」，不看另一份叫 targeted 的列表。

**白话：**  
系统里每条证据都在同一个证据库里。`listings` = **全部条目的简表**（给 Chat 看「有哪些 E1/E2…」）。  
所谓 `targeted` **不是另一批证据**，只是其中一类——「文档定向」简介（doc_profile 一类），用来帮模型认文档结构，**本来就不能当引用材料**。

代码选规则时：

- 看 listings 里有没有**真正的文档段落**（不是定向简介）→ 加工作区作答  
- 有没有网页 → 加公网作答  
- 两种都有 → 再加双源对照  

因此「只看 listings」是对的，不是漏看；定向简介故意不算「有材料」。**此项不构成缺陷。**

---

## 12. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-20 | 初稿：问题陈述、目标装配、agent-base 决议 A、派发节方案 A、波次 P0–P3 |
| 2026-07-20 | 增 §0 启用矩阵（拍板：纯聊天入口级绕过）；§0.1 Chat 拼接白话版 |
| 2026-07-20 | §0.1 修正：否定「一份根据材料写答案」；改为工作区/公网/双源积木组合（默认四文件 A） |
| 2026-07-20 | **P0 落地**：`dispatch_agent_mode` 纯聊天入口级绕过（`pipeline_steps.rs`）；`orchestrator-base.md` 按 §3.3 重写为工作过程；`capability-rag/search.md` 增 `## 给任务分配者` 小节；`brain.rs` 按已物化通道注入该节（缺节 fallback 全文 + warn），工具描述去「Chat exit / 物化」黑话 |
| 2026-07-20 | **P1 落地**：新建自包含 `chat-base.md`（含 memory 簇请求格式；不写引用禁令以免与编排 Chat exit 的 E-marker 冲突）；capability 两文件加身份首行自包含；`mode_assemble` parts 去掉 agent-base（chat=`chat-base`；能力=`capability-*` only）；删除 `agent-base.md`；`prompts/README.md` 同步。测试：`app-chat` 147 / `agent-tools` 151 / `agent-loop` 183 全绿 |
| 2026-07-20 | **P2 落地**：独有条款迁移（capability-rag 补「同块并行省 budget / 整簇注入、无单 reference 语法 / 禁 native tool schema」；capability-search 补「时效标注、多源分歧」）；§0.1 积木落地：新建 `answer-follow-brief` / `answer-from-workspace` / `answer-from-web` / `answer-dual-source`，`run_chat` 按 handoff 实际材料拼接（仅库 / 仅网 / 双源）；monomode 三份移 `prompts/deprecated/monomode-system/`；`modes/*.yaml` 的 `system_prompt_base` 改指 capability / chat-base；prompt_leak 参考文本换成 8 份 live system prompts，fixture 换 capability-rag 原文；token_budget 样例换 capability-rag；agent-tools 测试锁定 monomode 退役 |
| 2026-07-20 | **P3 落地**：再派探针两个入 `brain.rs`（partial 可观察且可再派、Empty 不锁 rag 再派）；brief 自包含人工抽查清单入 §7-P3（LLM-as-judge 留真机波次） |
| 2026-07-20 | **删扁平兼容路径**：`dispatch` 对 rag/search **永远** `run_orchestrator_v1`；删除 `run_rag_mode` / `run_search_mode`；`AGENT_ORCHESTRATOR_V1` 不再闸产品路径。§0/§11 过期文案修正；§11.1 澄清 listings vs targeted |
| 2026-07-20 | **证据断链修复**：Chat synthesize 注入 store **全量正文**（非 300 字预览）；`finalize_answer_evidence` 把 store 写成 `dense_retrieval` tool_results 供评测 `extract_retrieved_chunks` |
| 2026-07-20 | **证据库去二次截断**：去掉 `MAX_RAG_ENTRIES=24` / `MAX_FULL_TEXT_CHARS=4000` 硬砍；store 只去重 + 赋 En；条数/正文由 RAG 管道动态 rough→rerank→final（约 10–30）与 worker `top_k` 控制；入库 chunk 保持 `TARGET_CHUNK_TOKENS=512` |
