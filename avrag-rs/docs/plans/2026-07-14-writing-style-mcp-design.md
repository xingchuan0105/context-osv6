# Writing Style MCP 设计 — Persona Skill + 统计指纹 Tools

日期：2026-07-14  
状态：设计定稿（待实现）  
前置：

- [`2026-07-07-persona-layer-design.md`](./2026-07-07-persona-layer-design.md) — 人格骰子维度与 PersonaCard 字段
- [`2026-07-07-write-refine-agent-loop.md`](./2026-07-07-write-refine-agent-loop.md) — WriteRefine 控制环（本 MCP **不**打包）
- HeavyTail 指纹实现：`crates/heavytail`（metrics / score / validator / diagnosis）

---

## 0. 一句话

把 Write 管线里两个可独立复用的能力拆成 **Writing Style MCP**：

| 能力 | 载体 | 执行者 |
|------|------|--------|
| **Persona 人格层** | MCP **skill + server 说明** | 宿主 Agent 按规则生成、落盘、内化 |
| **统计指纹反馈** | MCP **tools**（确定性） | 服务端 `heavytail` 计数 / band / brief |

**直写（compose）与改写（revise）共用同一张 PersonaCard**：先卡后写，同任务只生成一次。

不做：research、skeleton orchestrator、WriteRefine ReAct 整环、服务端 `persona.expand` LLM API、服务端 session 存 persona。

---

## 1. 目标与非目标

### 1.1 目标

1. 外部 Agent（Cursor / Claude / 自建）可只接本 MCP，获得「像谁写 + 像人写」两层约束。
2. **compose**：从零生成文章时即可读到 persona 规则，按卡起草。
3. **revise**：改写前读同一规则/同一卡，按卡改语气与句式，并用指纹工具闭环。
4. 指标与产品 Write 同源（cv / hapax / zipf / burstiness），避免第二套「人性化」定义。
5. MCP 无状态（指纹）；persona 真相源在 Agent 侧文件或会话块。

### 1.2 非目标

- 替代产品 `agent_type=write` 全流水线（调研 + 大纲 + 分段起草 + ReAct refine）。
- 在 MCP 内代写全文或调度 `write_refine_*` 工具环。
- 强制代码侧 RNG 掷骰（产品内路径可继续用代码 RNG；MCP 路径由 Agent 按 skill 选维，见 §4）。
- workspace / org / 计费 / ingest 耦合。

### 1.3 与产品 Write 的映射

| 产品 Write | 本 MCP |
|------------|--------|
| 每篇 `generate_persona` + 注入 draft/refine | skill `writing-persona` + 本地 `persona.json` |
| diagnose + WriteRefine band 反馈 | tools `style.fingerprint.*` |
| research / skeleton / 全环 | **不提供**（宿主自管） |

共享：维度池字段、PersonaCard schema、「放得下」泄漏原则、`StyleParams` band。

---

## 2. 架构

```text
┌──────────────────────────────────────────────────────────────┐
│  Host Agent（Claude / Cursor / 本产品侧外部编排）               │
│  compose 或 revise：acquire persona → 动笔 →（可选）指纹闭环   │
└───────────────┬────────────────────────────┬─────────────────┘
                │ skill / instructions       │ tools
                ▼                            ▼
     ┌─────────────────────┐      ┌──────────────────────────┐
     │ writing-persona     │      │ style.fingerprint.*      │
     │ 骰子规则 · schema    │      │ analyze / diagnose /     │
     │ 落盘 · 双模式 checklist│      │ compare（heavytail）     │
     └─────────────────────┘      └────────────┬─────────────┘
                                               ▼
                                    heavytail（纯库，无 AppState）
```

**删除测试**：去掉 PG / Milvus / workspace 后，skill 文本 + fingerprint tools 仍完整可用。

---

## 3. 双模式（compose / revise）

