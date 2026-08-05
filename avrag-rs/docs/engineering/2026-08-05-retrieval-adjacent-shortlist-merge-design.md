# 检索短名单 × 长名单邻接合并（cursor 邻域）设计

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-05 |
| 状态 | **已实现**（`merge::adjacent_merge_shortlist_longlist`；dense/lexical 后处理；cursor 自 content_store metadata 或 source_locator）；**默认开**（`RETRIEVAL_ADJACENT_MERGE=0` 关闭） |
| 动机 | 缓解入库切块导致的 **语义/列表断裂**（典型：枚举 (1)(2)(3) 与 (4) 分属相邻 cursor） |
| 非目标 | 全库任意 hit 扩邻；BM25 滑窗重切；Agent 会话三层滑动窗（已废弃） |
| 相关 | `chunker.rs` overlap；`metadata.cursor`；full149 q017 诊断；SkillOpt 残差分层 |

---

## 0. 一句话

**正常产出短名单 S 与长名单 L；交模型证据前，以 S 为锚、在 L 内找同文档顺序编码（cursor）临近的块：邻块已在 S 则合并排序，邻块仅在 L 则拉进证据包。**

---

## 1. 问题背景

### 1.1 切块与 overlap

- 入库：`TARGET_CHUNK_TOKENS=512`，默认 `overlap_chars=64`（≈16 token）——**有 overlap，偏弱**。
- 表格行组路径：**组间无 overlap**。
- Agent「三层滑动窗口」：2026-05  harness 设计，**已回撤**，与本机制无关。
- BM25：对已有 chunk 整段打分，**不是**滑动窗切文。

### 1.2 顺序编码

| 环节 | 字段 |
|------|------|
| 切块 | 同文档递增 `cursor`（0,1,2,…） |
| PG 落库 | `metadata.cursor`（body chunks） |
| 预览/顺序读 | `order by cursor` |
| **`ScoredChunk` 检索返回** | **当前无 cursor**（实现前必须 hydrate） |

### 1.3 为何「只拼短名单内部」不够（q017 实证）

full149 续测 artifact `v2_20260804-154530` / q017：

| 指标 | 值 |
|------|-----|
| gold 四点 first_hit_ranks | `[0, 0, 0, 17]` |
| 评测 k | 15 |
| recall@15 | 0.75（前三点在 top，第四点 rank=17） |
| retrieved_count | 20（全流 recall=1.0） |

主列表块含（1）（2）（3）；**（4）过于追求短期利益** 在另一块。  
**仅 S 内合并**：第四点不在 S → 合不上。  
**S 锚 + L 邻域**（L 若 ≥20）：第四点在 L → 可在 cursor 相邻时 **从 L 拉近**。

多 gold 题（n=58）补充：K=15 时约 79% 全 gold 已同在短名单——名单内合并仍有 **读序** 价值；掉出 top 的断裂需 L 拉近或入库改进。

---

## 2. 机制定义（权威）

### 2.1 名单

| 符号 | 含义 | 建议默认（可调） |
|------|------|------------------|
| **L** | 长名单：检索融合后的宽候选 | dense pool / 融合列表，`min(n, 24~32)`；与 VGRAG pool cap 对齐时可取 **24** |
| **S** | 短名单：当前交给模型的精选 | final cut，产品常见 **≤12**（VGRAG `VGRAG_FINAL_CAP`） |

流程：

```text
retrieve → 得到有序候选 C（按 score）
L := C 的前 L_max 条（或整段 pool）
S := 现网 final_cut(C) 或 top S_max
证据包 E := adjacent_merge(S, L)
→ 模型只看 E（条数 ≤ |S| + budget，见 §3）
```

### 2.2 邻接关系

- **同 `doc_id`**
- **`|cursor_a - cursor_b| ≤ r`**，默认 **r = 1**
- 可选：同 `parse_run_id` / 同 `chunk_type` 族（默认 text 与 text；**table 默认不与 text 互并**）

### 2.3 对每个 `c ∈ S` 在 L 中的动作

| 邻块 n 的位置 | 动作 |
|---------------|------|
| `n ∈ S` | **合并**：同一 run 内按 cursor 升序拼接 `content` |
| `n ∈ L \ S` | **拉近**：将 n 纳入证据包，与 c 同 run 拼接 |
| `n ∉ L` | **不拉**（禁止无名单库侧全库 ±1） |

同一 run 内多个 S 成员：只产生 **一条** 证据条（去重）。

### 2.4 与「禁止方案」的边界

| 方案 | 是否本设计 |
|------|------------|
| 短名单内相邻粘连 | ✅ 包含 |
| 从长名单拉 cursor 邻居 | ✅ 包含 |
| 每个 hit 无视 L 去库 fetch ±1 | ❌ 不做 |
| BM25 滑窗重切原文 | ❌ 不做 |
| 加大入库 overlap / 重灌 | ❌ 本设计不依赖；可并行 |

---

## 3. 合并与预算

### 3.1 拼接

- 同 run：`content = join(chunks_sorted_by_cursor, "\n\n")`
- 可选轻量分隔标记（实现时再定，避免污染 citation 对齐）

### 3.2 Score

- 合并条 score = run 内 **原属 S 的成员的 max(score)**  
- 仅从 L 拉入的块不单独占更高排序键（避免「邻居虚高」）

