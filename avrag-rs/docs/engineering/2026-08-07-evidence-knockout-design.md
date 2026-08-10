# 证据敲除（Evidence Knockout）

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-07 |
| 状态 | **V1 硬抑制已关（W4）**；主路径 = KEEP + 宿主折叠，见 [KB 硬化 + 强制留存/宿主驱逐 v2](2026-08-07-kb-skill-hardening-and-evidence-working-set-design.md)。`KNOCKOUT_HARD_SUPPRESS=false` |
| 动机 | 多口径 / 噪声块先进入模型可见面后抢答（典型 full149 多口径竞争）；非「上下文整体太长」类污染 |
| 非目标 | 入库切块策略；相邻 S+L merge（已有独立路径）；verify 裁决逻辑；评测侧 golden 屏蔽；host 自动语义点名噪音 |
| 相关 | **SaC**（`client.*` 沙箱检索 + knowledge-base skill）；检索 observation / evidence pool；`SELECTED` 线协议（同族）；`RETRIEVAL_ADJACENT_MERGE`；三环 retrieve→synthesis→verify；`prompts/loop/*` + `host_markers`；**下文 V2 改向：标注优先于硬隐藏** |
| 评测锚 | full149 类：同主题竞争口径（如 q026）、噪声块诱导假拒 / 错锁；**使用率观测：自愿 KNOCKOUT 近零** |

---

## 0. 一句话

**SaC 检索回传中模型先看见 chunk → 用与 SELECTED 同族的线协议报出噪音 `chunk_id` 列表 → host 敲除；同 id 在后续任意检索工具第 2 次命中仍不进可见面；第 3 次命中再传入并贴第三人称 observation（曾被敲除且命中三次）。**

---

## 1. 拍板决策

| # | 议题 | 决议 |
|---|------|------|
| 1 | 噪音反馈形态 | **结构化 id 列表** |
| 2 | 「同一证据」键 | **`chunk_id`**（UUID，与沙箱回传字段一致） |
| 3 | 命中计数范围 | **跨工具共用**（dense / lexical / grep / struct / 其它带 `chunk_id` 的检索 Ok 回传） |
| 4 | 第 3 次放行标记 | **是** — `prompts/loop/*` 第三人称 observation + `host_markers` |
| 5 | 谁判定噪音 | **模型**（SaC 轮次内点名） |
| 6 | 谁执行 | **Host**（解析协议、敲除集、命中计数、过滤可见面、贴标） |
| 7 | **集成面** | **SaC 产品路径**：协议由 **knowledge-base skill（+ 薄 contract 指针）教会模型**；解析与过滤在 **agent-loop / 检索 observation 装配**，不另开原生 tool |

### 明确不是

| 误解 | 实际 |
|------|------|
| Host 一上来 hide top-k | **先见再敲**；名单只来自模型协议行 |
| 独立 native tool `knockout_chunks` | **不**新增 tool；与 `SELECTED` 一样是 **模型输出线协议** |
| 第 3 次「另开神秘检索」 | 仅当 **检索再次命中该 `chunk_id`** 时按计数放行 |
| 与相邻 merge 混谈 | merge 管切块邻接；敲除管噪音 id 的后续抑制 |

---

## 2. SaC 集成（权威实现路径）

### 2.1 为何挂在 SaC

- 文档侧证据只从 **沙箱 `client.*` 回传** 进入模型（capability contract）。
- 模型已用线协议 **`SELECTED: #n`** 圈定采用；敲除用 **平行线协议** 点名噪音，不破坏「无原生检索 FC」的 SaC 形状。
- skill 渐进披露：`knowledge-base/SKILL.md` 教协议；host 只做机械解析与过滤。

### 2.2 模型输出协议（wire）

与 `SELECTED` 同风格的 **独立行**（可出现在检索轮 assistant 内容任意处；允许 blockquote / 行首装饰，解析时 strip）：

```text
KNOCKOUT: <chunk_id>[, <chunk_id>…]
```

