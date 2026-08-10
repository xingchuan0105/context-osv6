# 知识库 Skill 硬化 + 上下文保留（强制留存 / 宿主驱逐）

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-07 |
| 状态 | **设计草案 v2** — **W0–W2+P1+W4 已落地**（KNOCKOUT 硬滤关）；W3（Tier B）待开工 |
| 动机 | （1）skill 包高成熟度，摩擦在**跨文件冗余**与 **contract 定位**；（2）full149 见 KNOCKOUT≈0 次：自愿删除激励倒挂；（3）多轮噪声淹没早期关键证据（中段注意力 + 相关干扰块） |
| 非目标 | 入库切块重做；verify 语义改写；eval golden 泄漏；host 自动语义「什么是噪音」；Tier C 学习 utility；子代理隔离（架构允许时另文） |
| 相关 | `prompts/capabilities/knowledge-base/**`；`SELECTED` / `KNOCKOUT`；`client.save`/`load`；`EvidencePool`；三环 [verify](2026-08-07-retrieve-synthesis-verify-loop-design.md)；[证据敲除 V1](2026-08-07-evidence-knockout-design.md)（**硬抑制语义将由本文 V2 改向**，见 §4） |
| 输入 | ① skill 包最佳实践对照评估（触发/披露/冗余/contract）；② KNOCKOUT 激励 + 上下文工程/ACE/Anthropic memory 谱系分析；③ 前版 EWS/KEEP 草案 |
| 评测锚 | full149：KNOCKOUT 使用率、终答漏 SELECTED、早期证据失踪、多跳噪声题、主上下文 token |

---

## 0. 一句话

**Skill：信息只存在于一处 + contract 真薄 + 加载/终止 checklist；运行时：LLM 只负责「强制正向留存（台账/KEEP）」，宿主负责确定性驱逐旧正文；KNOCKOUT 降级为可选标注/降权信号，不再依赖「自觉销毁可见性」。**

---

## 1. 综合评估结论

### 1.1 Skill 包（第二次交叉阅读 + skill-creator 对照）

| 维度 | 评级 | 结论 |
|------|------|------|
| description 触发 | ★★★★★ | 正/负向触发清晰 |
| 渐进披露 | ★★★★★ | thin + spoke 教科书级；触发可更机械 |
| 自由度匹配 | ★★★★★ | API 硬约束 / 策略中自由度匹配 |
| 内容价值 | ★★★★★ | 宿主语义、覆盖态、协议、多口径 — 值回 token |
| 内部一致性 | ★★★★★ | 跨文件规则无实质矛盾 |
| **跨文件去重** | **★★☆☆☆** | **最大短板**：核心规则 3–5 处重复，常驻 token 2–3× |
| 写作形式 | ★★★☆☆ | 第三人称观察体为本仓 **AGENTS 硬规则**（非疏忽）；与通用 skill-creator「祈使句」冲突时 **以本仓为准** |
| 可移植性 | ★★★☆☆ | `disclose_at` / `skill_request` 等为平台字段，可文档隔离 |

**亮点（勿改实质）：** 空结果/截断表、SELECTED⊥KNOCKOUT、虚构域 few-shot、多口径并列。

**重复热图（canonical 待定）：**

| 规则 | 约出现次数 | 建议唯一出处 |
|------|------------|--------------|
| truncated 只限样本、已见仍有效 | 5 | SKILL 空结果表 |
| total_hits = 命中行数 | 5 | how-to-read-tables（表格本体）或 SKILL 表格节二选一 |
| 第一个 = row_ord | 5 | how-to-read-tables |
| 行数 vs 去重多口径 | 4 | strategies-tables |
| 业界框架不用训练记忆 | 4 | SKILL 证据节 |
| spoke 目录表 | 2 | **仅 strategies.md**（SKILL 改指针） |

### 1.2 为何 KNOCKOUT 几乎不触发（机制，非「提示不够狠」）

