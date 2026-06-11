---
name: writing
description: "文体与语气：默认中性散文，按需加载一种写作风格"
disclose_at: synthesis
atomic: false
applicable_modes: [rag, search, chat]
---

## 何时加载

- **仅 Synthesis 阶段**披露；不进检索轮 ClusterIndex
- Index 仅 1 条 cluster description；默认中性散文，不加载 reference
- 最多加载 **1 个** `reference/<slug>.md`
- 选择来源：`writing_ref` metadata 或 `writing_hint`（Agent 可 override）

## 核心指令

本簇是**写作风格叠加层**。answer agent 已决定证据来源、引用格式与 fallback 策略；你只调整 prose 风格，不二次判断证据或发明引用。

| slug | 适用场景 |
|------|----------|
| `tone` | 匹配用户语气偏好（专业/随意/友好/正式等） |
| `concise` | 简短直接、无废话 |
| `professional` | 商务沟通、BLUF、执行摘要 |
| `academic` | 学术文体、论证结构、审慎措辞 |
| `storytelling` | 叙事、类比、场景化讲解 |
| `brainstorming` | 请求模糊、需澄清后再作答 |

未指定风格时：保持清晰中性散文，遵守 orchestrator §5 引用契约。

## Reference 路由表

| 文件 | 触发 |
|------|------|
| `reference/tone.md` | 用户关注语气/风格，无明确文体 |
| `reference/concise.md` | brief / TL;DR / 简洁 |
| `reference/professional.md` | 商务 / 邮件 / 汇报 / BLUF |
| `reference/academic.md` | 学术 / 文献 / 审慎论证 |
| `reference/storytelling.md` | 故事 / 类比 / 场景化 |
| `reference/brainstorming.md` | 模糊探索性请求 |

## 禁止

- 禁止同时加载多个 reference（最多 1 个）
- 禁止剥离 answer agent 提供的引用标记（`[[cite:…]]`、`[[n]]`）
- 禁止在无证据时发明引用
- 证据不足 fallback 时须保留 `EVIDENCE_INSUFFICIENT_FALLBACK` 标记
