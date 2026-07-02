# RAG 优化待办总表（2026-07-01）

> 单一来源汇总所有未决待办。完成一项就在此标 ✅ 并附验证结果；不再散落在对话/loop 日志里。
> 证据索引统一指向 `avrag-rs/prompts/_backups/loop_iterations.md` 的对应节。

## 当前状态（基线）
- smoke_v5 prompt loop 跑到 **Iter 3 full-12**，命中**系统层瓶颈**（检索 + 合成管线），提示词杠杆穷尽。
- 确定性提升：answerable PASS **2/9 → 4/9**；contract 修复（JSON mode + marker 禁用）。
- 剩余 non-PASS：Q1/Q6（检索漏召）、Q9/Q10（partial recall + 管线 fallback）、Q4/Q7（方差）。
- 评估代码已审计冻结（metrics_v2 / harness_extract / metrics.rs 不许再动）。

---

## A. 系统层代码修复（解锁 loop 继续推进的前提）

### A1. Milvus BM25 中文分词器　[P0]　治 Q1/Q6/Q9/Q10 漏召　✅ 代码完成 → 🔄 执行重灌（2026-07-01）
- **现状**：`crates/storage-milvus/src/schema.rs` 的 `text` 字段 `enable_analyzer=true` 但**未设 `analyzer_params`**，默认 `standard` 分词器只按空格/标点切，**中文连续汉字不切词** → L109"因此在2019年于大连市投资建厂"被整块当 1 token，查询词"建厂/大连/投资"0 命中 → L109 块从不返回。
- **修法**：给 BM25 `text` 字段配中文分词器。仓库已有 `jieba-rs`（`common/src/text_segment.rs` 用于 PG 记忆 FTS），但 Milvus BM25 没接——两套分词不一致，要打通。
- **证据**：loop_iterations.md「检索漏召根因实证 — Milvus BM25 未配中文分词器」节；Q1 trace（sse_events 搜 L109 独有短语"两大项目的建设完成"=0 命中）。
- **验收**：Q1/Q6 recall > 0；重跑 smoke_v5 graded_recall@15 上升。
- **边界**：改 storage-milvus schema + 灌库 analyzer，**非评估代码**。
- **执行（2026-07-01）**：`RAG_QUALITY_SMOKE_FORCE_INGEST=1` 时自动 drop Milvus 集合再重灌（`smoke_v5_corpus.rs`）。重灌命令见下。

### A2. 合成兜底 C+A　[P0]　治 Q10 SYNTHESIS_CONTRACT　✅
- **现状**：`synthesis.rs:170` `contract_violation_fallback` 在两轮 repair 都过不了质检、草稿无拒答句时吐英文串"I found relevant material but could not format a validated cited answer..." → partial recall 时 Q10 contract 失败。**代码替模型吐，prompt 改不动**。
- **C（治标）**：两轮候选都过不了质检且模型想答（草稿无拒答句）时，不吐英文 fallback，改从模型已写 `answer_text` 提取可用部分（丢不合法引用）输出中文；无可用正文则输出中文"资料不足以完整回答"。
- **A（治本）**：增加质检 repair 轮数 + JSON mode（deepseek 已开）+ 更宽松的 citation 校验。
- **证据**：loop_iterations.md「后续代码修复计划 A — 合成兜底 C+A」节；Q10 full-12 answer_preview。
- **验收**：Q10 contract 通过；contract=100% 稳定。
- **边界**：改 `synthesis.rs` / `answer_contract.rs`，**非评估代码**。
- **结果（2026-07-01）**：`DEFAULT_SYNTHESIS_REPAIR_ROUNDS=2`；新增 `extract_partial_synthesis_fallback` 剥离无效 `[[cite:]]` 后输出中文部分答案；`contract_violation_fallback` 改中文；拒答安全网（reasoning lift + draft 拒答检测）保留。`cargo test -p app-chat answer_contract synthesis` 14/14 PASS。

---

## B. 工具覆盖验证（新能力接线，独立于 loop）

### B1. index 两步流验证　[P1]　✅ 题面+指标就绪，待 E2E 跑
- **现状**：`codegen/SKILL.md` 已教 `doc_profile`（定位章节 chunk_id）→ `chunk_fetch` 两步 + 明确告知"该 chunk 即用户所求内容"（2026-07-01 接线，备份 `_backups/codegen-SKILL.md.2026-07-01-index-flow`）。
- **待办**：设计"某文档某章"题，跑一遍看 trace 工具序列是否 `doc_profile → chunk_fetch`（而非直接 dense 碰运气）。
- **题面草稿**：「Y冷冻设备公司论文里"4R策略"那一章讲了什么？」
- **验收**：工具调用序列 = `doc_profile → chunk_fetch`。
- **就绪**：✓（content_store 有数据，TOC chunk_id 两路径可靠填充）。

### B2. summary 验证题　[P1]　✅
- **题面**：S1「请用一段话概括 Y冷冻设备公司论文的核心内容和结论」/ S2「云南白药 IT 规划这份文档主要讲了什么？」
- **期望工具**：`doc_summary(level="doc")`
- **就绪**：✓（`INGESTION_LLM` 配了，`summary_generator` 跑，摘要已建）。

