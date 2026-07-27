# 词法图增强与得分落差截断（Canonical）

**状态**: Canonical（图增强触发与图证据打分的**唯一现行规范**）  
**日期**: 2026-07-23  
**取代 / 收窄**（见 §8 冲突消解）:

| 旧表述 | 文档 | 处理 |
|--------|------|------|
| dense **与** lexical 均 `graph_augment`；query embed 种子；hop=2 | `2026-07-04-vector-graph-rag-upgrade.md` §2 | **废止 dense 挂钩**；改为 **仅 lexical/BM25 强制带图**；hop=1 |
| 固定分 0.85 作图证据相关度 | milvus/pgvector `GRAPH_CHUNK_SCORE`、upgrade §2.4 | **降为可选 telemetry 标签**，不作相关度 |
| 四通道并行 + 统一候选池混排 | `2026-04-26-current-product-rag-architecture.md` §9 流程 1–4 | **收窄**：正文通道可 RRF/rerank；**图不进正文 rerank 池** |
| full116 中 dense 侧车 graph_augment | `2026-07-05-full116-observable-diag.md` | 行为描述保留为**历史观测**；现行策略以本文为准 |

**实现现状（2026-07-23）**: P0–P3 已接线 — augment 在 **`lexical_retrieval` 内** 一次执行（bridge + native 共用）；bridge 透传 `graph_context` 并写 telemetry side-car。**产品默认 on**；显式 `0` 可关。eval：`graph_augment` vs explicit。observation 空 stdout 时补 `chunks`+`graph_context`。SKILL 已说明返回 dict 习惯。

---

## 0. 一句话产品目标

**关键词检索（BM25/lexical）时，系统自动在关系网里走 1 步，捞出结构邻居；证据段落只保留与本跳关键词 rerank 后紧贴 TOP1 的那些。**  
语义 dense **不**自动带图。多跳靠 Agent ReAct 多轮规划，不靠一次深 BFS。

---

## 1. 设计动机（产品语言）

### 1.1 为什么不用「原用户问题」给图证据打相似度

图桥接的价值是：**结构相连**，不是「整句问题语义最像」。

- 用**原 query 的 embedding** 给图扩出来的 chunk 打分 → 结构上对的桥往往分偏低 → 缺陷。  
- Seed / path / evidence 若**全部**绑在原 query 上 → 同一缺陷。

### 1.2 为什么绑 BM25 / 关键词，不绑 dense

| 点 | 说明 |
|----|------|
| 关键词 ≈ 图接口 | 实体名、缩写（DRC/DRO）、专名本就是 lexical + 边字符串擅长的 |
| 补关键词过窄 | 1 跳邻接补上字面相连、漏检的邻居 |
| 多跳交给 loop | LLM 按任务输出本跳 terms → ReAct 再规划下一跳；图 **1 hop 够用** |
| dense 职责 | 语义改写、概念相近；**不**负责自动扩图（避免语义假相关灌噪声） |

### 1.3 与 Hybrid 论文（arXiv:2507.03608）的关系

- **保留**：向量/正文与图**分层**（正文在前、图在后或独立键）；控冗余。  
- **不照搬**：论文未规定边公式；也未要求 dense 必须带图。  
- **我们的选择**：图增强挂在 **lexical 任务**上，用 **关键词尺度** 选证据，比挂在整句 dense 上更贴图本质。

---

## 2. 触发规则（强制带图）

### 2.1 何时强制

| 入口 | 是否强制 1 跳图增强 |
|------|---------------------|
| `lexical_search` / `lexical_retrieval` / BM25 工具 | **是**（`RETRIEVAL_GRAPH_AUGMENT=1` 时；**产品默认建议 on**） |
| `dense_search` / `dense_retrieval` | **否** |
| 显式 `graph_search` / `graph_retrieval` | 按工具参数执行（可 hop>1，见 §2.3） |

环境变量：

| 变量 | 建议默认 | 含义 |
|------|----------|------|
| `RETRIEVAL_GRAPH_AUGMENT` | **`1`（产品）** / `0`（纯 A/B 关） | 总开关 |
| `GRAPH_AUGMENT_MAX_RELATIONS` | `5` | 本跳最多关系条数 |
| `GRAPH_AUGMENT_SEED_LIMIT` | `8` | 种子实体上限 |
| `GRAPH_AUGMENT_HOPS` | **`1`** | 强制增强通道固定 1 跳（显式工具可另设） |
| `GRAPH_EVIDENCE_MARGIN_ABS` | `0.08`（初值，可调） | 与 TOP1 绝对落差上限 |
| `GRAPH_EVIDENCE_MARGIN_REL` | `0.90` | 相对 TOP1 比例下限（`score ≥ α·s1`） |
| `GRAPH_EVIDENCE_MAX_K` | `3` | 支撑 chunk 硬上限（含 TOP1） |

