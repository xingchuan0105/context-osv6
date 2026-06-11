# CDS v1.1 — Cluster Disclosure Spec

| 项目 | 内容 |
|------|------|
| 状态 | 已采纳（2026-06-09） |
| 关联 | ADR-0007、ADR-0008 |
| 取代 | 扁平 `prompts/skills/*` 一级叶子 registry |

## 1. 三分法

| 族 | 路径 | Registry | 调用方 |
|----|------|----------|--------|
| **A Cluster** | `prompts/clusters/<id>/SKILL.md` | 是（id = 目录名） | ReAct ContextAssembler |
| **B Loop** | `orchestrators/`, `synthesis/` | 是 | orchestrator / mandatory synthesis |
| **C Pipeline** | `prompts/pipeline/*.system*.md` | **否** | worker / postprocess 单次 LLM |

判断：是否进入 ReAct ClusterIndex → 否则为 C 族。

`atomic-tools/` 不属于 CDS prompt disclosure root，也不进入 `PromptRegistry`。LLM-facing native tool schema 由 mode/runtime 配置显式拥有；RAG 检索按 ADR-0007 只通过 `codegen` SDK 调用与服务端 fallback。

## 2. Cluster SKILL 结构

```yaml
---
name: <cluster-id>
description: "<≤120 字符，与同 phase 其他 cluster 互斥>"
disclose_at: retrieve | synthesis
atomic: true | false
applicable_modes: [rag, search, chat]  # 可选
---
```

正文四段：何时加载 / 核心指令 / reference 路由表 / 禁止。

## 3. 基准簇

### codegen（retrieve, atomic, rag-only）

- Round 0 强制 Load + 全部 `reference/*.md`
- SDK 最小入口在 SKILL 正文；策略与 doc 概览在 reference

### writing（synthesis, non-atomic, 三 mode）

- Index 仅 1 条 cluster description
- 默认中性 prose；最多加载 1 个 `reference/<slug>.md`
- 选择：`writing_ref` metadata 或 `writing_hint`

## 4. Pipeline（C 族）

| 文件 | caller |
|------|--------|
| `section-index.system.v1.md` | worker ingestion |
| `summary-generation.system.v1.md` | llm SummaryGenerator |
| `triplet-extraction.system.md` | worker graph |
| `session-summary.system.md` | chat postprocess |
| `user-profile-extraction.system.md` | dream layer |

无 frontmatter routing；输出契约写在正文。

## 5. 验收

- RAG 检索 Index ≤4 条；Synthesis cluster Index ≤2 条（writing/format）
- codegen Round 0 无旧 leaf id
- pipeline 文件不在 PromptRegistry
- section-index：无 heading 文档 ingestion 后 document_toc 非空