| 层 | 说明 |
|----|------|
| **收益不可见** | 敲除收益在未来轮次；本轮无即时回传强化 |
| **与核心纪律冲突** | skill 反复强化「证据保全 / 未知≠不存在」；自愿销毁「已见证据」的可见性与训练方向相反 |
| **opt-in 无硬时机** | 「可」用 + 噪音判定无阈值；对比 SELECTED 末行、total_hits 有强制形态 |
| **反元认知难** | 要求在噪声正在影响自己时自觉判定噪声 |

旁证谱系（可靠性大致递增）：

| 机制 | 驱逐决策 | 可靠性 |
|------|----------|--------|
| LLM 自愿删除（KNOCKOUT 硬隐藏） | LLM | **低** |
| LLM 标记 + 代码剪枝（ACE helpful/harmful） | LLM 提议，代码裁决 | 高 |
| **正向留存 + 宿主驱逐**（Anthropic memory / 本设计主路径） | LLM 决定留什么，宿主清旧 | **高** |
| 纯宿主 `clear_tool_uses` 式阈值 | 宿主 | 最高（偏非语义） |
| 子代理隔离探索 | 架构 | 最高（另文） |

**收敛原则：** 上下文是有限资源；**LLM 自主权放在「留什么」**，**不放在「删什么」**。

### 1.3 噪声淹没的机制解释

- **位置衰减（lost-in-the-middle）**：多轮 chunk 堆在中段，注意力最弱。  
- **相关干扰块**比随机噪声更伤：dense 扇出每轮引入「主题相关但不含主张」的块 — 多轮 agentic RAG 主噪声源。  
- 最优须同时：**减少中段噪声存量** + **把关键证据放到高注意力位置（复读 / EWS 置顶）**。

### 1.4 与前版 EWS 草案的关系

| 前版 | 综合后 |
|------|--------|
| KEEP 为推荐、KNOCKOUT 为硬黑名单主路径之一 | **强制正向台账/KEEP 为主默认**；宿主确定性驱逐为主力 |
| KNOCKOUT 抑制 1–2 次再命中 | **V2：KNOCKOUT 降级为标注 + harmful 计数**；硬隐藏改为可选/弱化（见 §4） |
| sticky 空 KEEP | 保留；并增加 **流程强制写台账** 的时机 |

---

## 2. 目标架构（主默认 → 辅助 → 兜底）

```text
┌──────────────────────────────────────────────────────────┐
│ 每轮 / 饱和 / 进合成前                                    │
│  模型：强制写「证据台账」行（主张 → alias/chunk + 短引文）  │
│        可选 KEEP: #… 与台账对齐（Tier A 可用 KEEP 兼台账）   │
└────────────────────────┬─────────────────────────────────┘
                         ▼
┌──────────────────────────────────────────────────────────┐
│ 宿主：Active 台账/EWS 置顶注入主上下文                      │
│       旧轮原始 tool/chunk 正文 — 确定性折叠/驱逐            │
│       （保留最近 K 轮全文 + 更早轮占位 [已折叠…]）          │
└────────────────────────┬─────────────────────────────────┘
                         ▼
┌──────────────────────────────────────────────────────────┐
│ 终答前：台账关键短引文复读到上下文末端（recency）            │
│         SELECTED: #… ⊆ 台账/active 集合（校验→观察）        │
└──────────────────────────────────────────────────────────┘

辅助：KNOCKOUT → 标注 [已标注-存疑] + harmful 计数（重排降权），默认不销毁可见性
兜底：token 阈值 / 饱和信号触发宿主折叠（不依赖模型自觉）
```

| 角色 | 做什么 | 不做什么 |
|------|--------|----------|
| **模型** | 判定哪条证据支撑哪条主张；**必须**写入台账/KEEP；终答 SELECTED | 不负责「自觉清空上下文」 |
| **宿主** | 解析台账/KEEP；组装顺序；驱逐旧正文；可选 harmful 降权；复读注入 | 不自动语义判定噪音替代模型 |
| **Skill** | 教协议与何时落账；canonical 去重 | 不把 host 硬闸写成禁令堆 |