| 模式 | 用户意图 | 输入 | Persona | 指纹 |
|------|----------|------|---------|------|
| **compose** 直写 | 写一篇、起草、按主题成文 | 主题 / 要点 / 篇幅 | **第一句之前** acquire | 成文后 diagnose；差则同卡自改 |
| **revise** 改写 | 润色、去 AI 腔、按反馈改 | 已有正文 | **第一轮修改前** acquire | 改前 diagnose + 改后 compare |

### 3.1 共同点

1. 无卡则按 skill 生成并写入约定记录处；有卡则只读复用。
2. 同任务（同主题成文 + 后续精修）**禁止**静默换卡。
3. 内化 voice；正文禁止人设自报与 private_facts 泄漏。
4. 人设管「像谁」；band 管「像人」——二者解耦，band 阈值不随 persona 变。

### 3.2 不同点

| | compose | revise |
|--|---------|--------|
| 目标 | 在 persona 声音下**造**文 | 在 persona 声音下**修**文并抬 band |
| 事实来源 | 用户主题与材料 | **原稿事实/结构优先**，persona 只管怎么说 |
| 指纹主路径 | 成文后（+ 可选中段） | 每轮改前/改后 |

### 3.3 卡复用矩阵

| 场景 | 行为 |
|------|------|
| 先直写再改写同一篇 | 同一 `persona.json`，不重掷 |
| 新主题新开写 | 新任务目录或新 `persona-{slug}.json` |
| 用户明确「换人设」 | 允许覆盖；可选备份 `persona.prev.json` |
| 用户「同一人设另写一篇」 | 显式复用或 copy 同一卡 |

### 3.4 总工作流

```text
                ┌──────────────────────┐
                │  acquire_persona     │
                │  (skill → 记录处)     │
                └──────────┬───────────┘
                           │
          ┌────────────────┴────────────────┐
          ▼                                 ▼
   mode = compose                    mode = revise
   按卡从零生成                         按卡改已有稿
          │                                 │
          ▼                                 ▼
   diagnose（建议）                    diagnose → 改 → compare
          │                                 │
          └──────────── 交付 ───────────────┘
```

---

## 4. Persona = Skill + 说明（非 Tools）

### 4.1 包内布局

```text
writing-style MCP
├── instructions（server 级硬约束）
├── skills/
│   └── writing-persona.md
└── tools/                    # 仅统计，见 §5
    ├── style.fingerprint.analyze
    ├── style.fingerprint.diagnose
    └── style.fingerprint.compare
```

**不提供** `style.persona.roll` / `expand` / `render` 等 tool（与 skill 职责重复；expand 会引入服务端 LLM 与状态）。

### 4.2 Server `instructions`（硬约束）

1. **任何**写作任务（直写或改写）开始时：检查约定位置是否已有 PersonaCard。  
2. **没有** → 必须先按 skill `writing-persona` 生成并记录；**禁止**无卡开写。  
3. **有** → compose / revise 均按**同一张卡**执行；仅当用户明确要求换人设时覆盖。  
4. **compose**：大纲与正文全过程保持该声音；banned_phrases 与 signature_vocab 从第一段生效。  
5. **revise**：保留原稿事实；用 persona 改语气句式；每轮配合 `style.fingerprint.*`。  
6. 正文禁止自我介绍、禁止写入 private_facts / 小传字面。  
7. 统计 tools 的 brief 为硬约束清单；在**人设约束下**优先修 fail band。

一句话：

> **先卡后人设写作** — compose 与 revise 共用 PersonaCard；区别只在有无底稿、是否指纹闭环。

### 4.3 Skill：`writing-persona`

#### 触发

- 用户要求直写 / 起草 / 长文生成  
- 润色 / 去 AI 腔 / 多轮精修 / 改写  
- 调用 `style.fingerprint.*` 前若尚无 persona（仍应先 acquire 再测，以便后续按卡改）

#### Acquire（强制前置）

```text
acquire_persona(task):
  path = 约定记录处
  if exists(path): return load(path)
  card = generate_by_dice_rules(topic)   # 见下
  save(path, card)
  return card
```

