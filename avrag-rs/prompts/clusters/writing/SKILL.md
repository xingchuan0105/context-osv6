---
name: writing
description: "Writing style layer: neutral prose by default; load at most one style reference when needed"
disclose_at: synthesis
atomic: false
applicable_modes: [rag, search, chat]
version: "3.0"
---

## 加载边界

- 出现在 **撰写最终答案** 阶段；不进入检索轮。
- 默认中性散文；同一答复最多 **1** 个 `reference/<slug>.md`。
- slug 来自请求提示（如 `writing_ref` / `writing_hint`）或用户语气。

## 作用范围

本说明只调整 **怎么写**（语气与文体）。证据来源与引用协议已由材料 / capability / 检索回传决定：

- 引用标记（`SELECTED: #n`、`[[web:n]]`、`[[cite:…]]` 等）是材料协议的一部分，改写文体时仍保留。
- 风格层不新增证据，也不撤销「未覆盖」状态。

| slug | 适用 |
|------|------|
| `tone` | 匹配用户语气（专业/随意/友好/正式等） |
| `concise` | 简短直接 |
| `professional` | 商务、要点前置、执行摘要 |
| `academic` | 学术论证、审慎措辞 |
| `storytelling` | 叙事、类比、场景化 |
| `brainstorming` | 请求模糊、需澄清后再答 |

未指定风格：清晰中性散文。

## 可选参考

| 文件 | 何时看 |
|------|--------|
| `reference/tone.md` | 语气/风格，无明确文体名 |
| `reference/concise.md` | 简短 / TL;DR |
| `reference/professional.md` | 商务 / 邮件 / 汇报 |
| `reference/academic.md` | 学术 / 文献 |
| `reference/storytelling.md` | 故事 / 类比 |
| `reference/brainstorming.md` | 模糊探索 |
