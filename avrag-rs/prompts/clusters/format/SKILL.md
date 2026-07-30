---
name: format
description: "Output shape: HTML, slides, outline, or teaching steps — load at most one format reference"
disclose_at: synthesis
atomic: false
applicable_modes: [rag, search, chat]
version: "3.0"
---

## 加载边界

- 出现在 **撰写最终答案** 阶段。
- 同一答复最多 **1** 个 `reference/<slug>.md`。
- slug 来自格式提示（如 `format_choice` / `format_hint`）或用户关键词。

## 作用范围

本说明决定答案 **形态**（不是语气、也不是证据裁决）。材料中的事实与引用标记在重排后仍是同一集合：

| slug | 输出形态 |
|------|----------|
| `html-renderer` | 自包含 HTML 代码块 |
| `ppt-generation` | 结构化幻灯片 JSON |
| `framework-extraction` | 层级大纲（`##` / `###`） |
| `teaching` | 分步教学对话 |

引用标记随内容保留；缺口在材料中已是未覆盖的，换格式后仍是未覆盖。

## 可选参考

| 文件 | 触发词例 |
|------|----------|
| `reference/html-renderer.md` | html、图表、dashboard、可视化 |
| `reference/ppt-generation.md` | slides、PPT、演示 |
| `reference/framework-extraction.md` | framework、大纲、结构化概览 |
| `reference/teaching.md` | teach、tutorial、step by step |