- **topic**：compose 用用户主题；revise 可用原稿主题摘要或首段，但 **dims 仍按规则选**，禁止自由坍缩成「资深作者」。  
- MCP 路径：Agent 用自身随机性或主题 hash 在候选池中选维；卡上可记 `seed` 字符串便于复现。  
- 产品内路径（非本 MCP 必做）：可继续代码 `StdRng` 掷骰 + LLM 扩写（见 persona-layer 设计）。

#### 防坍缩：六维骰子（每维选 1）

| 维度 | 候选池 |
|------|--------|
| 出身背景 | 工程一线 / 学术转产业 / 媒体出身 / 自由职业 / 创业失败过 / 体制内出走 |
| 年龄段 | 28 上下 / 35 上下 / 45 上下（经历过行业周期） |
| 与主题关系 | 从业者 / 邻域旁观者 / 转行新兵 / 资深怀疑派 / 布道者 / 被坑过的用户 |
| 性情 | 急性子毒舌 / 温吞考究 / 冷幽默 / 谨慎克制 / 爱抬杠 |
| 修辞癖好 | 爱设问自答 / 爱生活类比 / 爱堆数字 / 爱讲小故事 / 爱下断言再让步 |
| 缺陷 | 容易跑题讲往事 / 对宏观议题不耐烦 / 过度自信 / 术语洁癖 |

**禁止**跳过选维直接写「资深从业者，理性务实」。  
在骰子约束下扩写姓名化身份、价值观、core_question、signature_vocab（8–15）、metaphor_domains、syntax_habits、banned_phrases、1–2 条虚构感官记忆、private_facts。  
与主题违和时：仅允许对**单一维度**最小合理化，并记 `dims_adjusted`。

#### 记录约定（任务级真相源）

| 环境 | 位置 | 说明 |
|------|------|------|
| 有工作区文件 | `./.writing-style/persona.json` | 默认；任务目录下 |
| 多篇并行 | `persona-{slug}.json` | slug = 主题短 hash |
| 仅会话 | 固定块 `PERSONA_CARD` | 与 JSON 同 schema |

规则：生成后立即写入；后续先读后写；用户说换人设才覆盖；private_facts 不进给终端用户的摘要。

#### PersonaCard schema（wire / 落盘）

```json
{
  "seed": "optional-replay-key",
  "dims": {
    "background": "工程一线",
    "age_band": "35上下",
    "relation": "从业者",
    "temperament": "冷幽默",
    "rhetoric": "爱生活类比",
    "flaw": "容易跑题讲往事"
  },
  "identity": {
    "name": "老周",
    "role": "…",
    "era_context": "…"
  },
  "values": ["…"],
  "core_question": "…",
  "voice": {
    "signature_vocab": ["…"],
    "metaphor_domains": ["…"],
    "syntax_habits": ["…"],
    "banned_phrases": ["赋能", "抓手", "闭环", "综上所述"]
  },
  "private_facts": ["仅防泄漏，勿写进正文"],
  "meta": {
    "created_for": "compose | revise | either",
    "topic": "用户主题摘要"
  }
}
```

`meta.created_for` 仅审计；**不限制**后续用于另一模式。

#### 模式 A checklist：compose

- [ ] 已 acquire persona  
- [ ] 大纲可带 core_question / values 立场（可选）  
- [ ] 开篇不做「我是某某」式自报  
- [ ] 性情 + 修辞贯穿全文，非只涂末段  
- [ ] signature_vocab 自然出现（约 2–5 处/短文），不堆砌  
- [ ] 避开 banned_phrases  
- [ ] 成文后建议 `style.fingerprint.diagnose`；明显 fail 则同卡自改一轮  

#### 模式 B checklist：revise