---

## 3. Part A — Skill 包硬化（去重 + 定位 + checklist）

### 3.1 写作体裁（本仓拍板）

- **LLM 面向文案**：继续 **第三人称观察体**（`AGENTS.md` 非协商）。  
- 评估中「祈使句最佳实践」适用于通用 skill 市场；**与本仓冲突时以本仓为准**。  
- 可在不改体裁下提高行动性：用「环境中的常见下一步 / 失败对照表」替代「应/必须」。

### 3.2 Contract 定位（P0）

| 选项 | 含义 | 决议 |
|------|------|------|
| A. 系统提示载体 | 与 skill 包分离注入 | **若 runtime 仍双注**：contract **只保留挂载声明 + 指针**，删与 SKILL 重复段 |
| B. skill reference | 包内 reference | 则 frontmatter 去掉误导性 `category: system-prompt`，并极度收薄 |

**拍板（默认 A）：**  
`contract.md` = **capability 挂载时的系统侧短合同**（「已挂载知识库；证据与签名以 knowledge-base skill 为准」）。  
操作细节、覆盖态表、KNOCKOUT 细则、门径 few-shot → **只在 SKILL / spoke**。  
KNOCKOUT/SELECTED **最多一行指针**。

### 3.3 Canonical 出处（P1）— 信息只存在一处

| 主题 | Canonical | 他处 |
|------|-----------|------|
| 空结果 / truncated / 饱和 | **SKILL.md** 空结果表 | spoke 一行指针 |
| SELECTED / KNOCKOUT 协议 | **SKILL.md**（KNOCKOUT V2 语义按 §4 改） | contract 一行 |
| total_hits / row_ord / 管道 ontology | **how-to-read-tables** | SKILL 表格节极短指针；strategies-tables 指针 |
| 行数 vs 去重多口径 | **strategies-tables**（FS 一处） | how-to-read-tables 不重复整例 |
| 业界框架槽 | **SKILL.md** 证据节 | grounding 指针 |
| spoke 目录 + 触发词 | **strategies.md 仅此一表** | SKILL 策略层：「目录见 strategies」 |
| entity-first 全表 | **strategies-graph** 或 thin 3–5 条 | 禁止双全表 |

预计常驻 token **−20–30%**（评估估计）。

### 3.4 P0 清单（skill-only，可先于 host）

| ID | 工作 |
|----|------|
| A-P0-1 | strategies spoke 表：唯一目录 + **加载触发词**列（表格→tables+how-to-read；关系→graph；…） |
| A-P0-2 | 终止 checklist（主张全闭合 + 饱和或已标未覆盖 + SELECTED 末行规则）— 观察体 |
| A-P0-3 | contract 收薄到挂载+指针；删重复证据/覆盖态/门径长文 |
| A-P0-4 | SKILL 删掉与 strategies **逐字重复**的 spoke 目录表 |

### 3.5 P1–P2（skill）

| ID | 工作 |
|----|------|
| A-P1-1 | 合并 strategies-tables FS3b 与 how-to-read-tables B2 重复例 |
| A-P1-2 | how-to-read-tables（>100 行）顶部 **TOC** |
| A-P1-3 | 最小成功首块 1–2 例（dense+lexical；catalog→query） |
| A-P1-4 | docscope vs `client.*` 一句话边界 |
| A-P1-5 | 混合结果：精确数字（lexical/grep）优先于 dense 叙述 — 一句 |
| A-P2-1 | FS 编号 spoke 内自洽或薄层全局表 |
| A-P2-2 | frontmatter 平台字段说明（`disclose_at` 等）隔离为「平台适配」短注 |
| A-P2-3 | last-synced 日期（contract ↔ skill） |

### 3.6 明确不改

- 空结果表、多口径立场、虚构 few-shot、SELECTED⊥KNOCKOUT **键类型分离** — 实质规则保留。  
- 不为迎合外部 skill 规范而改掉第三人称观察体。

