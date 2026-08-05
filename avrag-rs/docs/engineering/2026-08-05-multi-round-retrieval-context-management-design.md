# 多轮检索上下文管理：Evidence Pool × Notes × Model-visible Clearing

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-05 |
| 状态 | **P0 + U-P0～U-P3 已实现**（reseen 成员闭包、visibility card/expand、history stub、evidence_index）；P1″ 深度 notes 板可续 |
| 动机 | 多轮 `client.dense` / codegen 回传叠厚，稀释 agent 注意力（context rot）；单 call 内已有 RRF/VGRAG cap，**跨轮无 body 级去重与折叠** |
| 非目标 | Host 语义「够了禁止再检」；替代 skill 的 claim 覆盖决策；全量 transcript 丢弃（持久化/评测仍可全量） |
| 相关 | pi-book ch08 `transformContext` / ch09 tool pipeline；Anthropic compaction + tool-result clearing + memory；`retrieval_summary` / `seen_aliases`；`trim_tool_results_for_synthesis`；邻接 S+L 设计；VGRAG final cut；波次审查 `2026-08-05-wave-doc-review.md`；**与 S+L 统一**见 `2026-08-05-s-plus-l-vs-p1-plus-conflict-and-unification.md` |
| 验收锚题 | full149 **q141** REFUSAL_WRONG：金句 rank0 仍在，多轮 cited 灌入同文舆论块 → 假拒（证据噪声主因，VGRAG 为放大器非唯一源） |

---

## 0. 一句话

**全量证据与笔记落在 host 侧 durable pool；每次 LLM 边界只组装高信号 model-visible 视图（笔记 + 卡片 + 近 K / delta 全文）。已推理过的 chunk 默认不再把全文塞进 loop——前提是结论已进 notes，且原文可按 alias 再取。**

去重只是子集；主轴是 **tool-result clearing + structured notes +（可选）compaction**。

---

## 1. 问题

### 1.1 现象

- 同一 run 内多轮 dense/lexical/grep → 每轮 observation 带完整 hit body。
- 同 `chunk_id` 再命中会拿 **新 `#n` alias** 再灌全文（bridge 单调 `alias_counter`）。
- `seen_aliases` 只进 `[retrieval_summary]` 饱和文案（「本轮 M 新增 / K 已见」），**不删 body**。
- 合成收口才有 `trim_tool_results_for_synthesis`（identical `(tool,data)` + ~48k）；**中段推理已被稀释**。
- Per-message `TOOL_MESSAGE_MAX_CHARS`（24k）是盲截 JSON，不是语义工作集。

### 1.2 当前去重边界（实现事实）

| 层 | 有无跨轮 body 去重 |
|----|-------------------|
| 单 call RRF / VGRAG C8 fuse | 有（`chunk_id`） |
| Bridge alias 注入 | **无**（始终新 `#n`） |
| Loop observation 叠入 messages | **无** |
| Citations / sources 构建 | 有（产品侧列表，非模型上下文） |
| Eval harness 跨轮 first-seen | 有（评测用） |
| Synthesis replay | 有限（整包 identical） |

### 1.3 与「多轮推理」的关系

本质不是「少检索」，而是 **model-visible 工作集随轮线性胀**。  
业界对标（见 §3）：长程 agent 的瓶颈常是 **可重取的 tool payload**，不是 assistant 推理字数。

---

## 2. 权威分层：Durable vs Model-visible

```text
┌─────────────────────────────────────────────────────────┐
│  Durable（可大；评测/cite/回放可全量）                      │
│  · EvidencePool: chunk_id → text, doc, score, first_alias │
│  · 全量 tool_results / bridge capture                     │
│  · 每轮 notes / claim 板（提炼事实，非 raw thinking）       │
└─────────────────────────────────────────────────────────┘
                          │
                          │  LLM 边界组装（≈ pi transformContext）
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Model-visible（要小、高信号）                             │
│  · 任务 + skill / capability                              │
│  · claim/notes 板                                         │
│  · evidence 卡片表（alias, doc, score, 1 行 snippet）      │
│  · 正文：本轮 delta + working-set 顶 K（或 SELECTED）      │
│  · 更早 tool_result → stub（保留 tool_use 记录）          │
└─────────────────────────────────────────────────────────┘
```

**纪律（对齐本仓 AGENTS.md）：**

- Host **报告形态与预算**（第三称 observation），**不**做语义「覆盖够了禁 DirectAnswer」。
- Stop 仍 **model + skill**；structural gate 只计 Ok 回传。
- 新 host marker 须先注册 `host_markers.rs`。

