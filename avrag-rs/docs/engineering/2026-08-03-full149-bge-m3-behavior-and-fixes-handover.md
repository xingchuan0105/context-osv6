# 交接：Full149（bge-m3 重灌）行为诊断 + 已确认修改

| 项目 | 内容 |
|---|---|
| 类型 | 会话交接（诊断结论 + 已落盘文案 + 待续议题） |
| 日期 | 2026-08-03 |
| 分支 | 本地 `master`（solo trunk） |
| 评测跑次 | `E2E_FORCE_INGEST=1 bash scripts/test-full149.sh` |
| 主日志 | `output/runtime-logs/full149_20260803-090334.log` |
| v2 制品 | `avrag-rs/crates/app/tests/e2e_output/rag_eval_v2/v2_20260803-090356/` |
| tool_trace | `avrag-rs/crates/app/tests/e2e_output/realistic_corpus_full_eval/qNNN.json` |
| 前序 | SiliconFlow 迁移：`2026-08-03-siliconflow-migration-handover.md` |

---

## 0. 一句话

**bge-m3 全量重灌 + 新管线 149 题：v2 PASS 138/149（exit 0，~37min）。**  
主路径稳；漏分集中在细字段 / 表口径 / 有据计算。已确认并**落盘**的改动：query-card 计算+检索、sandbox 契约补全、strategies gotcha（表双报、Python 噪声、图绑错等）。**未做** harness stderr 落盘与 graph A/B 代码。

---

## 1. 跑次成绩（已确认事实）

| 指标 | 值 |
|------|-----|
| 配置 | SiliconFlow `Pro/BAAI/bge-m3` + 原件灌库管线；`E2E_FORCE_INGEST=1` |
| 灌库 | 10/10 completed；无 embedding `code:20015` |
| v2 PASS | **138 / 149（92.6%）** |
| mean correctness / faithfulness / relevancy | 0.959 / 0.974 / 0.981 |
| mean recall（golden>0） | ~0.93 |
| 耗时 | ~2203s（含重灌） |
| judge error | 1（q077 INFRA / empty_answer） |
| 对照基线（Qwen 会话） | PASS 141/149、recall ~0.978 → 本轮约 **-3 PASS** |

### v2 非 PASS（11）

| n | subset | label | 一句话 |
|---|--------|-------|--------|
| 26 | thesis_numeric | PARTIAL | 行业规模多口径，未拍板 1467 |
| 30 | thesis_numeric | UNGROUNDED | 纯 calculator×7，数对无依据 |
| 46 | adr_factual | RETRIEVAL_MISS | 答 06-09，gold 06-06（绑错） |
| 50 | adr_factual | RETRIEVAL_MISS | 首轮 code 挂 → 假拒答，tools=[] |
| 53 | adr_factual | PARTIAL | 方法列表只答一半 |
| 58 | cross_adr | RETRIEVAL_MISS | 同 50，tools=[] |
| 77 | ipd_table | INFRA_ERROR | 空答 |
| 78 | ipd_table | SELECTION_MISS | 主推合并 57，gold 行数 81 |
| 91 | baiyao_pdf | SELECTION_MISS | struct 失败拒用正文 11/100/638 |
| 92 | baiyao_pdf | PARTIAL | 通过率三元组不全 |
| 105 | cross_document | PARTIAL | 跨文档类比浅 |

---

## 2. 行为分层结论（已确认）

### 2.1 三类病灶的「转折点」

| 类型 | 关键转折 | 代表 |
|------|----------|------|
| **细字段** | A 字段/文档未消歧就定稿；B 列表早收工；C 执行失败误读成库空 | q046 / q053 / q050·058 |
| **表格数** | D 改计数口径只报一种；E struct 失败禁止用正文精确句 | q078 / q091 |
| **有据计算** | G 意图卡纯 calculator；H evidence 门只挡终答不挡再算 | q030 |

### 2.2 效率 / 质量 / 准确度（摘要）