---

## 4. Part B — 运行时：强制留存 + 宿主驱逐 + KNOCKOUT 改向

### 4.1 协议分层

| 协议 | V1 现状 | V2 目标语义 |
|------|---------|-------------|
| **证据台账 / KEEP** | 无 / 草案 | **流程强制**的正向留存（主默认） |
| **SELECTED** | 终答圈 alias | 终答引用；⊆ 台账/active；末行规则加强教学 |
| **KNOCKOUT** | 硬抑制 1–2 次 + 第 3 次 reexpose | **标注 + harmful 计数**（可选）；默认**不**再作为「自觉删除」主路径 |
| **宿主折叠** | 无（仅 char/visibility） | **确定性**保留最近 K 轮原文，更早轮占位 |

> **与 [证据敲除 V1](2026-08-07-evidence-knockout-design.md) 关系：**  
> V1 硬过滤代码可暂留（未发 KNOCKOUT 时行为不变）。  
> **产品叙事与 skill 教学**按 V2：不指望模型靠 KNOCKOUT 治噪声；若继续硬隐藏，须在 skill 写明「可选降权标注」并评估是否关闭默认 suppress（开放问题 §8）。

### 4.2 强制正向台账（主默认）

#### 4.2.1 时机（流程强制，非「可」）

在下列**任一**时刻，模型须产出台账更新（宿主可检测缺失并注入第三人称 observation，**不** invent 内容）：

1. 检索环出现 **饱和信号**（连续轮次新高价值 alias≈0）；或  
2. **即将离开检索**（handoff 合成 / 预算释放前）；或  
3. Tier A 简化：每轮检索观察之后 **一行 KEEP**（兼作台账）。

#### 4.2.2 最低形态（Tier A — 推荐先做）

```text
KEEP: #3, #7, #12
```

| 规则 | 约定 |
|------|------|
| 键 | `#alias`（与 SELECTED 同认知） |
| 空 KEEP / 无行 | **sticky** 上一 active（防误清空） |
| 非法 alias | 丢弃 |
| 与 SELECTED | 终答 SELECTED 宜 ⊆ 本 run 曾 KEEP/台账集合 |

可选 demote：`KEEP_DROP: #5` → archived。

#### 4.2.3 结构化台账（Tier B）

`client.save("evidence.json", …)` 或宿主 session 权威存储：

```json
{
  "updated_round": 3,
  "items": [
    {
      "alias": "#3",
      "chunk_id": "uuid-…",
      "doc_id": "…",
      "claim": "保修年限",
      "quote": "≤200字关键引文",
      "status": "active",
      "harmful_count": 0
    }
  ]
}
```

- **判断权（留什么）**在模型；**必须写**由流程/观察门驱动。  
- 一行一条主张槽；覆盖状态可附在 claim 字段旁或分列。

### 4.3 宿主确定性驱逐（主力）

| 规则 | 约定 |
|------|------|
| 保留 | **最近 K 轮**（建议 K=1 或 2）检索 observation **全文** |
| 更早轮 | 替换为占位：`[已折叠: 第n轮, m块, ids=…]` + 指向台账/EWS |
| 触发 | token 阈值 **或** 轮次阈值 **或** 饱和信号（与 skill 对齐） |
| 不删索引 | 折叠的是**上下文注入**；检索索引与 `client.*` 仍可再命中 |
| 再命中 | 可标 `[previously_archived]`（须 host_markers） |
| Active 台账/EWS | **不受**「仅因轮次旧」折叠；始终按 §4.4 置顶注入 |

对应 Claude `clear_tool_uses` 一类语义：**过程痕迹可保留摘要，冗长结果确定性清出主窗口**。

### 4.4 上下文组装顺序（权威）

1. 系统 + capability/skill 披露（渐进）  
2. **Active 台账 / EWS**（全文或高保真 quote — 最高优先级）  
3. **最近 K 轮** 原始检索 observation  
4. 本轮新命中  
5. 更早轮 **折叠占位**  
6. （终答前）**台账复读块**贴到末端  
7. 合成/verify 材料  

