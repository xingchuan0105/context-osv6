# avrag149 subset → skill 映射表(2026-08-01)

> 目的:回答「skill 多组 ↔ 多测试集」——149 题各 subset 实际在测哪个/哪些 skill。
> 依据:golden 的 `mode`/`capabilities`(决定挂载面)+ 题型内容。
> 挂载面规则(SaC):`agent-base` 恒定;`capabilities/knowledge-base`(rag)、`capabilities/web`(search)按 capability 挂载;cluster skills 按需披露。

## 1. 挂载面(按 capabilities)

| capabilities | 挂载面 | 题数 |
|--------------|--------|------|
| `[]`(chat) | agent-base(+memory/writing 按需) | 11 |
| `['rag']` | agent-base + capabilities/knowledge-base + knowledge-base skill(+表格 reference) | 128 |
| `['search']` | agent-base + capabilities/web + search skill | 3 |
| `['rag','search']` | agent-base + knowledge-base + web(双源) | 7 |

## 2. subset → 主要依赖 skill

| subset | 题数 | mode/caps | 主依赖 skill | 次依赖 | 题型特征 |
|--------|------|-----------|--------------|--------|----------|
| thesis_factual | 15 | rag | **knowledge-base** | — | 单文档事实检索 |
| thesis_synthesis | 10 | rag | **knowledge-base** | writing(synthesis 表述) | 多事实综合 + 归因 |
| thesis_numeric | 12 | rag | **knowledge-base** | — | 数字/统计检索 |
| thesis_adversarial | 8 | rag | **knowledge-base** | — | 对抗性/未记载题(拒答) |
| adr_factual | 12 | rag | **knowledge-base** | — | ADR 元数据/文档事实(含 Date 等英文字段) |
| cross_adr | 5 | rag | **knowledge-base** | — | 多 ADR 对照 |
| consulting_factual | 14 | rag | **knowledge-base** | — | 咨询报告事实 |
| ipd_table | 12 | rag | **knowledge-base + how-to-read-tables** | struct_query(表格) | IPD 表格:行计数/表序/阶段 |
| baiyao_pdf | 11 | rag | **knowledge-base + how-to-read-tables** | struct_query | 白药 PDF 表格/分层 |
| cross_document | 8 | rag | **knowledge-base** | — | 跨文档对照(相似/差异) |
| orchestrator_paradigm | 8 | rag(6)/chat(2) | **knowledge-base + agent-base** | — | 文档总结 + 纯对话 |
| rag_search_joint | 6 | rag+search | **knowledge-base + web** | search skill | 双源联合(知识库+联网) |
| chat_builtin_tools | 4 | chat | **agent-base**(calculator/weather 原生工具) | — | 计算/工具调用 |
| rag_codegen_channels | 7 | rag | **knowledge-base**(codegen 桥接:client.* 检索) | — | 检索方法/通道问题 |
| memory_coreference | 3 | rag(2)/chat(1) | **memory + agent-base** | knowledge-base | 跨轮指代/记忆 |
| search_web | 2 | search | **web + search skill** | — | 联网检索 |
| new_corpus_factual | 6 | rag | **knowledge-base** | — | 新语料事实 |
| option_d_pure_chat_smoke | 1 | chat | **agent-base** | — | 纯对话 |
| option_d_search_only | 1 | search | **web + search skill** | — | 纯联网 |
| option_d_dual_source | 1 | rag+search | **knowledge-base + web** | — | 论文+公开资料 |
| option_d_utility_tools | 3 | chat | **agent-base** | — | 计算工具 |

## 3. 训练管道现状 vs 该映射

| 维度 | 现状 | 映射揭示的缺口 |
|------|------|----------------|
| 优化目标 | 仅 `system/agent-base.md` 单文件 | **knowledge-base skill 是 128 题(86%)的主依赖**,但训练不改它——错题圈选/检索词/表格素养问题(本波修的)都发生在 knowledge-base,优化 agent-base 收效有限 |
| 数据集 | 149 统一划分 | 未按 skill 分组;val 13 题混合能力,gate 对"某个 skill 的改善"无区分度 |
| search 题 | 仅 3 题(5 题含 search) | search skill 几乎无法被训练/评估(样本太少) |
| memory/chat 题 | 11 题 chat | agent-base 优化会覆盖,但 memory skill 不参与 |

## 4. 若按 skill 分组训练(候选方案)

| 训练轮 | 优化目标 | 数据集(subset) | 题数 |
|--------|----------|----------------|------|
| KB 轮(主) | `clusters/knowledge-base/SKILL.md`(+reference) | 文档类 subset(thesis_*/adr_*/consulting/ipd/baiyao/cross_doc/new_corpus/…) | ~100 |
| KB-表格轮 | `reference/how-to-read-tables.md` | ipd_table + baiyao_pdf | 23 |
| Web 轮 | `clusters/search/SKILL.md` + `capabilities/web.md` | search_web + option_d_search_only + dual | ~9(样本少,需扩充) |
| Chat/记忆轮 | `system/agent-base.md` + memory skill | chat/memory/orchestrator | ~15 |

> 注:单轮优化单文件仍是 skillopt 的粒度(prompt_target);多文件并行优化需改 rollout 注入(多目标交换)或逐轮跑。