### 3.3 扩容 cap（防肥 context）

| 参数 | 建议默认 |
|------|----------|
| `r` | 1 |
| 每个 S 锚最多拉入的 L-only 邻块 | 1～2 |
| 单次请求总拉入块数 | ≤ `S_max`（例如 ≤12）或 ≤ 8 |
| 单条合并后 max chars | 可与 2× 单 chunk 预算对齐 |

超 cap：优先保证 **与 S 内最高分 hit 同 doc 的邻块**。

### 3.4 类型过滤

| chunk_type | 默认 |
|------------|------|
| text / markdown 正文 | 参与 |
| table 行组 | **默认不与 text 互并**；table-table 仅当 cursor 相邻且同表 metadata 时可选（P1） |
| multimodal / page_raster | 默认不参与 |

---

## 4. 实现落点（未开工）

### 4.1 前置：cursor 进检索结果

1. `ScoredChunk`（`retrieval-data-plane`）增加可选 `cursor: Option<i32>`（或 `sequence`）。
2. PG / pgvector / Milvus hydrate：从 `metadata.cursor` 或冗余列填入。
3. 无 cursor 的旧数据：跳过邻并（安全降级）。

### 4.2 钩子位置

- **优先**：dense（含 VGRAG fuse）与 lexical 在 **final_cut / 返回 ToolResult 之前** 对 **即将下发的列表** 做 `adjacent_merge(S, L)`。
- L = 同次调用的 pool（fuse 前或 cut 前宽列表）；S = cut 后列表。
- 多路 tool 各自 merge，或统一在 observation 组装层（二选一，实现时定一个，避免双重合并）。

### 4.3 配置

```text
# 建议 env（名称实现时可微调）
RETRIEVAL_ADJACENT_MERGE=1          # 0 关
RETRIEVAL_ADJACENT_RADIUS=1
RETRIEVAL_ADJACENT_LONG_CAP=24      # |L|
RETRIEVAL_ADJACENT_PULL_BUDGET=8    # 本请求最多从 L\S 拉入条数
```

产品默认：**开**（chunk 语义/列表断裂主修复，q017 类）。  
L\S 拉近会把 cut 掉的邻块送进可见包——这是预期增益，不是旁路；用 `RETRIEVAL_ADJACENT_MERGE=0` 做对照实验。跨轮 body 省略（reseen）与邻并正交，不互斥。

### 4.4 可观测

tool_trace / 日志字段建议：

- `adjacent_merge_runs`
- `adjacent_pulled_n`（从 L\S 拉入）
- `adjacent_merged_n`（S 内合并减少的条数）

---

## 5. 预期收益与局限

### 5.1 收益

| 场景 | 预期 |
|------|------|
| 列表 (1)(2)(3) 与 (4) 分块，(4) 在 L 不在 S（q017 类） | 拉近后模型可见完整枚举 |
| 多点已在 S 但 score 序乱 | 按 cursor 读，减少漏并 |
| 相对全库 ±1 | IO 少，只碰 L |

### 5.2 局限

| 场景 | 结果 |
|------|------|
| 邻块连 L 都没有 | 仍无（须入库 overlap / 列表感知切块 / 更深 L） |
| cursor 缺失 | 不合并 |
| 表计数口径、假拒、只 calculator | **不解决**（skill / 路由） |
| L 过深 | 拉近噪声↑，靠 pull budget |

### 5.3 与 SkillOpt

- 本机制是 **检索证据整形**，不替代 tables/grounding skill。
- 对 q017：降低「主块三点当完备清单」的概率；模型仍可能漏写，但证据更完整。

---

## 6. 验收建议

| 级别 | 内容 |
|------|------|
| 单测 | 同 doc cursor 5∈S、6∈L\S → E 含 5+6 拼接；6∉L → 不拉 |
| 单测 | 5,6 均 ∈S → 一条 run，条数 −1 |
| 回归 | q017 类：构造 ranks 0 与 17 在 L、仅 0 在 S → 拉近后含「过于追求短期利益」 |
| full149 | 对比开关；关注 PARTIAL/列表题与 cited_n 是否暴涨 |

---

## 7. 决策记录（本对话）

| # | 议题 | 决定 |
|---|------|------|
| D1 | 合并范围 | **S 锚 + L 邻域池**，非全库扩邻 |
| D2 | 顺序键 | 入库 **`cursor`**（实现须 hydrate 到 hit） |
| D3 | 默认半径 | **r=1** |
| D4 | 与 overlap | 并存；不替代入库改进 |
| D5 | 实现状态 | **先文档，后实现**（本文） |

---

## 8. 后续

1. 实现 P0：cursor hydrate + dense/lexical 后处理 + env 开关。  
2. 用 full149 列表断裂题（q017 等）做开关对照。  
3. 并行讨论 non-PASS 下一题——与邻并正交。
4. 多轮回传叠厚 / 注意力稀释 → 见 `2026-08-05-multi-round-retrieval-context-management-design.md`（EvidencePool × notes × clearing）。
5. **与 P1+ 机制冲突与统一管线** → `2026-08-05-s-plus-l-vs-p1-plus-conflict-and-unification.md`（身份闭包、预算、expand 优先）。