| 规则 | 约定 |
|------|------|
| 前缀 | `KNOCKOUT` 或中文别名 **`敲除`**（与 SELECTED/`选择` 对称）后接 `:` / `：` |
| 条目 | 标准 UUID 形 `chunk_id`（与回传 JSON 字段一致）；**不是** `#alias` |
| 多 id | 逗号 / 顿号 / 空白分隔；去重保序 |
| 非法 id | **丢弃**，不进敲除集、不报错烧轮 |
| 未见 id | **丢弃**（本题 run 中从未出现在 Ok 检索回传中的 id 不登记） |
| 空列表 / 无行 | 无操作 |
| 与 SELECTED | **正交**：SELECTED 圈定采用；KNOCKOUT 点名噪音。同一 id 可先 SELECTED 后敲除（后续抑制），或只敲除不采用 |

**示例（虚构 id）：**

```text
本轮回传中 #4 仅铺垫、与题干数字槽无关。
KNOCKOUT: 11111111-1111-1111-1111-111111111111
```

### 2.3 Skill 教学（prompts — 唯一 LLM 面向文案）

| 资产 | 职责 |
|------|------|
| `prompts/capabilities/knowledge-base/SKILL.md` | **主教学**：何为噪音面、何时点名、线协议、`chunk_id` 非 alias、与 SELECTED 正交、环境在敲除后会抑制再命中直至第三次 |
| `prompts/capabilities/knowledge-base/contract.md` | **薄指针**（能力已挂载时可见）：一行协议名 + 指向 skill |
| `prompts/capabilities/knowledge-base/reference/strategies*.md` | 可选 gotcha（多口径竞争 → 点名噪音 id） |
| **禁止** | 在 Rust 中硬编码中文「请敲除…」指令体 |

**Voice**：第三人称环境事实（「宿主接受如下线协议」「敲除后的 id 在后续检索命中中默认不进入回传可见面…」），非命令清单。

### 2.4 Host 执行（agent-loop）

| 步骤 | 时机 | 行为 |
|------|------|------|
| A. 解析 | **每轮** retrieve 模型输出入口（`apply_llm_output`，含纯 prose / 无 code） | `register_from_model_text` → 仅已见 UUID → 登记 knocked |
| B. 计数+过滤 | **bridge 回灌 Python 之前**（`RuntimeBridge` + knockout hook） | 已 knocked → `post_knock_hits++`；`<3` 从 JSON 剥离（print 干净）；`≥3` 保留并记 reexpose |
| C. 对齐 | codegen 收束 | `align_tool_results_no_count` + `take_reexposed` 贴标 |
| D. 贴标 | 本轮有 reexpose | `prompts/loop/knockout-reexposed.tmpl.md`（`[knockout_reexposed]`） |
| E. Telemetry | 可选 | register / suppress / reexpose Activity |

**状态归属**：本题 `IterationState` / run 级（与 `EvidencePool` 同寿）；**不**跨 session 持久化（V1）。

**计数语义（写死）**：

- 登记 knocked 时：`post_knock_hits = 0`。
- **仅敲除之后** 的检索命中累加；敲除前那次曝光不计。
- `post_knock_hits ∈ {1,2}` → 抑制；`>= 3` → 放行 + 贴标。

**跨工具**：dense / lexical / grep / struct_query（及凡 bridge 回传含 `chunk_id` 的检索方法）共用同一 `post_knock_hits`。

**装配点**：优先在 **bridge 写回 / observation 装配前** 过滤 structured `chunks[]`（与 visibility reseen 同阶段思想）；stdout 若再 `print` 全文，以 bridge 结构化路径为准（SaC 主路径）。

### 2.5 与 SELECTED 实现对照

| | SELECTED | KNOCKOUT |
|--|----------|----------|
| 线协议 | `SELECTED: #1, #3` | `KNOCKOUT: <uuid>, …` |
| 解析 | `helpers/selected.rs` | `helpers/knockout.rs`（或同族模块） |
| 解析对象 | alias `#n` → chunk_id | 直接 `chunk_id` |
| 副作用 | 引用 / citation 水合 | 敲除集 + 后续可见面过滤 |
| skill 教学 | knowledge-base「采用了哪些命中」 | knowledge-base「噪音敲除」节 |

