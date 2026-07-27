# RAG Quality Golden Set + System Prompt 设计 — 讨论状态总结

**更新时间**：2026-06-29
**背景**：为 Context OS 的 RAG agent 设计真实语料库 + Golden Set，并重新设计 RAG mode 的 system prompt 和 codegen skill。

---

## 1. 已完成的工作

### 1.1 Golden Set 与语料库（全部完成、已验证）

**文件**：`avrag-rs/tests/rag_quality/golden_set_realistic.json`
**配套设计文档**：`avrag-rs/tests/rag_quality/GOLDEN_SET_REALISTIC_DESIGN.md`

**7 份真实私有语料**（LLM 训练集未收录，无 parametric 污染）：

| 文件 | 格式 | 原始路径 | 大小 | 说明 |
|---|---|---|---|---|
| `thesis_y_refrigeration.txt` | DOCX→TXT | `E:\OneDrive\邢川\MBA\论文\答辩\41911407-邢川-林海芬格式2.docx` | 52K chars | MBA 论文 |
| `adr-0004-rag-agent-loop.md` | MD | 项目 ADR | 4.8KB | 技术 ADR |
| `adr-0009-codegen-sandbox-bridge.md` | MD | 项目 ADR | 13.6KB | 技术 ADR |
| `consulting_platform_network_effects.txt` | DOCX→TXT | `E:\OneDrive\邢川\咨询\智遥咨询\智遥咨询文章.docx` | 18K chars | 咨询文章 |
| `consulting_compensation_design.txt` | DOCX→TXT | `E:\OneDrive\邢川\咨询\智遥咨询\薪酬解构.docx` | 3K chars | 薪酬管理 |
| `huawei_ipd_370_activities.txt` | XLSX→TSV | `E:\OneDrive\邢川\咨询\智遥咨询\H为-IPD流程各阶段370个活动详解(3)(1).xlsx` | 54K chars | 华为 IPD 表格 |
| `baiyao_it_planning.txt` | PDF→TXT | `E:\OneDrive\邢川\咨询\佰世方略\【呈云南白药】数字化集成解决方案开发流程，重构中药材板块IT规划0411.pdf` | 20K chars | IT 规划 |

> **DOCX/XLSX/PDF 转 TXT 的原因**：office parser 服务（端口 9090）和 Paddle OCR 远程服务在 WSL 环境下无法使用。脚本：`scripts/office-parser-up.sh` 可启动 office parser。

**10 个子集、107 题**（验证通过）：

| 子集 | 题数 | 验证状态 |
|---|---|---|
| thesis_factual | 15 | ✅ |
| thesis_synthesis | 10 | ✅ |
| thesis_numeric | 12 | ✅ |
| thesis_adversarial | 8 | ✅ |
| adr_factual | 12 | ✅ |
| cross_adr | 5 | ✅ |
| consulting_factual | 14 | ✅ |
| ipd_table | 12 | ✅ |
| baiyao_pdf | 11 | ✅ |
| cross_document | 8 | ✅ |

所有 107 题 source_chunks 子串已验证存在于语料，adversarial 题已验证语料中无答案。

### 1.2 RAG Quality Production 测试（已完成、发现问题）

**文件**：`avrag-rs/crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs`
**新增测试函数**：`realistic_corpus_full_eval`

**测试结果**（首次校准运行）：
- Recall@15 = **10.44%**
- Citation Accuracy = **15.89%**
- 89/107 题返回 124 字节英文降级模板 `"I could not find relevant evidence..."`
- 8 题对抗题 100% 正确拒答
- 22 题 `parse: missing field answer`

### 1.3 根因诊断（已完成）

**根因链**：
1. system prompt 中"判断是否需要检索文档证据"给 LLM 不检索的许可
2. LLM planner 收到中文查询时，多数选择不调检索（直接参数知识回答或拒答）
3. auto_fallback 的 dense_retrieval 也返回 0 chunk（Embedding 召回问题）
4. `DegradedNoEvidence` 路径触发，返回 124 字节英文降级模板（位于 `crates/app-chat/src/agents/loop/policy/exit_policy.rs:460` 的 `degraded_no_evidence_answer("rag")`）

### 1.4 系统架构理解（已完成）

通过源码调研，已彻底理解：