### 4.5 终答前证据复读（廉价，直击中段丢失）

- 合成前或合成 user 组装时：把台账中每条 `quote`（或 EWS snippet）**再次打印**在上下文末端。  
- SELECTED 只圈 alias 不够；**内容**须出现在 recency 位。  
- 第三人称 observation 可选：`[evidence_reread]`（先登记 host_markers）。

### 4.6 KNOCKOUT V2 语义（辅助，非主路径）

| 项 | V2 |
|----|-----|
| 模型动作 | 仍可输出 `KNOCKOUT: <chunk_id>` |
| 宿主默认 | **不**从主路径销毁可见性；记 `harmful_count++`；块旁或台账 `status` 可标存疑 |
| 用途 | 后续重排降权、分析、可选 soft demote |
| 与「过度抑制」 | 标注方案天然避免 V1「藏起来导致无法复核」 |
| full149 预期 | 即使使用率仍低，**系统不依赖它**也能靠台账+驱逐变干净 |

若产品仍需硬屏蔽极少数有害块：单独 `KNOCKOUT_HARD` 或配置开关，**默认关**。

### 4.7 可选：噪声源头分流（架构允许时）

- 「摸范围」dense 扇出在子上下文跑，主上下文只回收收窄后的 `doc_ids` + 蒸馏候选。  
- 本设计 **不阻塞**；列为 Part C 备选。

### 4.8 生命周期（单轮，V2）

```text
Round t:
  1. 宿主注入：Active 台账/EWS + 近 K 轮全文 + 旧轮折叠占位
  2. Agent 检索（新 alias）
  3. Agent 对照主张；输出 KEEP/台账行（强制时机见 §4.2.1）
  4. 可选 KNOCKOUT → 仅计数/标注
  5. 宿主更新 active；执行折叠策略
  6. 若 handoff 合成：注入 evidence_reread，再合成 → verify
```

---

## 5. 与现网实现映射

| 组件 | 现状 | 本文要求 |
|------|------|----------|
| `KnockoutState` | 硬滤 1–2 + reexpose@3 + obs | 保留代码路径；**产品默认叙事改 V2**；评估是否默认 `apply` 关闭硬滤 |
| `EvidencePool` | alias/body/claim board | 复用映射；台账/EWS 为策略视图 |
| `SELECTED` | 终答 | + 教学强制末行；Tier B ⊆ 台账校验观察 |
| `client.save` | 自由 | Tier B `evidence.json` 约定 |
| `mode_debug.verify/knockout` | 已落 | 增 `ews` / `ledger` / `fold` 计数（W1+） |
| skill KNOCKOUT 教学 | 硬抑制描述 | 改为标注语义 + 指向台账主路径 |

---

## 6. 落地波次

| 波次 | 内容 | 验证 |
|------|------|------|
| **W0** | Part A：contract 收薄、canonical 去重、spoke 触发词、终止 checklist、spoke 表去重 | 人工 diff；token 量粗测 |
| **W1** | Part B Tier A：KEEP 解析 + sticky + **近 K 轮折叠** + EWS/KEEP 置顶注入 + skill 教学改向 + 单测 | `cargo test -p agent-loop`；定向多轮噪声题 |
| **W2** | 终答前 **evidence_reread**；SELECTED 教学强化；obs 字段 | full149 抽检漏 SELECTED、早期证据 |
| **W3** | Tier B：evidence.json、预算 knapsack、SELECTED⊆台账观察、harmful 计数接线重排（若有） | full149 c8 对比 |
| **W4** | KNOCKOUT 硬滤默认策略拍板（关 / 仅 hard 开关）；指标看板 | ✅ **关**：`KNOCKOUT_HARD_SUPPRESS=false`；skill/contract 撤硬抑制教学 |
| **W5** | 子代理探索隔离（可选） | 架构评估后另开 |