---

## 3. 状态机

```text
(未登记) --检索命中--> 正常传入 LLM
              |
         模型输出 KNOCKOUT 含此 id
              v
         knocked (post_knock_hits=0)
              |
    再次检索命中 (任意工具)
              |
         post_knock_hits += 1
              |
    +-- < 3: 不传 LLM
    +-- >= 3: 传 LLM + [knockout_reexposed] observation
```

---

## 4. 第 3 次放行 observation

- 文件：`prompts/loop/knockout-reexposed.tmpl.md`
- 标签：`[knockout_reexposed]` … `[/knockout_reexposed]`（先登记 `host_markers.rs`）
- 占位：`{chunk_ids}`（上限截断，避免刷屏）
- 语气：第三人称「曾列入噪音敲除集；本轮检索再次命中且敲除后累计达三次，故重新进入可见面」——**禁止**命令式「必须采用/禁止采用」

---

## 5. 边界

| 机制 | 关系 |
|------|------|
| 相邻 S+L merge | 独立；计数键为 **最终回传 `chunk_id`** |
| working-set demote | 先敲除策略，再 char 预算 |
| claim_notes | V1 不自动联动 |
| verify 证据摘录 | **与 retrieve 可见面一致**（verify 不见被抑块） |
| web / 无 chunk_id | **不进本协议** |
| 预算 | 敲除不替代 token/轮次天花板 |

---

## 6. 实现清单

1. **文档**（本文）：SaC 集成 + 协议 + skill 职责 — ✅  
2. **Skill 教学**：`knowledge-base/SKILL.md` + `contract.md` + strategies gotcha — ✅  
3. **Host 解析 + 状态**：`helpers/knockout.rs`（`KnockoutState`）— ✅  
4. **Host 过滤**：bridge 回灌前剥离（stdout 主路径干净）+ 跨工具计数 — ✅（review 修复）  
5. **全轮登记**：`apply_llm_output` 统一 `register`（含无 code 散文轮）— ✅（review 修复）  
6. **Loop 贴标** + `host_markers` + `prompt_assets` — ✅  
7. **单测**：knockout 模块 lib 测 — ✅  

**V1 不做**：跨 session 持久化；host 自动多口径扫描；`#alias` 作敲除键；独立 native tool。

---

## 7. 验收标准

- [ ] 未输出 `KNOCKOUT` 时行为与无本机制时一致  
- [ ] skill 中可发现协议说明；Rust 无中文指令长文  
- [ ] 合法 `chunk_id` 登记后，敲除后第 1/2 次命中不进模型可见检索观察  
- [ ] 第 3 次命中：可见 + reexposed observation  
- [ ] dense 点名后 lexical 再命中计入同一计数器  
- [ ] 非法 / 未见 id 丢弃  
- [ ] host 标签已登记；parity 覆盖 loop 模板  

---

## 8. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-08-07 | 初稿：先见再敲；chunk_id；跨工具；敲除后 1/2 抑、3 放行贴标 |
| 2026-08-07 | **SaC 集成定案**：线协议 `KNOCKOUT`/`敲除`；skill 主教学；host 解析过滤；与 SELECTED 正交；实现清单与验收 |
| 2026-08-07 | **V1 落地**：skill/contract/strategies 教学；`KnockoutState` + codegen 过滤；loop reexpose；host_markers |
| 2026-08-07 | **Review 修复**：全轮 `register`；bridge 回灌前剥离（print 干净）；UUID 小写归一；避免双重计数 |
| 2026-08-07 | **指针**：综合评估后主默认改为「强制留存 + 宿主驱逐」；KNOCKOUT V2 倾向标注/harmful 计数（见 `2026-08-07-kb-skill-hardening-and-evidence-working-set-design.md` v2）。V1 硬滤代码可保留至 W4 拍板 |
| 2026-08-07 | **W4 关硬滤**：产品 `apply_to_bridge_data` / `align_*` 为 no-op；skill 不再教 KNOCKOUT 抑制；硬算法仅测路径 `apply_to_bridge_data_hard` |