- mean iters **3.42**，mean tools **2.39**；tool≥5 时 PASS 降到 ~81%。
- `code_gen_error`：**26/149（17.4%）** 题至少一次；**绝大多数最终 PASS**；真·难恢复主要是 q050/058。
- `evidence_missing_continue`：全库 **仅 q030**（路径内 6 次）——不是全局失效，是「纯计算卡 + 题干自给」绕开。
- Host：**单次** sandbox error → Continue + nudge；**连续 2 次**才 BreakToSynthesis。一次挂弃疗是**模型归因**，不是 host 一挂就杀。

### 2.3 已拍板的产品口径

| 议题 | 决定 |
|------|------|
| 表计数 | **允许合并，但必须双报**（行数口径 + 合并口径并列并标明） |
| 计算意图卡 | 文档场景：**calculator + 检索（dense/lexical/grep）**；纯口算才可仅 calculator |
| Graph 定位 | lexical 侧车 1 跳保留；**先观测/A/B，不先再开独立 vector-graph 主通道** |

---

## 3. BM25 + 单跳图：本轮验证结论

### 3.1 机制（现行）

- BM25 = `client.lexical`
- 单跳图 = `RETRIEVAL_GRAPH_AUGMENT=1` 时挂在 **lexical 内**，telemetry/tool 上表现为 `graph_retrieval`
- 模型面 **无** `graph_search`；dense **不**自动带图

### 3.2 tool_trace 计数（本跑 `realistic_corpus_full_eval`）

| 通道事件 | 次数 |
|----------|------|
| doc_grep | 509 |
| dense_retrieval | 299 |
| lexical_retrieval | 263 |
| **graph_retrieval** | **111**（**81/149** 题至少一次） |
| calculator | 24 |

- 有 graph：PASS **96.3%**（81 题）
- lexical 无 graph：PASS **89.8%**（49 题）
- graph 与 lexical **始终同现**（0 次无 lexical 却有 graph）
- **非因果证明**（缺 A/B）；只能说图在约一半题上真实触发

### 3.3 关键失败题与通道

| q | tools（摘要） | 图是否触发 |
|---|---------------|------------|
| 030 | calculator×7 | 否 |
| 046 | dense+lexical+grep **无 graph** | 否 |
| 050 / 058 | **tools=[]** | 否 |
| 078 | dense/lex/graph/grep 都有 | 是（仍 SELECTION_MISS） |
| 091 | struct 为主，无 graph | 否 |
| 130（关系 PASS） | dense+lex+**graph**+grep | 是 |

### 3.4 论文机制迁移性（已确认判断）

| 题型 | 迁移性 |
|------|--------|
| 实体关系链 | 中高 |
| 表计数 / 纯计算 | 低 / 无 |
| 细字段日期 | **当前偏低**——取决于 triple 质量，抽错会帮倒忙 |

---

## 4. Triplet / 日期错题（e2e_smoke 实查）

库：`postgres://…/avrag_rs_e2e_smoke`（本跑灌库后）

| 项 | 结果 |
|----|------|
| 规模 | ~10813 entities / **8064** relations |
| ADR-0009 决策日期 → 2026-06-09 | **有**（多条） |
| ADR-0004 决策日期 / object `2026-06-06` | **0 条** |
| ADR-0004 其它边 | 几乎仅 `属于 Accepted` |
| EvidenceGate / RETAIN 类边 | **有**（q050 失败因零检索，非缺边） |

**含义**：q046 gold 要 0004 的 06-06；图里只有 0009 的 06-09 → 图增强**帮不了**该细字段，甚至可能强化错误日期。根因含 **抽取缺口**（英文 `Date` 行未进三元组）。

---

## 5. 已落盘修改（确认过的文案）

| 改动 | 路径 | 要点 |
|------|------|------|
| sandbox 失败观察 | `avrag-rs/prompts/loop/codegen-sandbox-error.nudge.md` | 补全 calculator / struct_* / user_context / save·load；点名无 `top_k`、`graph_search`、`dense_search` 等旧别名 |
| query-card 计算类型 | `avrag-rs/prompts/pipeline/query-card.system.md` | 文档型计算：`calculator` **连同** dense/lexical/grep；纯自洽算术才可仅 calculator |
| 策略 gotcha + Python 噪声 | `avrag-rs/prompts/capabilities/knowledge-base/reference/strategies.md` | 表双报口径；struct 空仍可用正文枚举；图绑错实体；失败≠库空；纯算空转；codegen 噪声表 |
| skill 版本 | `avrag-rs/prompts/capabilities/knowledge-base/SKILL.md` | **4.3 → 4.4** |

