# 证据平面：检索日志 + 自适应 top-k 设计

> **SUPERSEDED** — 本文描述的 orchestrator / worker / brief / handoff 多 agent 架构已被取代：2026-07-30 起产品路径改为单 agent（SaC 设计，见根 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），orchestrator 代码已于 2026-08-01 物理删除（commit `7f2d182d`）。本文仅作历史记录。（横幅添加于 2026-08-02 文档体系梳理）

| 项目 | 内容 |
|---|---|
| 状态 | **设计待评审** |
| 日期 | 2026-07-28 |
| 关联 | 薄编排范式讨论（orchestrator 只改写+传话）；RISE 交互空间（bounded/persistent/navigable）；QPP 与 adaptive-k 最佳实践（§2） |
| 范围 | subagent 证据产出与 orchestrator 证据消费之间的传递机制；不改评测 |

---

## 0. 一句话

subagent 向 orchestrator 传递证据改为两个支柱：**① 检索日志**——LLM 只登记 chunk 别名，代码负责水合全文；**② 分数形态自适应 top-k**——每次检索按分数梯度动态返回 1–5 条并附决策提示。多轮检索后，orchestrator 收到的是**有界、精准、可审计**的证据集，不需要任何 JSON 契约或指针校验。

## 1. 问题定义（已确认）

- 证据库全量堆积：每轮检索全量入库，模型观察窗只有 8000 字符（实测 ~7% 可见率）→ "模型没见过的证据"污染合成。
- "模型用过哪些"无可靠信号：key_facts 手抄 id 不可靠（E103 天天在拦）。
- 块级堆积无界（已由 loop budget 天然约束，不再另设上限——**用户决策 ②**）。
- 空圈选不做兜底（**用户决策 ③**：空日志 → 空内容交给 orchestrator 自决）。

## 2. 最佳实践核对（梯度算法选型）