### 2.2 种子（terms-first）

本跳检索任务的关键词/实体词集合 \(T\)（来自 LLM 编排的 terms，**不是**用户原句整句 embed）：

1. **精确/规范化匹配**：`subject/object/name/normalized_name` 与 \(T\) 对齐（大小写、简繁按现有 normalize）。  
2. **可选轻量对齐**（仅当精确种子为空或过少）：对 **terms 拼接串** 做 entity ANN，阈值可配；**禁止**对用户原 query 整句做 entity ANN 作为强制增强默认。  
3. 截断至 `GRAPH_AUGMENT_SEED_LIMIT`。

### 2.3 遍历

- 强制增强：**恰好 1 hop**（subject/object 命中种子 → 取邻接关系）。  
- 多跳：依赖 ReAct 下一轮新的 lexical + 再 1 hop。  
- 显式 `graph_search`：允许 `hop_limit>1`（产品深度关系题），与强制增强分离。

### 2.4 输出形状（分层，不混进 dense chunks）

```json
{
  "chunks": [ /* lexical 主结果，可含其自身 top 文段 */ ],
  "graph_context": [
    {
      "relation_id": "...",
      "subject": "...",
      "predicate": "...",
      "object": "...",
      "relation_text": "...",
      "hop": 1,
      "seed_terms_hit": ["DRC", "DRO"],
      "evidence_chunks": [
        {
          "chunk_id": "...",
          "score": 0.91,
          "score_gap_to_top1": 0.0,
          "kept_reason": "top1"
        }
      ]
    }
  ]
}
```

- `chunks`：词法主通道结果（**可**参与原有 lexical/正文 rerank 策略）。  
- `graph_context`：**不**并入 `chunks` 数组，**不**参与 dense 正文 cross-encoder 混排。  
- 固定 `0.85` **不再**作为图证据相关度；若保留字段仅允许 `score_type: "channel_proxy"` 且下游禁止用于排序。

---

## 3. 算法：词法图增强 + TOP1 得分落差截断

### 3.1 符号

| 符号 | 含义 |
|------|------|
| \(T\) | 本跳关键词/实体词集合 |
| \(S\) | 种子实体集合 |
| \(R\) | 1 跳关系集合（截断后） |
| \(C_r\) | 关系 \(r\) 的 supporting_chunk 候选 |
| \(s(c;T)\) | chunk \(c\) 相对 \(T\) 的证据分（越高越好） |
| \(c_{(1)}\) | \(C_r\) 中 \(s\) 最大者（TOP1） |

### 3.2 伪代码

```text
function LexicalGraphAugment(terms T, doc_scope D, auth):
  if RETRIEVAL_GRAPH_AUGMENT is off:
    return empty graph_context

  // --- 与 lexical 主检索并发 ---
  lexical_chunks = lexical_retrieve(T, D)   // 既有 BM25/FTS

  S = seed_entities_from_terms(T, D)        // §2.2
  if S is empty:
    return { chunks: lexical_chunks, graph_context: [] }

  R = one_hop_relations(S, D, limit=GRAPH_AUGMENT_MAX_RELATIONS)

  graph_context = []
  for r in R:
    C = load_supporting_chunks(r) ∪ maybe_neighbor_passages(r)
    // 去掉已在 lexical_chunks 中的 chunk_id（可选去重）
    C = dedupe_against(C, lexical_chunks)

    if C is empty:
      // 仍可保留 relation 短句本身作为结构提示（无 evidence_chunks）
      graph_context.append(relation_only(r))
      continue

    // 证据分：相对本跳 terms，不是原 user query
    for c in C:
      s(c) = evidence_score(c, T)   // §3.3

    sort C by s descending
    c1 = C[0]
    kept = [c1]
    for c in C[1:]:
      if |kept| >= GRAPH_EVIDENCE_MAX_K:
        break
      if score_gap_ok(s(c), s(c1)):   // §3.4
        kept.append(c)
      else:
        break   // 已按分排序，后续只会更差

    graph_context.append({ relation: r, evidence_chunks: kept })

  return {
    chunks: lexical_chunks,
    graph_context: graph_context
  }
```