- **ReAct loop 流程**：`run() → normalize_query() → run_retrieval_loop() (多轮) → resolve_synthesis_gate() → synthesis`
- **Round 0 LLM 上下文**：[System] rag-system.md → `<retrieve_cluster_index>` (codegen + memory 目录) → codegen SKILL.md 正文（mandatory）→ `Retrieval query: xxx` → [User] 原始问题
- **skill_request 机制**：LLM 输出 `{"skill_request":["cluster_id"]}` → `IterationControl::Continue` → 下一轮 `plan_retrieve` 触发 cluster body 注入
- **docscope_metadata** 已加载到 `AgentRequest`，但 ReAct loop **从不读取使用它**（只有废弃的 `prompts/plan.rs` 用过）
- **三种检索范式**：dense_retrieval（语义）/ lexical_retrieval（BM25 精确）/ graph_retrieval（实体关系），均自动注入 doc_scope
- **doc_profile / doc_summary** 在 bridge 层（`crates/rag-core/src/runtime/bridge.rs`）硬性要求 `doc_ids` 非空，不会自动注入 doc_scope
- **chunk_fetch** 需要 chunk_id（来自前序检索结果）

### 1.5 system prompt 和 codegen skill 第一版重写（已写但需重构）

**文件**：
- `avrag-rs/prompts/orchestrators/rag-system.md`（v3.0，已写）
- `avrag-rs/prompts/clusters/codegen/SKILL.md`（已修改）

**第一版的问题**（用户指出）：
1. ❌ 不应将 `dense_search` 作为万能开局——用户指出 dense 固有缺陷，对 BM25/graph 无直接帮助
2. ❌ metadata/index/summary 应按需调取，不是固定注入 round 0——但一旦加载就是全量
3. ❌ 完全遗漏了任务规划（分析任务 → 决定加载 skill → 输出检索代码）
4. ❌ 用户指出我对三种检索范式的设计哲学理解不够深入：
   - dense（语义）/ BM25（精确术语）/ graph（实体关系）对应不同检索需求
   - 一个 query 通常被规划为多个子查询
   - metadata/index/summary 补足全局/元数据/结构理解，这些无法从内容检索中自动获得

---

## 2. 未完成的任务（按优先级）

### 2.1 【高】重构 system prompt（`prompts/orchestrators/rag-system.md`）

**最终共识**（与用户多次讨论确认）：
- ✅ 保持 codegen 全量披露（mandatory + atomic），不改成渐进式
- ✅ skill 内容保持精简，不需要提示 LLM 已知的知识
- ✅ LLM 需要知道的是 **RAG tool 的特殊设计**（桥接/沙箱/SKD 签名）而非通用 RAG 概念
- ✅ system prompt 只描述**框架**：目标、约束、执行流程（任务规划框架）
- ✅ 不在 system prompt 里描述三种检索范式选择的具体策略（交给 codegen skill）
- ✅ metadata/index/summary 走按需 `skill_request` 机制

**当前状态**：v3.0 仍在仓库中但内容不正确（仍把 dense_search 当万能开局），需要完全重写。

**重写原则**（用户最后一次明确）：
> "skill保持精简，不需要提示LLM已经知道的知识，LLM需要知道的是RAG tool的特殊设计，能根据你的理解，列举一下，都有哪些内容符合RAG tool特殊设计而非LLM内化知识？"

**TODO**：
1. 列出所有"LLM 内化知识 vs RAG tool 特殊设计"的清单
2. 基于清单重写 system prompt

### 2.2 【高】重构 codegen SKILL.md（`prompts/clusters/codegen/SKILL.md`）

**共识**：
- 保持全量披露（mandatory + atomic）
- 不拆 reference 文件（fewshot/gotcha 放正文）
- 内容精简，聚焦"RAG tool 特殊设计"

**TODO**：
1. 列举 codegen skill 中需要保留/新增的"RAG tool 特殊设计"内容
2. 重写 SKILL.md

### 2.3 【高】新增 metadata cluster（按需加载 docscope_metadata）

**共识**：
- 选项 A：新增 `prompts/clusters/metadata/SKILL.md`
- 在 `modes/rag.yaml` 的 `skill_catalog.retrieve` 加 `metadata`
- LLM 通过 `{"skill_request": ["metadata"]}` 触发
- 在 `disclosure_plan.rs` 的 `render` 中，渲染 metadata cluster body 时，序列化 `request.docscope_metadata` 注入
- 一旦加载就是全量（所有 doc_scope 内文档的元数据）

**TODO**：
1. 创建 `prompts/clusters/metadata/SKILL.md`（描述何时请求、加载后看到什么、拿到 doc_ids 后能做什么）
2. 修改 `modes/rag.yaml`
3. 修改 `crates/app-chat/src/agents/loop/policy/disclosure_plan.rs` 的 `render_cluster_body` 函数，在渲染 metadata cluster 时注入 docscope_metadata
4. 验证：编译 + smoke test + 完整 RAG quality test

### 2.4 【中】重跑 RAG quality test 验证 prompt 改动

**当前**：`realistic_corpus_full_eval` 测试已运行过一次（Recall 10.44%），soft floor 设为 0%（纯校准）