| 方法 | 出处 | 要点 | 采用度 |
|---|---|---|---|
| **Largest-Gap（adaptive-k）** | Taguchi et al., [EMNLP 2025](https://aclanthology.org/2025.emnlp-main.1017.pdf)（"No Tuning, No Iteration, Just Adaptive-k"） | 取候选池（~50）按分数排序，找最大相邻分差位置截断；免调参、零额外调用 | **主算法** |
| CAR（距离空间聚类） | [Xu et al. 2025](https://arxiv.org/html/2511.14769v1) | 距离空间聚类找"密集相关簇→稀疏尾"分界 | 备选（更重） |
| NQC / UQC | QPP 族 | top-k 分数标准差；**文献对"高方差=好查询"与"低方差=好查询"两种解释并存**，语义不稳定 | 不采用 |
| MAIN-RAG 动态阈值 | [Chang et al. 2024](https://www.rohan-paul.com/p/challenges-and-techniques-of-filtering-in-vector-databases/) | 按分数分布动态裁剪，+2–11% 准确率 | 佐证方向 |
| QPP 总览 | [Meng et al. 2024](https://arxiv.org/pdf/2404.01012) | 后检索特征（分数形态）优于先检索特征 | 佐证方向 |

结论：用 **largest-gap + 平坦度判定**，免调参、可解释、零额外模型调用。

## 3. 支柱二：分数形态自适应 top-k（1–5）

### 3.1 算法（确定性的，接在 rerank 之后）

```
输入：rerank 后候选序列 s[1..n]（降序，n 由现有 rough 池决定）
窗口 w = min(n, 8)
gaps[i] = s[i] - s[i+1]，i ∈ [1, w-1]
i* = argmax(gaps)
range = s[1] - s[w]
flat_thresh = max(ε_abs, ε_rel × |s[1]|)   # 绝对+相对混合，免疫量纲

判定：
  ① range < flat_thresh    → FLAT（全员同分）：k = 5
  ② gaps[i*] / range ≥ γ   → 显著梯度：k = clamp(i*, 1, 5)   # 按 gap 位置截断，不是硬压
  ③ 否则                   → 区分度不足：k = 5
ε_abs=0.02, ε_rel=0.03, γ=0.4 为校准初值（用评测集校准）
```

- 提示分档：k≤2 报「命中明确（top 分数梯度大）」；k=5 且非 ① 报「区分度不足」；① 报「全员同分，疑似无有效命中」。

- 接点：`crates/rag-core/src/runtime/tools/dense.rs` 的 rerank 之后、`cut_final_candidates` 之前；lexical（pg_bigm similarity）同样适用（无 rerank 时直接用相似度序列）。
- **score 归一**：rerank 分数与 dense 原始分分布不同，梯度一律在**最终用于展示的那套分数**上计算（reranked 优先，lexical 用 similarity）。
- 与原 rough→final 漏斗的关系：本算法取代"固定 final_feed 比例"，成为每个检索调用的出口宽度；rough 池（召回侧）不变。

### 3.2 反馈提示（接 E3 教练式提示，统一体系）

- k≤2（命中明确）：「命中明确（top 分数梯度大）。可进入分析；若需交叉验证可换角度再查一次。」
- k=5（区分度不足）：「结果区分度低（分数平均）。建议：① 换更具体的词（专名/编号/表内字面值）；② 若换词后仍平均，该语料可能未覆盖——按查无流程处理。」

### 3.3 块级与轮级语义（用户决策 ②）

- 按**调用**分别计算与截断（同一块内并行多 query 各自 1–5）。
- 块级总量不设上限——迭代预算天然约束轮数。
- 观察文本头部带本轮汇总行：`本轮检索 N 次，共返回 M 条（k=…/…）`。

## 4. 支柱一：检索日志（LLM 圈选 + 代码水合）

### 4.1 模型侧（最小规则）

- 每个检索结果项的 dict 里由桥直接写入 **`alias` 字段**（`#1 #2 #3…`）——**别名命名空间按 worker 隔离，worker 内跨轮递增不重置**（双 worker 场景不撞车）。对 dense/lexical/graph/chunk_fetch/doc_scan 全部 client 方法统一注入（doc_scan 的结果由模型 Python 自打印，别名随 dict 走，与打印方式无关）。
- worker 收尾时唯一要做的"交接"：一行 `SELECTED: #2, #5, #9`（最终代码块 print 或收尾文本均可，两种来源均可容忍）。**没有 JSON schema、没有 key_facts、没有指针校验**——handoff 契约退化为"分析散文 + 一行圈选"。
- 教学进 codegen SKILL 与 task brief：「收尾时用 SELECTED 列出你实际用到的证据编号；没用到就不写。」

### 4.2 代码侧（全部确定性）

- 桥按 worker 命名空间记录 `alias → chunk_id` 映射（检索返回时天然有序）；**水合在 worker 上下文内完成后再合并**进证据库。
- 解析 SELECTED（正则即可，非契约；写 chunk id 或描述文字的条目直接忽略）→ 水合：去重、按 alias 顺序、取 chunk 全文、附 `{doc, page, chunk_id}` 出处头。
- **圈选 chunk 注册进 store 的 E-id 空间**（与现有渲染/引用同一通道）——Answer 的 `[[E:n]]` 引用、finalize 改写、评测 cited∩gold 全部照旧，不断链。
- **空日志语义（用户决策 ③）**：无 SELECTED 或全部解析失败 → **不兜底**：orchestrator 收到空的圈选区 + `（worker 未圈选证据）` 标记 + 全部 background（本轮捕获但未圈选，分层标注）。orchestrator 据此自决：重派（换策略）或按查无收尾。
- Answer 渲染：圈选区（★ 模型圈选）在前、背景区（○ 本轮捕获）在后且标注；引用锚点由代码生成（`[[E:n]]` 照旧，无需模型写 id）。

### 4.3 与现有机制的替换关系

- `internal_worker_handoff_v1` 的 `key_facts[].evidence` 指针校验（E103）退役——指针由代码水合，模型从不接触 id 本体。
- handoff schema 保留 `summary/gaps/coverage/premise_mismatch`（纯文本语义，零格式成本），`key_facts` 改由 `SELECTED` + 水合替代。
- **E 码存废清单**：E101（handoff 信封校验）退役；E103（指针真实性）退役；**E104 保留**（伪造 `<code_execution_result>` 剥离——散文防线）；**E105 保留**（零检索查无门禁）。
- **散文编造的剩余风险（明示的取舍）**：handoff 散文内容不再有确定性校验。三道剩余防线：① E104；② 只有水合真 chunk 可引用，散文是"说法"不是"证据"（A3 规则：冲突时以证据原文为准）；③ 评测 judge 兜底。接受"偶发散文夸大进 Channel outcomes"换取零格式债。

## 5. 端到端效果

```
worker 4 轮 × 平均 ~3 条/轮（自适应）≈ 12 条候选
  → 模型圈选 ~3-5 条（SELECTED）
  → 代码水合全文 + 出处
  → orchestrator：★ 圈选 3-5 条全文 + ○ 背景 ~7 条（标注）
  → Answer 基于有界、精准、可审计的证据集合成
```

对比现状：store 全量 ~100+ 条（93% 模型没见过）+ 指针全靠手抄。

## 6. 切片

| 切片 | 内容 | 验证 |
|---|---|---|
| K1 | largest-gap 算法 + 平坦度判定（dense/lexical 出口）+ 反馈提示 | 单测：构造 steep/flat/tie 序列断言 k 与提示；真实 rerank 分数冒烟 |
| K2 | 别名映射 + SELECTED 解析 + 水合 + 空日志语义 + Answer 分层渲染 | 单测：别名反查、空日志→空圈选+background 标记、渲染顺序 |
| K3 | handoff 瘦身（key_facts 指针退役、契约简化）+ SKILL/brief 教学 | 既有编译器测试迁移；q104 类场景回归 |
| K4 | 验收跑（q088/q097/q104/q114 + 观察轮） | 圈选率、orchestrator 证据体积、judge 标签 |

## 7. 风险

| 风险 | 缓解 |
|---|---|
| 梯度阈值 ε/γ 不适配新 reranker 分布 | 初值保守 + 评测校准；先报告后调 |
| 模型不写 SELECTED（学习曲线） | 空日志语义已覆盖（orchestrator 自决，不炸）；观察轮看圈选率 |
| 自适应 k 误杀长尾证据 | background 层保留全部捕获，Answer 可见可用，不丢召回 |
| 与现有 rough→final 漏斗重复 | 逐步替换：先并存（取两者交集上限），稳定后退役固定比例 |

## 8. 非目标

- 不做 CAR/NQC 等更重算法（largest-gap 够用且免调参）。
- 不改 orchestrator 编排逻辑（薄编排是更大切片，另案）。
- 不动评测侧（golden/judge 不变；检索轨指标用全流 recall，圈选层是产品内部分层）。
