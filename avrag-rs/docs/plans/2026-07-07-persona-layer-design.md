# 人格层（Persona Layer）设计 — 斯坦尼体系嵌入写作 Agent Loop

日期：2026-07-07
状态：设计稿
前置：`2026-07-07-write-refine-agent-loop.md`（WriteRefine ReAct loop）、band 人类标定（`heavytail/src/calibration.rs`）

## 0. 一句话

把「写文章」重定义为「演员带着角色小传进场演戏」：**小传（PersonaCard）** 是跨全文的常驻人格，**场景卡（SceneCard）** 是每节的规定情境与目标，二者分别注入 system prompt 与 section brief；小传永不直接出现在正文（「放得下」原则），由词汇水库与泄漏检查间接驱动。

**PersonaCard 不预设、不复用：每次写作运行时由 LLM 现场生成一个随机人格**（代码侧掷骰子定维度 → LLM 扩写成小传），同一篇文章内固定，跨文章各不相同——随机人格 = 随机风格指纹，消除「每篇都是同一个 AI 在说话」的跨文章一致性signature。生成结果落盘存档（含种子），可回放但不可预设。

## 1. 理论 → 工程映射

| 表演概念 | 工程落点 | 现有代码挂载处 |
|---------|---------|---------------|
| 角色小传 | `PersonaCard`（运行时随机生成，落盘存档） | 新 `heavytail/src/persona.rs` |
| 规定情境 | PersonaCard.identity / era_context | draft & refine system prompt 段 |
| 魔力假使 | system prompt 固定句式「你就是此人，此刻面对这位读者」 | `render_persona_system_zh()` |
| 七问 / 场景目标 | `SceneCard`（每节：目标/障碍/战术） | `SkeletonSection.scene` 新字段 |
| Meisner 当下反应 | `reader_state` 跨节链式传递 | `draft_sections()` 循环内 |
| 情感记忆 | PersonaCard.sensory_memories（触发词→具身细节） | section brief 条件注入 |
| 专属词汇 cheat sheet | PersonaCard.voice.signature_vocab | **并入 reservoir** → `write_refine_lexical` |
| 放得下原则 | private_facts 只用于泄漏检查，不渲染 | 新 `check_persona_leakage()` |
| 角色课题 | PersonaCard.core_question | skeleton outline prompt 注入 |

## 2. 数据结构与生成

### 2.1 随机生成协议（两步：掷骰子 → 扩写）

**为什么不能让 LLM 直接「随机想一个人格」**：不加约束的自由生成会模式坍缩——十次里八次是「资深从业者，理性务实，喜欢用类比」。随机性必须来自代码侧 RNG，LLM 只负责把骰子结果扩写成血肉。

**Step 1 — 代码侧采样维度**（`persona.rs`，`StdRng::seed_from_u64(seed)`）：

| 维度 | 候选池（示例，每维 5–10 项） |
|------|------------------------------|
| 出身背景 | 工程一线 / 学术转产业 / 媒体出身 / 自由职业 / 创业失败过 / 体制内出走 |
| 年龄段与代际 | 28 上下（互联网原住民）/ 35 上下 / 45 上下（经历过行业周期） |
| 与主题的关系 | 从业者 / 邻域旁观者 / 转行新兵 / 资深怀疑派 / 布道者 / 被坑过的用户 |
| 性情 | 急性子毒舌 / 温吞考究 / 冷幽默 / 谨慎克制 / 爱抬杠 |
| 修辞癖好 | 爱设问自答 / 爱生活类比 / 爱堆数字 / 爱讲小故事 / 爱下断言再让步 |
| 缺陷 | 容易跑题讲往事 / 对宏观议题不耐烦 / 过度自信 / 术语洁癖 |

每维掷一次骰子，组合空间 >10⁴，保证跨文章多样性；`seed` 记录进产物，可复现。

**Step 2 — LLM 扩写**（一次调用，temperature 0.9）：

```
generate_persona(llm, topic, dims: SampledDims, tokens_used) -> Result<PersonaCard>
```