### B3. metadata 验证题　[P2]　✅
- **题面**：P1「Y冷冻设备公司论文的作者/年代/领域是什么？」/ P2「云南白药 IT 规划文档的元信息」
- **期望工具**：`doc_profile`（name/author/era/domain/genre/publication_date 字段）
- **就绪**：✓
- **注**：metadata 需求已**合并进 `doc_profile`**（无独立 `doc_metadata` 暴露给 LLM——该工具 dispatch 里有但 codegen skill 没教、bridge 没映射）。

### B4. graph 验证题　[P1]　🔄 执行 triplet 重灌（2026-07-01）
- **题面**：G1「Y冷冻设备公司和它的韩方投资者是什么关系？请梳理这条关系链。」/ G2「云南白药 IT 规划中 4A 架构和数据资产目录（11/100/638）之间是什么关系？」
- **期望工具**：`graph_search`
- **就绪**：✗（smoke_v5 `INGESTION_TRIPLET_ENABLED=0`，图集合 `kg_entities`/`kg_relations`/`graph_passages` 空）。
- **前置**：`RAG_QUALITY_SMOKE_TRIPLET_ENABLED=1` + `RAG_QUALITY_SMOKE_FORCE_INGEST=1` 重灌（`smoke_v5_corpus.rs` 覆盖 `.env` 的 `INGESTION_TRIPLET_ENABLED=0`）。
- **执行命令**：
  ```bash
  E2E_MODE=nightly RAG_QUALITY_SMOKE_FORCE_INGEST=1 RAG_QUALITY_SMOKE_TRIPLET_ENABLED=1 \
    cargo test -p app --test product_e2e rag_tools_golden_set \
    --features product-e2e -- --ignored --nocapture --test-threads=1
  ```

### B5. golden_set_tools.json + 工具覆盖率指标　[P1]　✅
- **结果（2026-07-01）**：`golden_set_tools.json` 8 题 + `tool_coverage.rs`（coverage_rate / sequence_hit_rate / triplet_reingest 标记）+ E2E 入口 `rag_tools_golden_set`。`cargo test -p rag_quality` 41/41 PASS。
- **跑法**：`E2E_MODE=nightly cargo test -p app --test product_e2e rag_tools_golden_set --features product-e2e -- --ignored --test-threads=1 --nocapture`

---

## C. 小修（待决策）

### C1. `doc_summary(level="section")` 名不副实　[P3]　✅ 选项(b)
- **结果（2026-07-01）**：`codegen/SKILL.md` 已澄清 `doc_summary(level="section")` = TOC 章节**标题列表**（非散文摘要）；某章内容仍走 `doc_profile` → `chunk_fetch`。未改 runtime 代码。

---

## D. loop 继续推进（A1/A2 已落地，D 执行中 2026-07-01）
A1（BM25）+ A2（C+A）落地后，重跑 smoke_v5 full-12 两次，验 5 大目标：
1. answerable PASS ≥ 6/9
2. graded_recall@15 ≥ 70%，且 recall@15 ≥ 60%
3. contract=100%、refusal_correct=100%
4. 连续 2 次 graded_recall@15 波动 ≤ ±5pp
5. 语义裁判闸：non-PASS 全查 + 抽 2 道 PASS，确认无 deterministic 层漏掉的编造

未达则继续 Iter 4+（BM25 修好后，codegen 检索策略杠杆可能重新可调）。

---

## 依赖关系
- **A1 → D**：BM25 修好才能继续 loop（Q1/Q6 漏召是检索层）。
- **A2 → D**：合成兜底修好，Q10 contract 才稳。
- **B4 前置**：`INGESTION_TRIPLET_ENABLED=1` 重灌。
- **B1/B2/B3**：可立即跑（就绪）。
- **B5**：B1-B4 题面定稿后落文件 + 加指标。
- **C1**：独立，待决策。

---

## 编排执行记录（2026-07-01）

| 任务 | Subagent | 状态 |
|------|----------|------|
| A1 BM25 中文分词 | [A1 agent](1b53cc57-1b6e-42ac-9936-0c1d8049a2f5) | ✅ 代码完成，需 drop+重灌 |
| A2 合成兜底 C+A | [A2 agent](2012781c-62b4-4371-9094-212f517bc8cd) | ✅ 14/14 单测 PASS |
| B1-B5 + C1 | [B agent](b27025c4-5425-43ee-b3a3-eafa074d66e8) | ✅ golden+指标+SKILL 41/41 |
| D smoke_v5 ×2 | [D agent](357a4890-bcb6-4a40-ad01-00b60f31c730) | 🔄 后台执行中 |

---

## 已完成（参考，不在待办）
- prompt loop Iter 0-3；golden 合理化 Q1/Q4/Q5/Q9 + must_not_include；marker（EVIDENCE_INSUFFICIENT_FALLBACK）禁用 + 泄漏推理修复；DeepSeek JSON mode 接线；评估审计 + 分级相关性（graded_recall/graded_ndcg）+ must_not_include 接线 + 逐题裁判视图 dump；graph/index/summary/metadata 就绪性核实；index 两步流 codegen 接线。