### 3.3 证据分 \(s(c;T)\)（相对本跳关键词）

**优先顺序**（实现选一主、一备）：

1. **主**：对「terms 拼接 query」或 term 列表的 **cross-encoder / 现有 reranker** 分。  
2. **备**：BM25/覆盖率（命中 term 数 / |T|，或 lexical score 归一化到 \([0,1]\)）。

约束：

- **禁止**用「用户原句 embedding ↔ chunk」作为图证据主分（回到 §1.1 缺陷）。  
- 同一关系内所有候选必须用**同一** \(s\)，保证落差可比。

### 3.4 得分落差截断（相对 TOP1）

TOP1 **必留**。候选 \(c\)（已按 \(s\) 降序）保留当且仅当：

\[
\begin{aligned}
s(c) &\ge s(c_{(1)}) - \delta_{\mathrm{abs}}
\\
\text{且}\quad
s(c) &\ge \alpha_{\mathrm{rel}}\cdot s(c_{(1)})
\\
\text{且}\quad
|{\mathrm{kept}}| &< K_{\max}
\end{aligned}
\]

| 参数 | 符号 | 建议初值 |
|------|------|----------|
| 绝对落差 | \(\delta_{\mathrm{abs}}\) | `GRAPH_EVIDENCE_MARGIN_ABS` = 0.08 |
| 相对落差 | \(\alpha_{\mathrm{rel}}\) | `GRAPH_EVIDENCE_MARGIN_REL` = 0.90 |
| 硬上限 | \(K_{\max}\) | `GRAPH_EVIDENCE_MAX_K` = 3（含 TOP1） |

**落差定义（对外字段）**：

\[
\mathrm{score\_gap\_to\_top1}(c) = s(c_{(1)}) - s(c)
\]

- `kept_reason`: `top1` | `within_margin` | `dropped_gap` | `dropped_cap`  

**注意**：这不是「方差最小」，而是 **与 TOP1 的得分落差 band**；同分高原时靠 \(K_{\max}\) 封顶。

### 3.5 关系条数截断

在 1 跳关系集合上，优先保留：

1. 两端实体命中 \(T\) 更多的边  
2. 有 supporting_chunk 且 TOP1 \(s\) 更高的边  

至多 `GRAPH_AUGMENT_MAX_RELATIONS` 条。

---

## 4. 与 ReAct / 多跳的分工

```text
用户问题
  → Agent 规划本跳 terms（可能多组）
  → lexical(+强制 1 跳图) → observation（chunks + graph_context）
  → 若需第二跳：新 terms，再 lexical(+图 1 跳)
  → 合成
```

| 能力 | 负责方 |
|------|--------|
| 语义改写、概念召回 | dense（**不**强制图） |
| 专名/编号/实体词 + 结构邻接 | lexical + 强制图 1 跳 |
| 2+ 跳推理 | Agent 多轮，非单次 BFS |

显式 `graph_search`：用户点名两端实体、要更深关系时仍可用；与强制增强 **互补**，不互相替代。

---

## 5. 实现落点（恢复时）

| 位置 | 行为 |
|------|------|
| `RuntimeBridge::call` | **仅** `lexical_search`（及等价 method）在开关 on 时 `join!(lexical, graph_augment_from_terms)` |
| `dense_search` | **不**调用 graph_augment |
| `graph_augment` | 输入 **terms**（从 lexical args / codegen 参数抽取），hop=1，输出 `graph_context` + evidence 落差字段 |
| `storage-*:search_graph` | 支撑 1 跳 query；固定 0.85 **勿再用于** evidence 排序 |
| Prompt codegen SKILL | 说明：lexical 结果可能含 `graph_context`；主体仍优先 `chunks`；多跳靠多轮 terms |
| Eval | `graph_augment_hit` vs `graph_explicit_called` 分离；强制增强 on 时勿把 augment 记成「误调 graph」 |

---

## 6. 验收标准