prompt 给定主题 + 采样维度，要求输出 JSON（复用 `llm::parse_json_object` 解析，失败重试 1 次）：
在骰子约束下补全姓名化身份、价值观、角色课题、signature_vocab（8–15 个该人格会挂嘴边的词）、
metaphor_domains、syntax_habits、banned_phrases、1–2 条 sensory_memories（虚构但具身）、
flaws、private_facts。「与主题的关系」维度保证人格对该 topic 可信；其余维度提供张力。

### 2.2 PersonaCard schema（LLM 生成产物示例）

```yaml
# <run_dir>/personas/topic-01.persona.yaml   ← 运行时生成落盘，非预设
seed: 8412379
dims: {背景: 工程一线, 年龄: 35上下, 关系: 从业者, 性情: 冷幽默, 修辞: 爱生活类比, 缺陷: 容易跑题讲往事}
identity:
  name: 老周
  role: 推理平台工程师，读者是刚接触 LLM infra 的后端同事
  era_context: 2020年代大模型落地潮
values:
  - 数字不撒谎，benchmark 之前先讲清测的是什么
core_question: 工程上的省，省的到底是谁的钱
voice:
  signature_vocab: [显存账本, 摊开算, 首token, 掉链子, 水位线]
  metaphor_domains: [记账, 仓库调度, 老小区停车]
  syntax_habits: [关键结论用极短句收尾, 类比之后马上给真实数字]
  banned_phrases: [赋能, 抓手, 闭环, 综上所述]
sensory_memories:
  - trigger: 显存溢出
    detail: 凌晨两点看着 OOM 日志把 batch size 从 32 改到 24 的那次值班
flaws:
  - 讲着讲着容易岔到自己踩过的坑
private_facts:
  - 上一家公司因推理成本失控裁掉了他所在的团队
```

Rust 侧：

```rust
// heavytail/src/persona.rs
pub struct PersonaCard {
    pub id: String,
    pub version: u32,
    pub identity: Identity,
    pub values: Vec<String>,
    pub core_question: String,
    pub voice: Voice,
    pub sensory_memories: Vec<SensoryMemory>,
    pub flaws: Vec<String>,
    pub private_facts: Vec<String>,
}

pub struct SceneCard {          // 七问压缩为四字段
    pub objective: String,      // 我想让读者理解/感受/做什么
    pub obstacle: String,       // 读者疲劳/怀疑/知识缺口
    pub tactic: String,         // 克服行动：案例/设问/数据/类比
    pub reader_state_out: String, // 本节结束时读者应处状态（喂给下一节）
}
```

### 2.2 渲染约定

- `render_persona_system_zh(&PersonaCard) -> String`：身份 + 价值观 + 课题 + 语言习惯 + 禁用词 + 魔力假使句式。**不含** private_facts 与 sensory_memories 全文（后者按触发词命中才进 brief）。
- `render_scene_brief_zh(&SceneCard, reader_state_in) -> String`：【本节场景】块。

## 3. 三层注入点

### 3.1 Skeleton（`heavytail/src/skeleton.rs`）

- `plan_skeleton()` user prompt 追加 persona 摘要（身份/课题/价值观 ~100 字），让大纲天然带角色立场。
- 新增第二次轻量 LLM 调用 `plan_scenes(llm, skeleton, persona)`：对每节回答七问，产出 `SceneCard`，写入 `SkeletonSection.scene: Option<SceneCard>`。一次调用产全部节，token 成本 ~1k。

### 3.2 Draft（`heavytail/src/draft.rs`）

- `draft_sections()` 增参 `persona: Option<&PersonaCard>`：
  - system = `PLAIN_SYSTEM` + `render_persona_system_zh()` + 现有 priming；
  - `build_section_brief()` 追加 `render_scene_brief_zh(scene, reader_state_in)`；
  - **Meisner 链**：`reader_state_in[i] = scene[i-1].reader_state_out`，首节用「冷启动读者」；
  - sensory_memories：若本节 key_points 命中 trigger，把 detail 以「可用的亲历素材（可改写，勿照抄）」注入 brief。

### 3.3 Refine（`app-chat/src/writer/refine_loop.rs`）