**推荐立刻做：W0 + W1**（skill 去重不改规则实质；运行时翻转为强制留存+宿主驱逐）。

---

## 7. 成功指标

| 指标 | 方向 |
|------|------|
| full149 / 多轮题 **KNOCKOUT 使用率** | 不再作为成功标准（预期仍可低） |
| 主路径「无 KNOCKOUT 时上下文是否仍被旧噪声撑满」 | ↓（折叠生效） |
| 终答 SELECTED 覆盖率（有 doc 命中时） | ↑ |
| 终答 SELECTED ∩ 中间 KEEP/台账 比例 | ↑ |
| 第 3 轮+ 主上下文平均 token | ↓ |
| 相关干扰主导的失败题（人工标签） | ↓ |
| 常驻 skill token（strategies+SKILL 默认披露） | ↓ 20–30% 目标 |

---

## 8. 开放问题（拍板后删）

1. **KNOCKOUT 硬滤**：W1 是否立即默认关闭 `apply_to_bridge` 抑制，只保留计数？还是双轨一版？  
2. **K（近轮保留）**：默认 1 还是 2？与 `max_iterations` 关系？  
3. **强制台账缺失**：仅 observation 提示，还是结构闸阻止 handoff 合成？  
4. **KEEP 是否允许合成环再写**？  
5. **台账权威源**：仅宿主状态 vs 信任 `client.save`？  
6. **第三人称 vs 祈使**：对外文档是否增加「本仓 observation 体例外说明」（已倾向是）？

---

## 9. 风险

| 风险 | 缓解 |
|------|------|
| 强制台账增加格式失败 | 解析宽松；失败观察；sticky |
| 折叠过狠丢可回溯 | 占位保留 ids；索引可再检；K≥1 |
| 复读撑爆 token | quote≤200 字；knapsack |
| 与 evidence 保全叙事冲突 | 折叠是「不自动注入」不是「语料不存在」；skill 写清 |
| 评测泄漏 | claims/quote 禁止抄 golden |

---

## 10. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-08-07 | v1：skill 评估 P0–P2 + EWS/KEEP/KNOCKOUT 硬抑制并存草案 |
| 2026-08-07 | **v2 综合定稿方向**：二次 skill 评估（冗余/contract/canonical）；KNOCKOUT 激励倒挂与业界谱系；主默认改为**强制正向台账 + 宿主确定性驱逐 + 终答复读**；KNOCKOUT→标注辅助；W0–W5 波次；写作体裁以本仓第三人称为准 |
| 2026-08-07 | **W0 落地**：`contract.md` v1.6 收薄为挂载合同；`strategies.md` 唯一 spoke 表+触发词列+终止 checklist；`SKILL.md` v5.0 删重复 spoke 表/表格长述，指针到 strategies / how-to-read-tables |
| 2026-08-07 | **W1 落地**：`helpers/ews.rs` KEEP/KEEP_DROP+sticky；`IterationState.ews`；model_visible 优先注入 `[ews_active]`；近 K=2 轮折叠复用 `context_visibility`；`mode_debug.ews`；skill KEEP 教学 |
| 2026-08-07 | **P1 落地**：how-to-read-tables TOC；FS3b→B2 指针去重；SKILL 最小首块示例 A/B/C；docscope vs client 边界；strategies 精确数字优先句 |
| 2026-08-07 | **W2 落地**：合成前 `[evidence_reread]`（EWS 短引文 recency）+ `mode_debug.ews.reread_*`；SELECTED 末行/⊆KEEP 教学强化；**同轮 KEEP 接线修复**（tool 回填后再 `apply` KEEP） |
| 2026-08-07 | **W4 落地**：`KNOCKOUT_HARD_SUPPRESS=false` — bridge/tool 不再剥离；`mode_debug.knockout.hard_suppress_enabled=false`；skill/contract/strategies 撤 KNOCKOUT 硬抑制教学；V1 算法留 `apply_to_bridge_data_hard` 测路径 |