---

## 3. 外部最佳实践（映射）

参考：

- [pi-book ch08 agentLoop](https://zhanghandong.github.io/pi-book/ch08-agent-loop.html)：loop 只 append；**`transformContext` 在 LLM 边界裁剪**；compaction 不在引擎内核。
- [pi-book ch09 tool execution](https://zhanghandong.github.io/pi-book/ch09-tool-execution.html)：prepare/execute/finalize —— **不是** context 政策正文。
- Anthropic：[compaction / tool-result clearing / memory](https://platform.claude.com/cookbook/tool-use-context-engineering-context-engineering-tools)；[effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)。

| 机制 | 作用 | 本仓对应 |
|------|------|----------|
| **Tool-result clearing** | 旧可重取 result → 短占位；保留「调过」 | 旧 observation body → stub；pool 可 rehydrate |
| **Memory / notes** | 跨轮要留的事实写出 durable 存储 | claim 板 / findings；**不是** raw CoT 日志 |
| **Compaction** | 整段对话阈值摘要 | 可选后期；阈值触发 |
| **短读 + 展开** | 默认 snippet，按需全文 | 卡片默认 + hydrate/SELECTED |
| **Dedupe / novelty** | 同 id / 近重 / MMR | 跨轮 `chunk_id`；饱和收紧 k |
| **Subagent** | 大 dig 隔离 | 非本设计首版 |

### 3.1 对「每轮推理记录 → 不用再送 chunk」的判定

| 命题 | 判定 |
|------|------|
| 应有 durable 的「本轮结论/claim」记录 | **是**（notes，不是 thinking 全文） |
| 有 notes 后 model-visible 可不常驻已处理 chunk 全文 | **是**（配 re-fetch） |
| 只记 reasoning、不写可引用事实就清 body | **否**（终答丢数、cite 断） |
| 物理从 loop 删除且永不可取 | **否**（需 SELECTED / 验表 / 纠错） |

**笔记是可丢原文后的可计算状态；chunk 全文是可重取的工具载荷。**

---

## 4. 方法族（实现菜单）

### 4.1 P0 — 跨轮 dedupe + delta body（小）

- Host 维护 `seen_chunk_ids`（及 `chunk_id → first_alias`）。
- 再命中：observation **不重复 body**；写 `reseen:#old` 或只增卡片行。
- 可选：同 alias 复用（若 SELECTED 命名空间允许）vs 新 `#n` 但 body 省略。

### 4.2 P0′ — 饱和收紧回传

- 已有 `new_aliases` / `seen` 计数 → 挂到 **回传形态**（降 top-k、只卡片、只 summary）。
- Observation 第三称：「本轮 new_ratio 低；回传已收紧为卡片」。

### 4.3 P1 — 二级证据（短表 + hydrate）

- Dense 默认：alias / doc / score / snippet。
- 全文：`hydrate` 原语、或 SELECTED 前展开、或 working-set 提升。
- 与现 `#n` / SELECTED / citations 天然契合。

### 4.4 P1′ — Loop 中 working-set / clearing

- 近 1–2 轮完整；更早 tool_result → stub。
- Working-set 顶 K 全文常驻（score / SELECTED / 本轮 new）。
- 将 synthesis 的 snip **前移到每轮 LLM 前**（pi `transformContext` 位）。

### 4.5 P1″ — Structured notes / claim 板

- 每轮或 saturation 时：模型或 codegen 更新「已支撑 claim / 未见 claim / 关键数字」。
- Model-visible **优先 notes**；终答主盯 notes + 小 working-set。
- Skill 侧第三称 gotcha 可描述「笔记已覆盖 claim 时再 dig 的 observation 常为重见」——**不** host 禁检。

### 4.6 P2 — 检索侧减噪

- 后轮 adaptive_k；多 query 先 merge 再一包 observation。
- 邻接 S+L（另文）；novelty/MMR。
- VGRAG final cut 只解 **单 call**，不解多 call 相加。

### 4.7 不推荐作主闸

| 做法 | 原因 |
|------|------|
| Host 语义禁再检 | 违背 stop-by-model |
| 只加大 char 盲截 | 可能砍掉后排金 chunk |
| 仅 synthesis 去重 | 中段已稀释 |
| 只记 CoT 不记 facts | 清 body 后无法作答 |

---

## 5. 推荐落地顺序

```text
P0   跨轮 chunk_id dedupe + delta-only body
P0′  饱和时收紧本轮 k / 只回卡片（挂 seen_aliases）
P1   默认 shortlist 卡片 + hydrate/SELECTED 展开
P1′  LLM 边界 transformContext：旧 tool_result clearing + working-set K
P1″  claim/notes 板（model-visible 主记忆）
P2   novelty、邻并、多 query 单 observation、可选 compaction
P3   skill 饱和/换轴 gotcha（观察语气）
```

**最小闭环（常够用）：**

1. 同 `chunk_id` 不复述 body（可标 reseen）。  
2. 每轮：**卡片表 + 仅 new 全文**。  
3. `new_ratio≈0`：本轮自动更短 + summary 写清环境事实。  
4. Working-set 顶 K；其余指针；cite 走 pool。

---

## 6. 与现码挂点（实现时）

| 组件 | 路径 / 符号 | 角色 |
|------|-------------|------|
| Alias 注入 | `rag-core` `runtime/bridge.rs` K2 | 可挂 reseen / 不灌 body |
| 饱和信号 | `agent-loop` `iteration_codegen::retrieval_callouts` | 已有 new/seen；扩展收紧 |
| Per-msg 裁切 | `message_format::trim_json_for_context` | 保留；不替代 working-set |
| 合成 snip | `synthesis::trim_tool_results_for_synthesis` | 前移思路的参考实现 |
| SELECTED | `helpers/selected.rs` | alias→chunk_id 须在 pool 稳定 |
| Host markers | `host_markers.rs` | 新 tag 先注册 |
| VGRAG | `runtime/tools/vgrag.rs` | 单 call cap 不变；**多 call 并集**才胀（q141） |
| 沙箱连续失败 | `MAX_CONSECUTIVE_SANDBOX_ERRORS=4`（2026-08-05 已码） | 与证据折叠**正交**：只加模型修码轮次，不减 hit body |

**pi 对齐：** loop 可继续 append capture；**送模型前**做可见视图变换，避免把政策塞进 execute 内核。

---

## 7. Observation 契约草案（P1 级）

示意字段（实现时可落 `prompts/loop/` 模板 + 第三称叙述）：

```text
[evidence_index]
  pool_n=40 expanded=8 new_this_round=3 reseen=5
  cards: #1 doc=… score=… snippet=…
  bodies: #3 #7 #12   // 仅 expanded
  reseen: #2→#2 (chunk_id=…)  // 或新 #n 无 body
[retrieval_summary]
  … 本轮 N 次，M 条；new_ratio=…；回传形态=cards|delta|full …
```

Stub 例（clearing 后）：

```text
[tool_result_cleared] dense call@round2 hits=12 bodies_cleared=12
  note: 全文见 EvidencePool；alias 映射仍有效；可 hydrate(#n)
```

---

## 8. 风险与验收

| 风险 | 缓解 |
|------|------|
| 笔记漏写 → 终答丢细节 | 默认保留近轮全文 + working-set K；notes 渐进 |
| SELECTED 指向已 clear body | cite/hydrate 走 durable pool，不依赖 messages 内全文 |
| 评测 recall 依赖 transcript 全文 | harness 已可走 tool_results；保持 capture 全量 |
| 过早 compaction 丢数字 | 首版不做全量 compaction；先 clearing + notes |
| 新 marker 未注册 | parity test 失败 |

**验收建议（实现后）：**

- 单测：同 chunk 两轮 dense → 第二轮无重复 body；alias 映射可解析。  
- 人工/探针：dense 风暴题 model-visible token 不随轮线性涨。  
- full149 子集：PASS 不回归；SELECTION_MISS / 多 claim 题盯 notes 质量。

---

## 9. 非目标再声明

- 不在此设计内实现邻接 S+L（见 `2026-08-05-retrieval-adjacent-shortlist-merge-design.md`）。  
- 不改变 multi-caliber 表计数产品政策（另文）。  
- 不把 CORRECT_UNGROUNDED 与本上下文管理绑定（标签另文）。

---

## 10. 决议摘要

1. **问题定性**：多轮推理的 **model-visible 工作集** 管理，不是单次检索 fuse。  
2. **主轴**：EvidencePool（durable）+ notes/claim + clearing/working-set（visible）。  
3. **去重**：必要但不够；挂在 P0。  
4. **挂载点**：LLM 边界（pi `transformContext` 位），不膨胀 tool execute 内核。  
5. **自主权**：host 只改可见形态；是否再检仍由 model + skill。
