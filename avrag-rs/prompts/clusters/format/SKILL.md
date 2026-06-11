---
name: format
description: "输出形态：HTML、幻灯片、框架大纲、教学步骤"
disclose_at: synthesis
atomic: false
applicable_modes: [rag, search, chat]
---

## 何时加载

- **仅 Synthesis 阶段**披露；与 `writing` 簇对称
- Index 列出叶子能力；Agent 自选 **0~1** 个 `reference/<slug>.md`
- 选择来源：`format_choice` metadata 或 `format_hint`（可 override）
- 与 mandatory answer skill 并列叠加

## 核心指令

本簇决定**答案长成什么形态**（非怎么说）。answer agent 已提供证据与引用；你按所选格式重渲染，保留全部引用标记。

| slug | 输出 |
|------|------|
| `html-renderer` | 自包含 ` ```html ` 代码块 |
| `ppt-generation` | 结构化 JSON 幻灯片 |
| `framework-extraction` | `##`/`###` 层级框架 |
| `teaching` | 分步教学对话 |

各格式互斥：一次只选一种。

## Reference 路由表

| 文件 | 触发关键词 |
|------|------------|
| `reference/html-renderer.md` | html、图表、dashboard、可视化 |
| `reference/ppt-generation.md` | slides、PPT、deck、演示 |
| `reference/framework-extraction.md` | framework、outline、分解、结构化概览 |
| `reference/teaching.md` | teach、tutorial、step by step、walkthrough |

## 禁止

- 禁止同时加载多个 format reference
- 禁止剥离引用标记
- 禁止编造证据或引用
- 证据不足 fallback 须保留 `EVIDENCE_INSUFFICIENT_FALLBACK`
