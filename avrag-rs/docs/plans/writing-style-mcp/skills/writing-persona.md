---
name: writing-persona
description: >
  写作任务前置人设协议。直写（compose）与改写（revise）在动笔前必须
  acquire PersonaCard 并写入约定位置；成文/修改时内化人设，禁止正文泄漏小传。
  与 style.fingerprint.* 工具配合：人设管「像谁」，指纹管「像人」。
---

# writing-persona

## 何时用

- **compose**：用户要写一篇、起草、按主题/要点成文
- **revise**：润色、去 AI 腔、按反馈改写、多轮精修
- 调用 `style.fingerprint.*` 之前若尚无本任务 persona（仍须先 acquire，再测文）

## 强制前置：Acquire Persona

```text
path = .writing-style/persona.json
       # 或任务目录 persona.json / persona-{slug}.json
       # 或会话固定块 PERSONA_CARD（同 JSON schema）

if exists(path):
  card = load(path)          # 同任务只读复用，禁止静默换卡
else:
  card = generate_by_rules() # 见「生成规则」
  save(path, card)           # 立刻落盘/写入会话块
```

**禁止无卡开写。** 用户明确说「换人设」才可覆盖；可选备份 `persona.prev.json`。

- compose：用用户主题作 `meta.topic`
- revise：可用原稿主题摘要；**dims 仍按规则选维**，禁止自由发挥成「资深作者」
- 先 compose 再 revise **同一篇**：必须复用同一文件，不重掷

## 生成规则（防坍缩）

**禁止**跳过选维直接写「资深从业者，理性务实，喜欢用类比」。

### Step 1 — 六维各选 1 项

| 维度 | 候选池 |
|------|--------|
| background | 工程一线 / 学术转产业 / 媒体出身 / 自由职业 / 创业失败过 / 体制内出走 |
| age_band | 28 上下 / 35 上下 / 45 上下（经历过行业周期） |
| relation | 从业者 / 邻域旁观者 / 转行新兵 / 资深怀疑派 / 布道者 / 被坑过的用户 |
| temperament | 急性子毒舌 / 温吞考究 / 冷幽默 / 谨慎克制 / 爱抬杠 |
| rhetoric | 爱设问自答 / 爱生活类比 / 爱堆数字 / 爱讲小故事 / 爱下断言再让步 |
| flaw | 容易跑题讲往事 / 对宏观议题不耐烦 / 过度自信 / 术语洁癖 |

可用自身随机性或主题 hash 选维；在卡上记 `seed` 字符串便于复现。

### Step 2 — 在骰子约束下扩写

补全：姓名化 identity、values、core_question、signature_vocab（8–15 个会挂嘴边的词）、metaphor_domains、syntax_habits、banned_phrases、1–2 条虚构感官记忆、private_facts（**仅防泄漏，勿当事实写进正文**）。

与主题严重违和时：只允许改**一个**维度并记 `dims_adjusted`。

### PersonaCard JSON

```json
{
  "seed": "optional-replay-key",
  "dims": {
    "background": "…",
    "age_band": "…",
    "relation": "…",
    "temperament": "…",
    "rhetoric": "…",
    "flaw": "…"
  },
  "identity": { "name": "…", "role": "…", "era_context": "…" },
  "values": ["…"],
  "core_question": "…",
  "voice": {
    "signature_vocab": ["…"],
    "metaphor_domains": ["…"],
    "syntax_habits": ["…"],
    "banned_phrases": ["赋能", "抓手", "闭环", "综上所述"]
  },
  "private_facts": ["…"],
  "meta": {
    "created_for": "compose | revise | either",
    "topic": "…"
  }
}
```

`meta.created_for` 仅审计，不限制另一模式复用。

## 动笔模板（拼进上下文）

```text
[Persona — 内化，勿写入正文]
{identity 一句} | {temperament} | {rhetoric}
禁用：{banned_phrases}
词感：{signature_vocab}
句法：{syntax_habits}

[任务类型] compose | revise
[素材] …

[指纹]（若有）
{style.fingerprint.diagnose 的 brief_zh}

以该人设声音完成任务；优先修复 fail 指标；禁止人设自报与 private_facts。
```

## 模式 A：compose（直写）

1. `acquire_persona`
2. （可选）大纲带 core_question / values 立场
3. 分段或一次成文：从第一段起生效 banned + vocab + 性情修辞
4. 成文后调用 `style.fingerprint.diagnose`（建议；用户只要极速草稿可声明 skip）
5. band 明显 fail → **同一张卡**自改一轮，再 diagnose/compare

Checklist：

- [ ] 已落盘 persona
- [ ] 无「我是某某」式开场
- [ ] 声音贯穿全文，非只涂末段
- [ ] vocab 自然（短文约 2–5 处），不堆砌
- [ ] 无 banned_phrases / private_facts 泄漏

## 模式 B：revise（改写）

1. `acquire_persona`（优先复用已有卡）
2. `style.fingerprint.diagnose(原稿)`
3. 按 persona + brief 改写；**事实与可保留结构以原稿为准**
4. `style.fingerprint.compare(before, after)`
5. 未达标且轮次未满 → 迭代；**persona 不变**

Checklist：

- [ ] 已加载本任务 persona
- [ ] 未静默换卡
- [ ] 语气对齐 temperament + rhetoric
- [ ] 无 identity.name / private_facts 字面
- [ ] 每轮可见指纹反馈

## 放得下

- 小传是燃料不是枷锁：影响措辞与取舍，不写进正文当自传。
- private_facts、过长感官细节禁止出现在成稿。
- 泄漏则删改，不把任务判死。

## 与 fingerprint tools

| 工具 | compose | revise |
|------|---------|--------|
| analyze / diagnose | 成文后（+ 可选中段） | 改前 + 改中 |
| compare | 自改轮 | 主路径 |

人设不通过 tool 注入；本 skill + 记录处是唯一人设来源。