**TODO**：system prompt + codegen skill + metadata cluster 全部改完后，重跑测试验证 Recall 提升

---

## 3. 关键技术约束（不要重复摸索）

### 3.1 Round 0 上下文组装逻辑

```
LLM 在 round 0 看到的完整上下文：

[System message]
  1. rag-system.md 全文（base）
  2. <retrieve_cluster_index> 列出当前阶段所有 cluster 的目录（id + description）
  3. mandatory clusters 的 SKILL.md 正文（当前只有 codegen）
  4. Retrieval query: 用户query
     User display query: 用户query

[User message]
  用户的原始问题
```

**关键代码位置**：
- `crates/app-chat/src/agents/loop/assembler.rs` — `assemble_retrieve()` 组装上下文
- `crates/app-chat/src/agents/loop/policy/disclosure_plan.rs` — `plan_retrieve()` 决定披露哪些 cluster， `render()` 渲染
- `crates/app-chat/src/agents/loop/policy/disclosure_plan.rs` — `render_cluster_body()` 渲染 cluster 正文（可注入额外上下文）

### 3.2 skill_request 协议

**LLM 输出**：
```json
{"skill_request": ["memory", "metadata"]}
```

**机制**：
- `parse_skill_request` 解析 → `validate_skill_request` 校验 cluster_id 合法性
- 存入 `state.disclosed.last_skill_request`
- 下一轮 `plan_retrieve` 读取 `last_skill_request`，对每个请求的 cluster 调 `push_cluster_body`
- `push_cluster_body` 创建 `DisclosureSlice::ClusterBody { cluster_id, reference: None }`
- `render` 渲染正文

**当前不支持**：`{"skill_request": ["codegen:fewshot"]}`（reference_slug 语法）—— LLM 不能请求单个 reference 文件

### 3.3 codegen skill 当前问题（与用户讨论中明确）

| 问题 | 用户反馈 |
|---|---|
| "档案 vs 正文分流" 表格说用 doc_profile 查作者/目录 | ❌ 死路——doc_profile 需要 doc_ids，LLM 开局没有 |
| 教 LLM 调 dense_search 查"目录 摘要 作者" | ❌ 违背 dense 固有语义定位（语义检索不擅长结构化元数据） |
| 没有任务规划框架 | ❌ LLM 不知道怎么分解 query |
| 没有讲三种范式协作 | ❌ 用户说"通常一个 query 被规划为多个子查询" |

### 3.4 三种检索范式 + 元数据/结构工具的设计意图

| 工具 | 定位 | 特殊设计 |
|---|---|---|
| `dense_search` | 语义相似，概念性问题 | 不需要 doc_ids（服务端注入 doc_scope）；方法签名 `dense_search(query, top_k=10, method="auto")` |
| `lexical_search` | 精确关键词，术语/编号/表格 | BM25 检索；`lexical_search(query, top_k=10)` |
| `graph_search` | 实体关系，关联分析 | 知识图谱；`graph_search(query, depth=2)` |
| `doc_profile` | 文档结构化元数据（作者、目录、TOC） | **需要 doc_ids**——只能从 metadata skill 或前序检索结果获取 |
| `doc_summary` | 文档/章节摘要 | **需要 doc_ids**；`doc_summary(doc_ids, level="doc"\|"section")` |
| `chunk_fetch` | 读取特定 chunk 完整正文 | 需要 chunk_id（来自前序检索结果的 chunk_id 字段） |

**用户的设计意图**：
- 三种检索范式针对不同内容需求
- 一个 query 通常规划为多个子查询
- metadata/index/summary 补足"全局理解/元数据理解/结构理解"——这些无法从内容检索中获得
- dense 有固有缺陷，不应做万能开局

### 3.5 沙箱约束（LLM 必须知道）

**文件**：`crates/code-interpreter/src/bridge.rs:26-52`

**Blocked modules**：`os`, `subprocess`, `socket`, `sys`, `ctypes`, `shutil`, `posix`, `fcntl`, `pty`, `pwd`, `grp`, `resource`, `signal`, `multiprocessing`, `threading`

**含义**：LLM 不能 import 任何这些模块，不能联网，不能读写本地文件，不能多进程。

**只能用的库**：标准库 + 注入的 `client` 对象

**通信机制**：fd3/fd4 行式 JSON RPC（用户看不到，但 LLM 知道传参收结果就行）

### 3.6 observation 格式（LLM 收到的检索结果）

`<code_execution_result>[{"chunk_id": "abc-123", "content": "...", "doc_id": "doc-001", "score": 0.92, ...}]</code_execution_result>`

