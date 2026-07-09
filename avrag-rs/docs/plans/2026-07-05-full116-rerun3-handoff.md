# Full116 Rerun3 离线评分 & 挂题深析交接（2026-07-05）

> **接手入口**：本文档汇总第三次 full116 observable 批跑、离线 `rag-diag` 结果，以及 Q11/Q19 逐题深析结论。  
> **前置文档**：[`2026-07-05-full116-observable-diag.md`](./2026-07-05-full116-observable-diag.md)（旧 run `e2e_20260704` + B/C/D 类修复背景）

---

## 1. Run 标识

| 项 | 值 |
|---|---|
| **Run ID** | `e2e_20260705-042359_local_c7f96b3bdb474934bc8eadacc7523e75` |
| 产物根 | `crates/app/tests/e2e_output/rag_quality_smoke_v5/e2e_20260705-042359_local_c7f96b3bdb474934bc8eadacc7523e75/` |
| 批跑日志 | `crates/app/tests/e2e_output/realistic_full_observable_batch_20260705_rerun3.log` |
| Probe | **116/116 产物，0 probe 失败**，~68min |
| 对比基线 Run | `e2e_20260704-172230_local_edb73279542c4f86a634086cf1f6eaa2` |

**批跑前环境修复（e2e PG `avrag_rs_e2e_smoke`）**：

1. migration `0050`（`usage_kind` 列）
2. 清空 `llm_usage_events`；free 计划 token hard/soft limit 置 NULL

---

## 2. 离线评分命令 & 产物

```bash
cd avrag-rs
RUN=crates/app/tests/e2e_output/rag_quality_smoke_v5/e2e_20260705-042359_local_c7f96b3bdb474934bc8eadacc7523e75

cargo run -p e2e-analyzer -- rag-diag \
  --run "$RUN" \
  --golden tests/rag_quality/golden_set_realistic.json \
  --output crates/app/tests/e2e_output/rag_diag_realistic_20260705_rerun.md

cargo run -p e2e-analyzer -- rag-diag \
  --run "$RUN" \
  --golden tests/rag_quality/golden_set_graph.json \
  --output crates/app/tests/e2e_output/rag_diag_graph_20260705_rerun.md
```

| 报告 | 路径 |
|---|---|
| Realistic | `crates/app/tests/e2e_output/rag_diag_realistic_20260705_rerun.md` |
| Graph | `crates/app/tests/e2e_output/rag_diag_graph_20260705_rerun.md` |
| Full116 合并摘要 | `crates/app/tests/e2e_output/rag_diag_full116_summary_20260705_rerun.json` |

---

## 3. 分数对比（修 golden 后 × 新 run）

| 指标 | 旧 run（7/4） | 新 run（7/5 rerun3） | Δ |
|---|---|---|---|
| **Full116 PASS** | ~105/116（96/107 realistic + graph 9/9） | **108/116** | **+3** |
| Realistic 挂题 | 11 | **8**（修 golden 后；Q11 仍挂见 §5.1） | −3 净 |
| Graph | 9/9 | **12/12**（golden 含 NC 重复题） | 持平 |
| 标签 | 全 `RETRIEVAL_MISS` | 全 `RETRIEVAL_MISS` | — |

> **注意**：`rag-diag` 离线评分的 `ret_recall` 对 observable 产物常为 **0%**（`tool_results` 无 chunk 正文），**不能**当检索失败依据；PASS/FAIL 看 `must_include` / `answer_correct`。

### 3.1 本轮 8 挂题清单

| 题号 | subset | faith | 状态 |
|---|---|---|---|
| Q11 | thesis_factual | 0% | ✅ golden 已修，待重评应过（§5.1） |
| Q19 | thesis_synthesis | 100% | ❌ agent query 解析回归（§5.2） |
| Q61 | cross_adr | 0% | ⏳ 待深析 |
| Q77 | ipd_table | 0% | ⏳ 待深析 |
| Q79 | ipd_table | 0% | 已知 A 类，产品暂不动（§5.3） |
| Q95 | baiyao_pdf | 0% | ⏳ 待深析 |
| Q102 | cross_document | 100% | ⏳ 待深析（旧 run PASS → 新挂） |
| Q104 | cross_document | 0% | ⏳ 待深析（C 类，golden 已修但仍挂） |

### 3.2 Golden 批量修复已生效（旧挂 → 新过）

Q17、Q33、Q47、Q49、Q60、Q88、Q90、**Q101** — 共 8 题。

### 3.3 新挂（旧过 → 新挂）

Q11、Q61、Q77、Q95、Q102（+ Q19 层级回退、Q104 仍挂）。

---

## 4. 本轮会话前已完成（背景，勿重复劳动）

| 项 | 状态 |
|---|---|
| D 类 Graph §10 深析写入主诊断文档 | ✅ |
| D 类 P0 eval：`graph_explicit_called` + augment NC 门禁 | ✅ `rag_quality_prod.rs` |
| B 类 golden 批量（Q17/33/47/49/60/88/90 等） | ✅ `golden_set_realistic.json` |
| C 类 Q101/Q104 golden | ✅ |
| A 类 Q79 | 只修 codegen overlap 提示；`table_stats` 等产品侧 **暂不动** |

---

## 5. 逐题深析（本会话）

### 5.1 Q11 — 合成措辞 / golden 锚点（**已过会，golden 已修**）

**Query**：论文使用了哪四个理论工具来分析营销环境？