- `RefineContext` 增 `persona: Option<PersonaCard>`；
- `build_system_prompt()`（现 L968）在 mandatory skills 之后追加 persona 块，标注「**内化人格：影响措辞与取舍，禁止在正文自我介绍或引用小传事实**」；
- reservoir 强化：`build_refine_reservoir(topic, fp)` 增参 persona，把 `signature_vocab + metaphor_domains 词` 并入（去重、过滤已在正文的词）。直接修复当前 reservoir 弱（topic-01 仅「一篇、面向、科普」3 词）的问题，给 `write_refine_lexical` 的 `repeat_term` 提供有语义的候选；
- 诊断 brief 不变（指标驱动），persona 只改变「怎么改」的语感。

## 4. 「放得下」泄漏防线

```rust
// heavytail/src/persona.rs
pub fn check_persona_leakage(ws: &DraftWorkspace, p: &PersonaCard) -> Vec<LeakHit>;
```

- 检查 private_facts / sensory_memories.detail 与正文的字面重叠（≥8 字连续窗口或 >60% 字符重合的句子）；
- 命中 → (a) refine loop 里生成一条 revise hint「s{id} 疑似自传式泄漏，改为隐式驱动」；(b) 写 orchestrator 的 degrade_trace `persona:leak` 警告；
- 不做硬失败：小传是燃料，不是枷锁。

## 5. 配置与实验通路

- 写管线入口（`plan_skeleton` 之前）调用 `generate_persona`，同一篇文章内 PersonaCard 固定；
- 落盘：`<run_dir>/personas/topic-NN.persona.yaml`（含 seed 与 dims），审计与回放两用；
- 实验参数：`--persona-seed N`（复现实验）、`--persona-replay <file>`（加载**已生成存档**重跑，用于 debug，非预设人格）、`--no-persona`（对照臂）；
- M4 新对比臂 `arm-b-persona`：每 topic 现场生成随机人格 → skeleton/draft/refine 全程注入；
- Persona 与 band 解耦：band 是「像人」的下限（全局人类标定，不随 persona 变），persona 是「这次像谁」。随机人格天然打散跨文章的风格指纹一致性，这本身就是反检测收益。

## 6. 分期

| 期 | 内容 | 验收 |
|----|------|------|
| P0 | persona.rs：维度池 + RNG 采样 + `generate_persona` + 落盘；system prompt 注入（draft/refine）+ signature_vocab 并入 reservoir | 同一 topic 连生 5 张 persona 两两不同（dims 不重复≥4/6 维）；topic-01 重跑 reservoir ≥10 有效词、正文含 signature_vocab、无 banned_phrases |
| P1 | SceneCard 生成 + section brief 注入 + reader_state 链 | 人工评：节间衔接有「回应上一节」感；band 不退化 |
| P2 | 泄漏检查 + degrade trace + `arm-b-persona` 实验臂 | vs arm-b：band 持平；跨 topic 风格多样性↑（10 篇 pairwise 词汇重叠度下降）；单篇内 voice 一致性人工评分↑ |

## 7. Token 成本估算

- `generate_persona` 一次性 ~1.5k tokens / 篇；
- persona system 块 ~400 字 × refine 每轮重发：topic-01 6 轮 ≈ +2.4k 字 ≈ +5%；
- SceneCards 一次性 ~1k tokens；合计可接受。

## 8. 开放问题

1. SceneCard 由 LLM 生成还是人工写？MVP：LLM 生成 + 落盘可人工改（`<run_dir>/scenes/topic-NN.yaml`）。
2. 维度池本身是否需要随版本迭代扩充（避免池子太小导致「随机但眼熟」）——P2 后按跨文章重叠度数据决定。
3. 随机人格与「事实正确性」的边界：sensory_memories / private_facts 是虚构的，必须只影响语气与取舍，禁止作为事实证据写进正文（泄漏检查 + prompt 标注双保险）；涉及真实数据仍走 research 工具。
4. 极端骰子组合可能与主题违和（如「被坑过的用户」写纯数学主题）：Step 2 prompt 允许 LLM 对单一维度做最小幅度合理化改写，改写记录进落盘档案。