- dense_search/lexical_search/graph_search 返回 `chunks` 数组
- doc_profile/doc_summary 返回结构化 JSON 数组（含 metadata、sections 等）
- chunk_fetch 返回单个 chunk 对象

---

## 4. 文件修改清单（待执行）

| 文件 | 操作 | 说明 |
|---|---|---|
| `prompts/orchestrators/rag-system.md` | 重写 | v3.0 内容不正确，需完全重写为"框架式"system prompt |
| `prompts/clusters/codegen/SKILL.md` | 重写 | 精简，只保留 RAG tool 特殊设计 |
| `prompts/clusters/metadata/SKILL.md` | 新建 | metadata cluster 描述 + doc_ids 获取说明 |
| `modes/rag.yaml` | 修改 | `skill_catalog.retrieve` 加 `metadata` |
| `crates/app-chat/src/agents/loop/policy/disclosure_plan.rs` | 修改 | `render_cluster_body` 中，渲染 metadata cluster 时注入 docscope_metadata |
| `avrag-rs/crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs` | 不动 | 测试代码已就绪 |

---

## 5. 关键代码位置速查

| 需求 | 文件:行 |
|---|---|
| 降级模板（124 字节英文） | `crates/app-chat/src/agents/loop/policy/exit_policy.rs:460` |
| doc_profile bridge 要求 doc_ids | `crates/rag-core/src/runtime/bridge.rs:170` |
| doc_summary bridge 要求 doc_ids | `crates/rag-core/src/runtime/bridge.rs:155` |
| Python SDK shim 源码 | `crates/code-interpreter/src/bridge.rs:17-52` |
| 沙箱 blocked modules | `crates/code-interpreter/src/bridge.rs:31-46` |
| ReAct loop run 入口 | `crates/app-chat/src/agents/loop/mod.rs` |
| assemble_retrieve | `crates/app-chat/src/agents/loop/assembler.rs:30-75` |
| plan_retrieve | `crates/app-chat/src/agents/loop/policy/disclosure_plan.rs:38-65` |
| render_cluster_body | `crates/app-chat/src/agents/loop/policy/disclosure_plan.rs:84-114` |
| skill_request 解析 | `crates/app-chat/src/agents/loop/skill_request.rs:7-32` |
| dispatch_content (skill_request 处理) | `crates/app-chat/src/agents/loop/iteration/content_dispatch.rs` |
| docscope_metadata 结构 | `crates/common/src/docscope.rs:242` |
| build_docscope_metadata | `crates/app-documents/src/helpers.rs:116` |
| load_docscope_metadata | `crates/app-chat/src/agent_runtime.rs` |
| Golden set | `avrag-rs/tests/rag_quality/golden_set_realistic.json` |
| 设计文档 | `avrag-rs/tests/rag_quality/GOLDEN_SET_REALISTIC_DESIGN.md` |
| RAG quality test | `avrag-rs/crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs` |

---

## 6. 下一步（用户新窗口可直接继续）

1. **新窗口第一件事**：读这份 `docs/RAG_PROMPT_DESIGN_STATUS.md`
2. **列出"RAG tool 特殊设计 vs LLM 内化知识"清单**——用户原话："能根据你的理解，列举一下，都有哪些内容符合RAG tool特殊设计而非LLM内化知识？"
3. **基于清单重写三个 prompt 文件**：rag-system.md、codegen/SKILL.md、metadata/SKILL.md
4. **修改 disclosure_plan.rs 的 render_cluster_body** 让 metadata cluster 加载时注入 docscope_metadata
5. **修改 modes/rag.yaml** 加 metadata cluster
6. **编译 + smoke test 验证**：`cargo check -p app --test product_e2e --features product-e2e` + `cargo test -p app --test product_e2e rag_smoke --features product-e2e`
7. **重跑 RAG quality test**：`bash scripts/office-parser-up.sh`（可选，DOCX/XLSX/PDF 原始格式）→ `E2E_MODE=nightly cargo test -p app --test product_e2e realistic_corpus --features product-e2e -- --ignored --test-threads=1 --nocapture`

---

## 7. 已废弃的旧设计（不要回头参考）

- ❌ system prompt 中"判断是否需要检索文档证据"——已删除
- ❌ codegen SKILL.md 中"档案 vs 正文分流"建议用 doc_profile——已删除（死路）
- ❌ "dense_search 万能开局"思路——已废弃
- ❌ 固定注入 docscope_metadata 到 round 0——已废弃（改为按需 skill_request）
- ❌ 渐进式披露 codegen——已决定保持全量披露
- ❌ 旧 planner 路径（`prompts/plan.rs` 中的 `parse_rag_plan_decision`/`build_rag_plan_user_prompt`）——已废弃，ReAct loop 完全取代