- [ ] 已 acquire（或复用直写留下的卡）  
- [ ] `diagnose` 原稿  
- [ ] 按 persona + brief 修改；事实以原稿为准  
- [ ] `compare`；未达标且轮次未满则迭代（**persona 不变**）  
- [ ] 无 identity.name / private_facts 字面泄漏  

#### 动笔上下文模板（Agent 自拼）

```text
[Persona — 内化，勿写入正文]
{身份一句} | {temperament} | {rhetoric}
禁用：{banned_phrases}
可用词感：{signature_vocab}
句法：{syntax_habits}

[任务类型] compose | revise
[素材] 主题… 或 原稿…

[指纹]（若有）
{diagnose.brief_zh}

请以该人设声音完成直写或改写；优先修复 fail 指标。
```

#### 放得下（泄漏）

- private_facts / 过长感官细节不得进入正文。  
- 命中则删改，不做硬失败。  
- 本 MCP 不强制提供 `check_leak` tool；skill 要求 Agent 自检。产品侧可继续用 `heavytail::persona::check_persona_leakage`。

---

## 5. 统计指纹 Tools

实现复用 `heavytail`：`analyze_sentences` / `validate` / `composite` / `render_diagnosis_brief_zh` 思路；入口宜为纯 `&str`（内部 segment + tokenize），不绑 WriteRefine workspace。

### 5.1 指标与默认 band

与 `StyleParams::default` + `validator` / `score::bands_for` 对齐：

| 指标 | 含义 | 默认 target（约） |
|------|------|-------------------|
| cv | 句长变异系数 | style.cv×0.85 … ×1.15（cv 默认 0.75） |
| hapax_ratio | 内容词 hapax 占比 | 0.35–0.55 |
| burstiness | 句长 lag-1 自相关 | 0.1–0.6 |
| zipf_exponent | rank-freq 幂律 | 0.8–1.3 |

复合分 \(S = 0.4\cdot len + 0.2\cdot burst + 0.25\cdot hapax + 0.15\cdot zipf\)（len 来自 W1 分位数对齐）。

### 5.2 Tool 面

| Name | 输入 | 输出 |
|------|------|------|
| `style.fingerprint.analyze` | `text`, `style_params?` | FingerprintReport 摘要（句长序列可截断） |
| `style.fingerprint.diagnose` | `text`, `style_params?`, `reservoir?` | bands[]、score_s、brief_zh、word_hints |
| `style.fingerprint.compare` | `before`, `after`, `style_params?` | ΔS、band 变好/变坏、短结论文本 |

可选 P1：`style.fingerprint.directives`（句级加长/缩短候选，对齐 sensitivity/compiler 思想）。

### 5.3 diagnose 输出示例

```json
{
  "fingerprint": {
    "mean_length": 21.2,
    "cv": 0.71,
    "hapax_ratio": 0.41,
    "zipf_exponent": 1.05,
    "burstiness": 0.28,
    "total_tokens": 812,
    "vocab_size": 430
  },
  "bands": [
    { "metric": "cv", "actual": 0.71, "target": [0.6375, 0.8625], "passed": true },
    { "metric": "hapax_ratio", "actual": 0.41, "target": [0.35, 0.55], "passed": true },
    { "metric": "burstiness", "actual": 0.28, "target": [0.1, 0.6], "passed": true },
    { "metric": "zipf_exponent", "actual": 1.05, "target": [0.8, 1.3], "passed": true }
  ],
  "score_s": 0.87,
  "brief_zh": "## 诊断摘要\n…",
  "word_hints": []
}
```

### 5.4 语言与确定性

- v1 主中文（现有 jieba + segment）；英文可降级或 `unsupported`。  
- 同一 unicode 文本 analyze/diagnose **bit-stable**。  
- 全链路 **零 LLM**。

---

## 6. 部署形态

| 模式 | 用途 |
|------|------|
| **A. 独立 stdio/SSE MCP binary** | 第三方 Agent；`bins/writing-style-mcp` 或等价；依赖 heavytail，无 PG/Milvus |
| **B. 产品 `/api/v1/mcp` 挂载（可选）** | `style.*` 命名空间；纯文本工具，**不要求** workspace_id |

