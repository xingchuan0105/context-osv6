# RAG Agent 问答校正 & 多轮记忆测试总结（2026-07-06）

> **会话范围**：从交接文档 `2026-07-05-full116-rerun3-handoff.md` 出发，解决 Q19/Q77 层级歧义问题，重写合成 prompt（reporter 姿态），新增多轮记忆与指代消解测试套件。

---

## 1. 问题起点：Q19 的「主观陷阱」

交接文档 §5.2 记录了 Q19（`论文提出Y冷冻设备公司应该从哪三个方面进行能力建设？`）的 agent query 解析回归：

- 旧 run（7/4）命中 §6.1.2 研发三项 → PASS
- 新 run（7/5 rerun3）命中第六章总述保障三方面 → FAIL

深析 reasoning 发现：agent 在 budget 耗尽后，把字面第一个命中「三个方面」的段落当答案，事后用「常见理解」合理化。这是**答题者姿态**的典型失败——把「找到一个说得过去的命中」当成功，而非「把语料里多种读法如实摆出来」。

## 2. 改动一：rag-answer.md → v0.3 reporter 姿态

### 设计原则

将 agent 角色从「答题者」翻转为「检索过程的陈述者」：
- query 是线索，不是考题
- findings（语料说了什么）与 interpretation（你在多个发现间如何取舍）**物理分栏**
- 判断须自报家门，不得伪装成事实

### 关键设计决策（经 code review 校正）

| 决策 | 初稿 | 修正后 | 原因 |
|---|---|---|---|
| schema_version | `internal_answer_v2` | 保持 `internal_answer_v1` | `answer_contract.rs:274` 硬校验 v1，v2 会被打回 → contract_violation_fallback → 全量挂题 |
| findings/interpretation | 独立 schema 字段 | v1 附加字段（serde 无 deny_unknown_fields） | 零代码通过校验，token 流仍可观测 |
| 措辞密度 | 三段训诫 + 失败模式清单 | 简洁协议，失败模式由结构兜住 | prompt 是协议不是说服文 |

### 结构推断 vs 编造规则（Q77 修复）

Q77（华为IPD阶段）首轮 A/B 暴露：agent 看到「生命周期」有文档佐证（LMT 团队负责生命周期监管），但因「不得假装已验证」规则把它压下了。

修正规则区分两种情况：
- **结构推断**（语料有文字佐证但不在主列举）→ answer_text 正常陈述 + cite
- **编造**（语料完全没有文字佐证）→ 写入 caveat，不进 answer_text

## 3. 改动二：codegen SKILL 路由行

在 `prompts/clusters/codegen/SKILL.md` 首轮路由表新增：

```
| 结构性列举（哪几个方面/分为几类/包括哪些内容）且可能存在层级歧义 | doc_profile()（sections 树）+ dense_search 同块并行 |
```

检索阶段用 `doc_profile()` 拿到 sections 树，当场消解层级歧义（如 Q19 看到 6.1 下只有两个小节、6.1.2 才是「三个内容」），比合成阶段补救更治本。

## 4. 改动三：golden scorer 支持 OR 跨组

### 背景

Q19 在语料中有两套自洽读法，rigid `must_include`（AND）只接受其一，导致诚实报告歧义反而受罚。

### 实现

新增字段 `alternative_answer_sets: Vec<Vec<String>>`：
- 组内 AND（一组内所有关键词都命中）
- 组间 OR（任一组全中即过）
- 空 = 原 AND 语义，向后兼容

改动三处 scorer：
- `tests/rag_quality/src/golden_set.rs` — GoldenExample 加字段
- `tests/rag_quality/src/metrics_v2.rs` — `answer_correctness()` 加 OR 逻辑（决定 PASS/FAIL 标签）
- `crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs` — e2e 统计 scorer 加 OR

## 5. A/B 验证结果

### Q11/Q19/Q77 定向 A/B（两轮）

| 题号 | rerun3 (7/5) | Round 1 (v0.3) | Round 2 (+结构推断) |
|---|---|---|---|
| Q11 | FAIL | **PASS** | **PASS** |
| Q19 | FAIL | **PASS** | **PASS** |
| Q77 | PASS | FAIL（漏「生命周期」） | **PASS**（6阶段全中） |

Round 2 全 3/3 PASS，prompt 改动无回归。

## 6. 改动四：多轮记忆与指代消解测试套件

### 设计

新增 `golden_set_multiturn.json`（13 题）+ multiturn harness + LLM-as-Judge：

| 维度 | 题数 | 轮数 | 测什么 |
|---|---|---|---|
| 近程指代 | 2 (M1-M2) | 2 | 2-prior 注入窗口内消解 |
| 远程指代 | 3 (M3-M5) | 4 | T1 出 2-prior 窗口 |
| 跨文档消歧 | 2 (M6-M7) | 3 | 同名概念跨文档 |
| 话题切换 | 3 (M8-M10) | 3 | 旧指代失效 |
| 高难度远程 | 3 (H1-H3) | 6 | 中间4轮全切文档，逼出 history_load |

### Judge 设计

- Judge **独立**推断指代对象（不告诉它 expected referent）
- 判定 `anaphora_resolved` + `answer_correct` 两个布尔
- 硬门禁：≥60% 的题两项都通过

### 结果：13/13 全通 (100%)

关键发现：
- **H2 是唯一触发 `conversation_history_load` 的题**——因为 T6 query「那个决策中被废弃的组件叫什么？」完全领域无关，无法靠检索绕过指代消解
- H1/H3 的 T6 query 含「速冻机」关键词，agent 直接检索到答案，不需消解指代
- **测纯粹指代消解能力**的范式：T6 query 必须做到领域无关

---

## 7. 文件变更清单（本次会话）

| 文件 | 变更 |
|---|---|
| `prompts/synthesis/rag-answer.md` | 重写为 v0.3 reporter 姿态 + 结构推断/编造区分 |
| `prompts/synthesis/rag-answer.md.v1-reporter-pre` | v1 备份（untracked） |
| `prompts/clusters/codegen/SKILL.md` | 新增结构性列举路由行 |
| `tests/rag_quality/src/golden_set.rs` | 新增 `alternative_answer_sets` + multiturn 类型（`GoldenMultiturnExample` 等） |
| `tests/rag_quality/src/metrics_v2.rs` | scorer OR 跨组逻辑 + test literal 补字段 |
| `tests/rag_quality/src/tool_coverage.rs` | test literal 补字段 |
| `crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs` | e2e scorer OR + `realistic_multiturn_observable_probe` + Judge |
| `tests/rag_quality/golden_set_realistic.json` | Q19 加 `alternative_answer_sets` |
| `tests/rag_quality/golden_set_multiturn.json` | **新文件**：13 题多轮测试 |

---

## 8. 未做 / 后续

| 项 | 说明 | 优先级 |
|---|---|---|
| full116 rerun 验证 | prompt v0.3 是全局改动，blast radius = 全部 116 题 | 建议 |
| orchestrator 双锚下钻 budget | codegen 路由行治本，但极端情况仍可能 budget 不足 | P1 |
| `coverage: "ambiguous_reported"` scorer 奖励 | 当前 scorer 只读 answer_text 纯文本，不解析 envelope | P2 |
| H1/H3 T6 query 去领域关键词 | 当前含「速冻机」可绕过指代消解 | 可选 |
| search / chat skill 测试 | 下一步工作 | — |

---

*文档版本：2026-07-06 会话末；RAG 问答校正收工。*
