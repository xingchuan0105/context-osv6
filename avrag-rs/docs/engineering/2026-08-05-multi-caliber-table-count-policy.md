# 表计数多口径：揭露差异，不裁决唯一整数

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-05 |
| 状态 | **产品/评测策略已拍板；skill 已改；金标 q078/q088 类已改** |
| 动机 | full149 q078：81（行）vs 57（活动名去重）均可理解；旧评测只认 81 逼模型「裁决」 |
| 相关 | `strategies-tables` FS3b；`how-to-read-tables` B2；golden `rubric_notes` |

---

## 1. 策略

对「同一张角色展开表、问有多少活动/行」类问题：

| 口径 | 含义 | 回传常见来源 |
|------|------|----------------|
| **行 / 条目** | 角色×活动 展开行 | `total_hits`、表序行 COUNT、编号连续行 |
| **去重项** | 按活动名或活动号合并 | 回传中可复核的去重统计（须标明键） |

**终答工作 = 分口径揭露检索/统计现状**，不是替用户裁定「只许一个整数」。

**不合格形态：**

- 只报去重数、不提行数、不标口径；
- 编造第三种无回传支撑的数；
- 用禁令口吻否定另一种已可见口径。

---

## 2. 已改文件

| 文件 | 改动 |
|------|------|
| `prompts/.../strategies-tables.md` | 多口径默认叙述 + FS3b + gotcha 去「裁决行数唯一」语气 |
| `prompts/.../strategies.md` | 薄层：行数与去重可并列 |
| `prompts/.../how-to-read-tables.md` | B2 改为「观察侧完整写法」 |
| `tests/rag_quality/golden_set_realistic.json` | 概念阶段活动数、验证/发布活动数：expected + rubric 多口径 |
| skillopt `skills/strategies*.md` | 与 product 同步 |

---

## 3. 评测判分（judge rubric）

概念阶段（原只认 81）：

- **correct**：行口径（81）+ 去重口径并标明键；或至少行口径写清「行/条目」且可并列去重。
- **incorrect**：仅 57 且不提行数/不标去重。
- 无据第三套数 → FA 扣分。

验证/发布（59/30）：

- **必须**出现行口径 59 与 30；
- 去重可并列；不得把 59 改成 58 或宣称缺 309。

实现侧：依赖 judge 读 `rubric_notes` / `expected_answer`（已写入 golden）；`must_include` 含 81+去重+口径 或 59+30。

---

## 4. 与 CORRECT_UNGROUNDED

q030 路径问题仍用路径标签；**多口径表计数** 走 **内容+rubric** 调整，不混为 CORRECT_UNGROUNDED。