逻辑共享同一 catalog 与 heavytail 封装。  
**禁止** MCP handler 调用 `conversation().execute(agent_type=write)`。

鉴权（B）：沿用现有 MCP 网关即可；tools 不读用户知识库。

---

## 7. 实现分期

| 期 | 交付 | 验收 |
|----|------|------|
| **P0** | heavytail 纯文本 `analyze`/`diagnose` API + 3 fingerprint tools + 最小 MCP skeleton | fixture `human_like` / `ai_like` 可分；同文两次结果一致 |
| **P1** | `instructions` + `skills/writing-persona.md`（含 compose/revise） | 文档评审：双模式路径与落盘约定无歧义 |
| **P2** | 独立 MCP 可运行 + 宿主示例 prompt（compose 一篇 + revise 一轮） | 手工：无卡拒写；有卡直写；同卡改写；diagnose/compare 可见 |
| **P3** | 可选产品 MCP 挂载；可选 `directives`；与产品 persona 字段对齐注释 | 外部 Agent 3 轮 revise S 不降；compose 后 band 可测 |

Persona 服务端状态与 expand：**明确不做**。

---

## 8. 风险与对策

| 风险 | 对策 |
|------|------|
| Agent 跳过 skill 直接写 | instructions 硬约束 + skill 触发描述写清「禁止无卡开写」 |
| 自由发挥导致人设坍缩 | 强制六维候选池；skill 写明反例 |
| 直写后改写换卡导致风格跳变 | 任务级单文件；同目录默认复用 |
| 指纹与人设打架（修 band 毁语气） | brief 要求「在人设下修句」；软结束，不强制全 band |
| 与产品 Write 双轨漂移 | schema/维度池/band 以 heavytail + 本文为源；改 band 先改库 |

---

## 9. 验收口径（设计层）

1. **compose**：新任务第一次成文前生成并保存 persona，再出正文。  
2. **revise**：读已有卡（或生成一次），改写后卡未变。  
3. **compose→revise 同篇**：不重掷。  
4. 正文无人设自报 / private_facts 泄漏（抽检）。  
5. 指纹 tools 可对任意中文稿给出四维 band 与 brief。  
6. MCP 包内可见 skill 全文与 instructions，无需再查产品 Write 源码即可执行双模式。

---

## 10. 参考代码与文档

| 资源 | 路径 |
|------|------|
| Persona 设计（产品内 RNG+LLM） | `avrag-rs/docs/plans/2026-07-07-persona-layer-design.md` |
| 指纹 metrics / bands | `avrag-rs/crates/heavytail/src/metrics.rs`, `score.rs`, `validator.rs` |
| 诊断 brief | `avrag-rs/crates/heavytail/src/diagnosis.rs` |
| Write 模式用户文档 | `frontend_next/public/docs/write-mode.md` |
| 现有产品 MCP catalog | `avrag-rs/crates/transport-http/src/mcp/catalog.rs` |

---

## 11. 开放问题（实现时再定）

1. skill 载体：独立 MCP 包内 markdown vs 与 `agent-tools` progressive skill 格式对齐。  
2. `reservoir` 是否接受调用方传入 signature_vocab 以增强 word_hints（建议 P2：可选参数）。  
3. 是否在 P3 增加**无 LLM** 的 `style.persona.sample_dims`（仅返回六维，仍由 Agent 扩写）——默认 **不做**，除非实测 Agent 经常跳过选维。

---

## 12. 变更摘要（相对「整包 Write Agent MCP」）

| 不做 | 做 |
|------|-----|
| 全链路 Write MCP | Style 协处理器 MCP |
| persona.* tools + 服务端 expand | **writing-persona skill** + 落盘约定 |
| 仅改写 | **compose + revise 双模式** |
| 第二套人性化指标 | **heavytail 同源 band** |