语气约束：第三人称观察 / 误读对照；**无**黄金集原题泄漏；**无**命令式第二政策层。

---

## 6. 明确未做（待续 / 待批）

| 项 | 状态 | 备注 |
|----|------|------|
| `tool_trace` 扩 stderr 截断 + `graph_context_len` | 未做 | 打开逐题 code 失败分类与图「有货」观测的最小工程 |
| `RETRIEVAL_GRAPH_AUGMENT=0|1` 子集 A/B | 未跑 | 建议探针：关系链 + 细字段日期 + 表/纯算负对照 |
| ADR-0004 日期 triple 抽取修复 | 未做 | 英文 Date 行 → `(ADR-0004, 决策日期, 2026-06-06)` |
| 独立 vector-graph 主通道 | **不做优先** | 先观测与 triple 质量 |
| harness 把 tool_trace 并入 v2 artifact | 未做 | 现分两目录 |
| 代码改 agent-loop 硬门（算完必须检索） | 未做 | 现靠 query-card + required_action 结构门 |

### 建议的图有效性测试窗口（备忘）

```bash
# 同 digests，子集题号按需填
RETRIEVAL_GRAPH_AUGMENT=1 E2E_QUESTIONS="..." bash scripts/test-full149.sh
RETRIEVAL_GRAPH_AUGMENT=0 E2E_QUESTIONS="..." bash scripts/test-full149.sh
# 对照: realistic_corpus_full_eval/qNNN.json 的 tool_trace + v2 label
```

看板最小字段：`has_graph`、on/off label·recall、（待加）`graph_context_len`、错绑案例。

### stderr 分类打开方式（备忘）

1. **推荐**：扩展 `rag_quality_prod` 写 `tool_trace` 时带 `stderr`/`error` 截断（500 字）+ 可选 `graph_context_len`  
2. **临时**：`E2E_QUESTIONS` 定向 error 题 + 日志 grep `Execution failed`  
3. 聚合桶：unknown_method / type_error / await / forbidden_import / other；字段 `recovered(bool)`

---

## 7. 优化优先级（已对齐的业务排序）

1. **数据**：细字段 triple / 英文 Date 抽取（日期题）  
2. **卡与策略**：计算+检索（已落盘）；表双报（已落盘）  
3. **可观测**：tool_trace stderr + graph 长度；再跑 graph A/B  
4. **韧性**：失败后勿假拒答（gotcha 已写；硬重试策略未改代码）  
5. **效率**：对抗拒答早停（未做）  
6. **勿优先**：再开独立 graph 通道、用全 149 当 embedding 迭代器

---

## 8. 关键路径速查

| 用途 | 路径 |
|------|------|
| 本交接 | `avrag-rs/docs/engineering/2026-08-03-full149-bge-m3-behavior-and-fixes-handover.md` |
| SF 迁移 | `avrag-rs/docs/engineering/2026-08-03-siliconflow-migration-handover.md` |
| lexical-graph 规范 | `avrag-rs/docs/plans/2026-07-23-lexical-graph-augment-scoring-design.md` |
| Full149 脚本 | `scripts/test-full149.sh` |
| 机读中间表（会话临时） | `/tmp/full149_behavior_diag.json`（可能已过期，以制品为准） |

---

## 9. 续聊入口（给下一棒）

用户将继续讨论；下列为**未关闭问题**，文档仅记账：

- [ ] harness：`tool_trace` + stderr / graph_context_len 是否本波就改  
- [ ] 日期 triple 抽取修复方案（ingestion 侧）  
- [ ] graph A/B 子集题号最终名单  
- [ ] 是否对「失败后假拒答」加结构/观察层（超出 gotcha）  
- [ ] 表 few-shot 是否再加 1～2 个虚构情境（双报）到 skill 正文  

*完。*