| # | 标准 |
|---|------|
| A1 | `RETRIEVAL_GRAPH_AUGMENT=1` 时，单次 `lexical_search` 的 observation **可**含非空 `graph_context`（语料有边时） |
| A2 | 同开关下，`dense_search` 的 observation **不得**仅因 dense 而出现 augment 侧车（无显式 graph_search） |
| A3 | `graph_context[].evidence_chunks`：必含 TOP1；任一条 `score_gap_to_top1 ≤ δ_abs` 且满足相对阈值；条数 ≤ \(K_{\max}\) |
| A4 | 图证据 chunk **不**出现在 dense 主 rerank 输入列表 |
| A5 | 单测：构造 s=[1.0, 0.95, 0.70]，δ=0.08, α=0.9 → 仅保留前 2 条 |

---

## 7. 非目标

- 用原 user query cosine 给图支撑 chunk 主排序  
- dense 强制带图  
- 强制增强 hop≥2  
- 用常量 0.85 表示图相关度  
- 图边与 dense 分混进同一 cross-encoder 池

---

## 8. 文档冲突消解（统一表述）

### 8.1 现行真理来源

**本文为图增强触发与图证据截断的唯一 canonical。**  
与本文冲突的旧句一律以本文为准。

### 8.2 各文档应如何读（**勘误已写入**，2026-07-23）

| 文档 | 调整原则 | 状态 |
|------|----------|------|
| `2026-07-04-vector-graph-rag-upgrade.md` §2 | 历史设计；**dense 挂钩废止**；文首 + §2.1/2.2 勘误 | ✅ |
| `2026-07-04-graph-channel-analysis.md` §7 | 指向本文；§7.0 现行 / §7.1 历史 | ✅ |
| `2026-04-26-current-product-rag-architecture.md` §9 | 图增强绑 lexical；`graph_context` 不混排 | ✅ |
| `2026-07-05-full116-observable-diag.md` | 历史观测 + 文首/§10.2 现行指针 | ✅ |
| `2026-07-23-pgvector-vector-graph-rag-design.md` | 存储双栈仍有效；0.85 仅 telemetry；语义对齐本文 | ✅ |
| `docs/README.md` | 摘要增加图增强 canonical 链 | ✅ |
| 代码 `GRAPH_CHUNK_SCORE=0.85` | 标注 channel_proxy；evidence 用落差分 | ✅ P2 |

### 8.3 术语表（统一用词）

| 用词 | 含义 |
|------|------|
| **词法图增强** | lexical/BM25 强制附带的 1 跳图 |
| **得分落差截断** | 相对 TOP1 的 margin 截断（非「方差」） |
| **本跳 terms** | LLM/工具为本轮检索产出的关键词，非用户原句全文 |
| **graph_context** | 图结果独立载荷，不混入 dense `chunks` |
| **显式 graph_search** | 模型主动调用的图工具，可更深 hop |

---

## 9. 实现分期（建议）

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 恢复 `graph_augment`：**仅 lexical** 钩子；hop=1；terms 种子；`graph_context` 输出 | ✅ |
| P1 | evidence \(s(c;T)\) + TOP1 落差截断 + 单测 A3/A5 | ✅ |
| P2 | 默认 on；eval 拆分 augment vs explicit；0.85 channel_proxy 注释 | ✅ |
| P3 | SKILL.md；observation 含 graph_context；structured log；native lexical 对齐 | ✅ |

### 9.1 参数 A/B（P3）

| 旋钮 | 建议扫描 | 观察指标 |
|------|----------|----------|
| `RETRIEVAL_GRAPH_AUGMENT` | 0 vs 1 | `graph_augment_hit`、答案 must_include、耗时 |
| `GRAPH_EVIDENCE_MARGIN_ABS` | 0.05 / 0.08 / 0.12 | evidence 条数、噪声 cite |
| `GRAPH_EVIDENCE_MARGIN_REL` | 0.85 / 0.90 / 0.95 | 同上 |
| `GRAPH_EVIDENCE_MAX_K` | 2 / 3 / 5 | token 膨胀、冗余 |
| `GRAPH_AUGMENT_MAX_RELATIONS` | 3 / 5 / 8 | 关系覆盖 vs 噪声 |

日志字段（`graph_augment completed`）：`seed_count`、`graph_context_len`、`evidence_kept`、`max_score_gap_to_top1`、`margin_*`、`elapsed_ms`。

---

## 10. 摘要

```text
不要：dense 强制带图 + 原 query 语义给图打分 + 固定 0.85 当相关度 + 深 BFS 一次多跳
要做：BM25/lexical 强制带图 + 本跳关键词种子 + 仅 1 跳
      + 支撑 chunk 对关键词 rerank + 相对 TOP1 得分落差截断 + 小 K_max
      + 多跳交给 ReAct
```