| Run | 标签 | 答案要点 |
|---|---|---|
| 旧 7/4 | PASS | **外部数据估算**市场规模和结构 + cite |
| 新 7/5 | FAIL | **市场需求（数量与结构）分析**，无 cite |

**结论**：

- 四项语义**全对**；语料双表述：第三章「外部数据估算」vs 摘要「公开资料估算市场需求」。
- 挂因：`must_include` 硬要「外部数据估算」，非检索失败。
- 与 Q11 同类：**答案对、golden 过严**（不是 Q19 那种 agent 回退）。

**已做修改**（`golden_set_realistic.json`）：

```json
"must_include": ["PEST分析", "市场", "波特五力", "内部环境"]
```

> 重跑 `rag-diag` 后 Q11 应转 PASS。次要：合成丢 cite（推理有计划、最终无 `[[cite:]]`），P1 另查。

---

### 5.2 Q19 — agent query 解析回归（**真挂，golden 不动**）

**Query**：论文提出 Y 冷冻设备公司应该从哪三个方面进行能力建设？

| Run | 解析落点 | 答案 |
|---|---|---|
| **旧 7/4** | **§6.1.2 研发能力建设** | 研发硬件设施 / 大数据指导研发 / 增强研发团队 → **PASS** |
| **新 7/5** | **第六章总述**（绪论/路线图） | 公司能力建设 / 管理制度调整 / 营销活动量化管理 → **FAIL** |

**Golden 锚点**（未改）：§6.1.2 三项 — `研发硬件设施`、`大数据指导研发`、`研发团队`。

**结论**：

- **不是**「和 Q11 相反、该放宽 golden」；而是**同一 query、两轮 agent 解析层级相反**。
- 本轮被绪论「三个方面…6项措施」吸走；推理意识到 §6.1.2 可能更贴切，但 **iteration budget 用尽** 后选了粗层。
- 归类：**P1 query 解析 / 层级消歧**（「能力建设」须下钻 §6.1.2，勿停第六章总述）。

**曾误改 golden 为第六章三分法 → 已撤回。**

**修复方向（产品，非 golden）**：

- codegen / rag-answer：「能力建设」+「三个方面」→ 优先 `doc_profile` 看 6.1 小节或 `chunk_fetch` §6.1.2
- 可选：query 加「研发」消歧（仅当有意测 §6.1.2）

---

### 5.3 Q79 — 已知 A 类（接受偏差）

- `doc_chunks` 计数 93 vs 92（chunk overlap）
- 已修 codegen SKILL overlap 提示；产品 `table_stats` **本轮不修**
- 继续挂属预期内

---

## 6. 待接手队列（逐题深析顺序建议）

1. **Q61** — cross_adr，Slice/Phase 计数（新挂）
2. **Q77** — ipd_table，阶段列表（新挂）
3. **Q95** — baiyao_pdf，S 级项目标准（新挂）
4. **Q102** — cross_document，竞争性原则（新挂，faith 100%）
5. **Q104** — cross_document，客户重要性（C 类，golden 已修仍挂）
6. **Q79** — 核对是否仍 93/92，记录即可

**建议节奏**：每题拉 `response.json` + `sse_events.jsonl` reasoning → 判 golden / 解析 / 检索 / 合成 → 再决定修 golden 还是开产品项。

---

## 7. 关键路径速查

| 用途 | 路径 |
|---|---|
| Golden realistic | `tests/rag_quality/golden_set_realistic.json` |
| Golden graph | `tests/rag_quality/golden_set_graph.json` |
| Eval 门禁 | `crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs` |
| 离线评分 | `crates/e2e-analyzer/src/rag_diag.rs` |
| 语料 fixture | `crates/app/tests/product_e2e/fixtures/thesis_y_refrigeration.txt` |
| 主诊断（旧 run + D 类 §10） | `docs/plans/2026-07-05-full116-observable-diag.md` |
| Graph 通道分析 | `docs/plans/2026-07-04-graph-channel-analysis.md` |

---

## 8. 接手后立即可做

```bash
# 1. 验证 Q11 golden 修复
cd avrag-rs
RUN=crates/app/tests/e2e_output/rag_quality_smoke_v5/e2e_20260705-042359_local_c7f96b3bdb474934bc8eadacc7523e75
cargo run -p e2e-analyzer -- rag-diag --run "$RUN" \
  --golden tests/rag_quality/golden_set_realistic.json \
  --output /tmp/rag_diag_post_q11_fix.md
# 确认 realistic_q11 → PASS

# 2. 更新合并摘要（可选）
# 编辑或脚本重生成 rag_diag_full116_summary_20260705_rerun.json

# 3. 继续 Q61 深析
# 读 RUN/realistic_q61/{response,metadata,sse_events}.json(l)
```

---

## 9. 分类速记（避免混淆）

| 类型 | 含义 | 本会话例题 |
|---|---|---|
| **B** | 答案对，golden/`must_include` 过严 | Q11 |
| **G** | 合成措辞/层级 | Q11 cite 丢失 |
| **Query 解析** | 同 query 层级选错（非 golden 问题） | **Q19**（旧 §6.1.2 ✓ → 新第六章总述 ✗） |
| **A** | 真产品缺陷 | Q79（已知接受） |
| **C** | 跨文档表述 vs golden 句式 | Q104（待续） |
| **D** | Graph 通道指标 | 9/9 PASS；NC 误触已修 eval |

---

*文档版本：2026-07-05 会话末；Run rerun3 离线评分 + Q11/Q19 深析交接。*
