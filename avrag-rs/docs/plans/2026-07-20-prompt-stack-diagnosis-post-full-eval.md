# 诊断说明：Option D 主路径提示词叠层与可精简项

> **SUPERSEDED** — 本文描述的 orchestrator / worker / brief / handoff 多 agent 架构已被取代：2026-07-30 起产品路径改为单 agent（SaC 设计，见根 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），orchestrator 代码已于 2026-08-01 物理删除（commit `7f2d182d`）。本文仅作历史记录。（横幅添加于 2026-08-02 文档体系梳理）

| 项目 | 内容 |
|------|------|
| 状态 | **事实链已闭合 · PR-A 已开工落地（2026-07-20）** — Worker/cap 双轨拆除见 `mode_assemble` / modes yaml / `host::run_chat`；P1 叠层未做 |
| 日期 | 2026-07-20 |
| 读者 | 产品 / 实现负责人；落地 PR 须对照 §7–§8 与 Option D 锁定项同步改文档 |
| 关联 | [Option D 设计](./2026-07-20-unified-product-agent-option-d.md)、[测试缺口](./2026-07-20-option-d-test-gap-and-drift.md)、[编排提示词优化](./2026-07-20-orchestrator-prompt-engineering-optimization.md) |
| 评测锚点 | `realistic_corpus_full_eval`，`E2E_FAIL_FAST=1`；日志 `full_eval_failfast_*_from{88,125,129,142}.log`；dump `e2e_output/realistic_corpus_full_eval/q*.json` |
| 复核 | 人工逐条核验可证伪 claim 全部属实；核心判断「协议双轨」成立，且 **比初稿更深一层**（见 §3.2） |

---

## 0. 本文要回答什么

1. **当前主路径上，LLM 实际吃到了哪些提示词**（不是目录里有哪些文件）？
2. 这些提示词之间是否 **职责重叠 / 协议冲突**？
3. full_eval 上的失败与质量标签，更像 **缺提示** 还是 **提示过多 / 协议双轨**？
4. 哪些可以 **删除 / 停挂 / 合并 / 精简**，哪些必须保留？
5. 哪些建议 **reopen Option D 锁定项**，落地时必须同步改设计文？

本文 **不做实现**；§13 给出落地时不可拆半的处方范围。

---

## 0.1 复核一句话结论

> **诊断可以采信。** P0 方向正确，但 **P0-1 不可只摘 yaml mandatory**：双轨的另一轨硬编码在 `agent-loop` 的 `synthesis_output.contract` + `synthesis_contract_block` + `complete_json_mode` 里。只摘 skill = **合同还在、老师没了**（半吊子）。完整 P0-1 = **yaml + worker contract/loop_exit + dual assemble** 一起改。

---

## 1. 背景：产品意图 vs 历史 monomode

### 1.1 Option D 已拍板的终态（摘要）

| 决策 | 含义 |
|------|------|
| Dispatch / Answer 两阶段 | 有 rag/search：先协调派活与取证，再写用户可见答案 |
| 无 capability | pure chat：不经编排，直接 AnswerOnly |
| **OQ-Cite=A** | 用户侧引用以 **`[[E:n]]`** 为准，host `finalize` 写成 `[[cite:chunk_id]]` / `[[web:n]]` |
| **OQ-Tools** | Answer / pure chat 只挂 **效用工具**；禁止 Answer 再检索/派活 |
| 证据条数 | chunk ~512；TOPK/TOPN 定条数；store 去重赋 `En`，不二次截断 |
| doc_scan | **代码侧** 装段统计；背景化叙事优先于禁则堆叠 |
| **KD-6** | 作答积木 **按 store 实际材料** 选型，不按用户勾选硬套 |
| **KD-12** | Answer system = **`product-answer-base` + `chat-base`** + 积木（chat-base 承载 memory `skill_request` 协议） |
| **KD-13** | 合成 skill 经 DisclosurePlanner；AnswerOnly 设计文写 `mandatory_synthesis: [chat]` |

### 1.2 历史 monomode 残留

P2 已退役 `rag-system` / `search-system` / `chat-system` → `prompts/deprecated/monomode-system/`。  
主路径文案改为 capability / chat-base / orchestrator-base，但：

1. **modes/*.yaml** 仍挂 mandatory synthesis（`rag-answer` / `search-answer` / `chat`）
2. **agent-loop 代码合同** 仍按 `synthesis_output.contract` 拼 JSON envelope（见 §3.2）

合成 skill 教的旧合同：JSON envelope、`[[cite:UUID]]` / `[[n]]`、depends `grounded-answer`。  
与 Option D Answer 的 **ProseOnly + `[[E:n]]`** **不是同一协议**。

---

## 2. 运行时真实装配（复核用锚点）

下列为 **代码路径**，复核时已逐条核对。

### 2.1 总览

```
用户 query + capabilities
        │
        ├─ capabilities 空 ──► AnswerOnly
        │                      system: chat-base
        │                      tools: utility pool
        │                      ModeConfig ← assemble_mode(default) / chat.yaml
        │                        mandatory_synthesis: [chat]
        │                        contract: ProseOnly
        │                        early-stop 时常不进合成；一旦进合成 → 叠 synthesis/chat.md
        │
        └─ rag 和/或 search ──► Orchestrator (Dispatch)
                                  system: orchestrator-base
                                         + channel_dispatch_manual(「给任务分配者」切片)
                                  tools: delegate_* / finish_answer / …
                                        │
                    ┌───────────────────┴───────────────────┐
                    ▼                                       ▼
              Worker RAG                              Worker Search
              system: capability-rag                  system: capability-search
                      + codegen (disclose)                    + search cluster
                      + brief 内嵌 handoff JSON 规则   ← Option D 意图终点
              modes / assemble:
                mandatory synthesis: rag-answer         search-answer
                contract: InternalAnswerUnifiedV1       ← 代码硬编码合同（§3.2）
                loop_exit: require_evidence, 无 early-stop → 合成阶段几乎必走
                synthesis 追加 synthesis_contract_block()（JSON + user-visible markdown）
                complete_json_mode
                                        │
                                        ▼
                              EvidenceStore（En 编号）
                                        │
                                        ▼
                              Answer（run_chat）
                              system_prompt_parts:
                                1. product-answer-base
                                2. chat-base
                                3–6. answer_rule_parts（按 store 有料条件注入，KD-6）
                              query: chat_exit Evidence + [[E:id]] 合同
                              tools: utility pool
                              host 覆盖: contract=ProseOnly, early-stop 开
                              ⚠️ skill_catalog 未覆盖 → 仍继承 chat 的
                                 mandatory_synthesis: [chat]
                                 工具调用后 / 未 early-stop 时会叠 synthesis/chat.md（英文长文人设）
                              finalize: normalize_loose_e_markers + rewrite [[E*]]
```

### 2.2 代码锚点

| 行为 | 位置 |
|------|------|
| pure / capability system 零件 | `app-chat/src/mode_assemble.rs` |
| 任意 cap 路径将 contract 设为 `InternalAnswerUnifiedV1` | 同文件 assemble 分支（rag/search） |
| Answer pack 零件 + ProseOnly + utility；**不改 skill_catalog** | `orchestrator/host.rs` → `run_chat` |
| Worker brief handoff JSON + 单通道 assemble | 同文件 → `run_channel` |
| 合成阶段把 **代码合同块** 追加进 system | `agent-loop/.../synthesis.rs` + `answer_contract.rs::synthesis_contract_block` |
| 协调者「给任务分配者」切片 | `orchestrator/brain.rs` |
| Evidence + E 合同写入 query | `orchestrator/chat_exit.rs` |
| peel 旧 envelope / loose E 归一化 | `orchestrator/workers.rs` |
| mode mandatory synthesis | `modes/{rag,search,chat}.yaml` |

### 2.3 提示文件体量（行数，含 frontmatter；复核 wc 全过）

| 文件 | 约行数 | 主路径角色 |
|------|--------|------------|
| `product-answer-base.md` | 18 | Answer system |
| `chat-base.md` | 26 | pure chat + Answer 叠挂 |
| `answer-follow-brief.md` | 13 | Answer 积木 |
| `answer-from-workspace.md` | 16 | 积木（有 doc 才注入） |
| `answer-from-web.md` | 16 | 积木（有 web 才注入） |
| `answer-dual-source.md` | 15 | 积木（doc∧web） |
| `orchestrator-base.md` | 41 | Dispatch |
| `capability-rag.md` | 63 | Worker 全文；Dispatch 切片 |
| `capability-search.md` | 58 | 同上 |
| `clusters/codegen/SKILL.md` | 94 | Worker RAG 披露 |
| `synthesis/rag-answer.md` | 39 | rag mandatory |
| `synthesis/search-answer.md` | **217** | search mandatory（> product-answer+chat-base+四积木之和 **103**） |
| `synthesis/grounded-answer.md` | 80 | depends |
| `synthesis/chat.md` | 78 | chat mandatory；**Answer 继承** |

---

## 3. 协议双轨（核心判断 · 已加深）

### 3.1 三层期望终点

| 阶段 | 期望输出 | 引用形态 | 谁规定 |
|------|----------|----------|--------|
| **Worker（Option D 意图）** | `internal_worker_handoff_v1` | observation pointer；**不写用户长文** | `host::run_channel` **brief 硬编码** schema |
| **Worker（仍在跑的 monomode 轨）** | unified / search / answer JSON envelope | `[[cite:UUID]]` / `[[web:n]]`；**answer_text = user-visible markdown** | ① yaml mandatory skill ② **`synthesis_contract_block` 代码** |
| **Answer（Option D 落地）** | 用户可见散文 | **`[[E:n]]`** → finalize | host `ProseOnly` + chat_exit + product-answer |

### 3.2 双轨不只是「两个 md 文件撞车」

复核补强（初稿未写清、现为 **闭合事实**）：

1. **`answer_contract.rs::synthesis_contract_block`**（约 76–103 行）按 `mode.synthesis_output.contract` **在代码里**拼出 `internal_answer_unified_v1` 合同，明确：
   - `answer_text = user-visible markdown`
   - Doc: `[[cite:CHUNK_ID]]`；Web: `[[web:n]]`
2. **`synthesis.rs`** 将该块追加到合成 system 末尾，并用 **`complete_json_mode`** 调模型。
3. **`mode_assemble`** 对任意 cap 路径把 contract 设为 **`InternalAnswerUnifiedV1`**（rag-only / search-only / dual 均如此；yaml 里的 `internal_answer_v1` / `internal_search_answer_v1` 会被 assemble 逻辑覆盖或合并为 unified 路径）。
4. Worker **loop_exit** 默认 `require_evidence=true`、`allow_content_early_stop=false`、`skip_synthesis_on_direct_answer=false` → 有证据后 **几乎必进合成**，brief 的 handoff JSON **抢不过** 合成合同。
5. `workers::parse_worker_handoff` **peel** `internal_answer_v1`：把「用户可见长文 envelope」降级当 summary 用 —— 双轨的 **运行时补丁**，不是协议统一。

**恶心细节（复核原文）：** 代码合同教 worker 写 **user-visible markdown**，host 再 peel 成 handoff summary。  
→ 双轨 = **代码合同 vs brief**，不是仅 skill 文件重复。

### 3.3 对 P0-1 的直接推论

| 只做 | 结果 |
|------|------|
| 只摘 `rag-answer` / `search-answer` mandatory | 合同块仍在 + json_mode 仍在 → handoff 仍抢不赢；且 **失去教 schema 的 skill** → 更易畸形 envelope |
| **完整处方（必须）** | ① worker 侧 `synthesis_output.contract` → **ProseOnly**（或新增 **handoff 专用合同**）② `loop_exit.allow_content_early_stop` / `skip_synthesis_on_direct_answer` **放开**，使 handoff JSON 能成为 final message ③ 再摘 mandatory synthesis ④ dual assemble 同步 ⑤ 相关单测 |

**宁可不做半截 P0-1。**

### 3.4 与 Answer 引用的关系

- Answer / eval `expect_citations` 看 finalize 后的 `chat.citations`。
- 继续加长 Answer 禁则 **解决不了** Worker 合成抢戏；Q142 类应用 **代码归一化 + 单合同**。

---

## 4. Answer 阶段叠层

### 4.1 Grounding 同义重复（材料本体只在 query）

| # | 来源 | 形态 |
|---|------|------|
| 1 | `product-answer-base.md` | system 短文 |
| 2 | `chat-base.md` | 有证据按证据 |
| 3 | `answer-follow-brief.md` | 听写作说明 |
| 4 | `answer-from-workspace.md` | 有 doc 才注入（KD-6） |
| 5 | `answer-from-web.md` | 有 web 才注入 |
| 6 | `answer-dual-source.md` | doc∧web |
| 7 | **`chat_exit` query** | **唯一 Evidence 全文 + Citation 合同** |

1–6 不携带证据正文；7 才是材料本体。

### 4.2 双人设：pure chat **与 Answer** 都会叠 `synthesis/chat.md`

| 路径 | 机制 |
|------|------|
| pure chat | `chat.yaml` `mandatory_synthesis: [chat]`；early-stop 时常不灌；进合成则叠英文长文 |
| **Answer（复核补强）** | `run_chat` 用 `assemble_mode(CapabilitySet::default())` 得 ModeConfig，**只改** tool_pool / loop_exit / contract，**不覆盖 `skill_catalog`** → 原样继承 `mandatory_synthesis: [chat]`。工具调用后或未 early-stop 时，**英文 `synthesis/chat.md` 叠在 product-answer-base + chat-base 之上** |

这加强了 **P0-2**（清/降 chat mandatory）与 **P1-2**（Answer 与 chat-base 叙事抢戏，见 Q129）的论据。  
§2.1 已补画这一层。

### 4.3 chat-base 与 Q129 拒答话术

`chat-base` 含「不执行检索 / 不假装查过 / 可建议用户勾选能力」。  
有 Evidence 的 Answer 轮次里，该叙事与「必须根据材料作答」**抢戏**（Q129 首次：有 bridge 仍称没联网）。  
P1-2 方向合理，但 **reopen KD-12**：摘掉完整 chat-base 前必须保证 `{"skill_request":["memory"]}` 协议不断链（见 §8）。

---

## 5. full_eval 行为对照

### 5.1 契约失败（fail-fast）

| 题 | 失败 | 归类 | 证据 |
|----|------|------|------|
| **Q125** 天气 | G-17 weather 像未命中 | **环境**（无 OPENWEATHER key → Error；旧 gate 只计 Ok） | `mock_weather_server`；`from125` 日志 `mock weather` + `tool_hit: weather_query ok` |
| **Q129** 小菜园（首次） | expect_citations 0 | **生成抖动 + 拒答话术**（dispatch 有 item_count，有 dense bridge，答「没联网」） | 首次 fail 日志；**`from129` / `from142` 续跑日志** 重跑通过（dump 现为通过态） |
| **Q142** 手艺人诅咒（首次） | expect_citations 0 | **格式**：`[**[E3]**]` 等，finalize 不认 | 代码 `normalize_loose_e_markers` 注释明引 Q142；`from142` 通过后 dump 为最新态，原始松散标记靠 **注释 + 历史 fail 日志** 交叉印证 |

### 5.2 质量标签（不 fail-fast）

`GENERATION_UNGROUNDED` / `REFUSAL_WRONG` 等：质量问题，优先查证据是否进 query、是否遵循 **单一** 合同，而非再堆 monomode 合成文。

### 5.3 正向观察

- G-16 dense_retrieval bridge、G-17 utility（mock 后）、精确数字在 cite 正确时可答对 → 主链路可工作。

---

## 6. Worker：capability / codegen

### 6.1 保留

| 资产 | 理由 |
|------|------|
| capability「工作背景」+ codegen 方法表 / doc_scan | 对齐 TOPK / 代码侧统计 |
| `## 给任务分配者` 切片 | 协调者读者分离正确 |
| `orchestrator-base` | 短、单一 |

### 6.2 有害或过时

| 资产 | 问题 |
|------|------|
| capability-rag **「引用符号 `[[cite:CHUNK_ID]]`」** + 「最终回答由合成阶段」 | **错阶段教学**（worker 不写用户答案） |
| mandatory `rag-answer` / `search-answer` | 与 handoff 抢终点；token 大户 |
| **代码 unified 合同** | §3.2：比 skill 更硬 |
| capability 与 codegen 背景复述 | 可只留 codegen |

P2-1 删 `[[cite:]]` 段：**安全**（worker evidence pointer 不依赖该教程）——复核同意。

---

## 7. 建议处置（复核修订后）

### P0 — 必须完整做（双轨是质量与 token 双重根因）

| ID | 处方 | 状态 |
|----|------|------|
| **P0-1（扩 scope）** | **不可只摘 yaml。** 必须同时：① worker（及 dual assemble）`synthesis_output.contract` → ProseOnly **或** 新增 handoff 合同 ② `allow_content_early_stop` + `skip_synthesis_on_direct_answer` 放开，handoff JSON 可作 final ③ 再摘 `rag-answer`/`search-answer` mandatory ④ `run_channel` / assemble 单测 ⑤ 避免「合同在、老师没了」 | **同意；半截宁可不做** |
| **P0-2** | 取消或降级 `mandatory_synthesis: [chat]`（pure + **Answer 继承路径**一并处理：Answer 须显式清空或覆盖 skill_catalog） | **同意，但 reopen Option D §5.1 / KD-13** — 落地 PR 同步改设计文 |
| **P0-3** | monomode synthesis 文件退出主路径；**可留盘**供 `token_budget` `include_str!` 引用 | **同意；无需例外入口** |

### P1 — 部分同意

| ID | 处方 | 复核 |
|----|------|------|
| **P1-1** | ~~合并四积木为恒注入单一块~~ **否。** 可：**压缩各块行数**；或 **follow-brief 并入 product-answer-base**。**doc/web/dual 条件注入必须保留（KD-6）** | **部分同意** |
| **P1-2** | Answer 弱化/摘完整 chat-base 抢戏叙事 | **同意方向**；**reopen KD-12**；必须保住 memory `skill_request` 协议 |
| **P1-3** | 详细 E 合同仅 `chat_exit` 一处；system 一行 canonical；依赖 `normalize_loose_e_markers` | **同意** |

### P2 — 同意

| ID | 处方 |
|----|------|
| P2-1 | capability-rag 删用户合成 / `[[cite:]]`；写清「交 handoff，不写用户长文」 |
| P2-2 | capability-search 保留双语/空结果早停；弱化用户长文 `[[n]]` 教材 |
| P2-3 | codegen 与 capability 去重 |
| P2-4 | orchestrator-base 保留 |

### P3 — 仓库卫生

deprecated / `_backups` 不进主路径；禁则堆叠 PR 对照 doc_scan 背景化纠偏。

### 明确保留

`chat_exit` Evidence 注入；finalize + loose E；utility schema；Dispatch 切片；codegen 方法表。

---

## 8. 与 Option D 锁定决策的冲突（落地必须同步改设计文）

| 建议 | 冲突锁定 | 说明 |
|------|----------|------|
| P0-2 取消 `mandatory_synthesis: [chat]` | Option D §5.1 system 行 + **KD-13** | 设计写明 AnswerOnly = chat-base + synthesis skill 披露（mandatory `[chat]`） |
| P1-2 Answer 摘完整 chat-base | **KD-12** | 锁定 product-answer-base + chat-base；memory 协议在 chat-base |
| P1-1 若取消条件注入 | **§6.1 / KD-6** | 按 store 有料选型；恒注入 web 规则到 doc-only 轮 = 另一侧「按勾选硬套」 |

方向可合理，但 **PR 必须同步改 Option D 对应行**，否则两文长期漂移。  
host 现有测试按 **文件名** 断言积木选型；改积木文件集合 = 改 KD-6 测试契约。

---

## 9. 复核清单（已勾选）

- [x] §2 装配描述与代码一致（已逐条核；现稿已补 Answer 继承 chat synthesis）
- [x] §3 双轨成立 — **同意，且补充一轨在 `answer_contract.rs` 硬编码**
- [x] §5 评测归类可接受；Q129 证据行已补 from129/from142；Q142 靠注释+日志交叉印证
- [x] **P0** — 同意；**P0-1 必须扩 scope（contract + loop_exit + dual assemble）**
- [x] **P1** — 部分：P1-2、P1-3 同意；P1-1 只压缩/并 follow-brief，**保留条件注入**
- [x] **P2** — 同意
- [x] 例外入口 — **无需**保留 monomode 主路径；文件可留盘给 token_budget
- [x] 落地顺序 — **先 P0，后 P1**；两 PR 均同步 Option D 文档锁定行

---

## 10. 目标心智图（修订后）

```
Dispatch:  orchestrator-base + 「给任务分配者」×N
Worker:    capability-*（无用户合成、无 UUID cite 教程）
           + codegen|search skill
           + brief handoff JSON 即 final（ProseOnly 或 handoff 合同；可 early-stop）
           ✗ 不再：unified JSON 合同教 user-visible markdown
Answer:    product-answer-base（± 并入的 follow-brief）
           + chat-base 或 KD-12 修订后的 memory 协议载体
           + answer_rule_parts 条件注入（KD-6 保留）
           + query: Evidence + 唯一 [[E:id]]
           + utility；skill_catalog 显式控制（勿默默继承 chat 长文）
Pure chat: chat-base + utility（synthesis/chat 是否 mandatory 按 KD-13 修订）
```

---

## 11. 非目标

| 非目标 | 说明 |
|--------|------|
| 本文直接改代码 | 处方在 §7/§13 |
| 宣称 1–149 单次全量质量基线 | 多为 START_AT 续跑 |
| 质量标签当 fail-fast | 与执行口径一致 |
| 用更多禁则代替协议统一 | Q142 反例 |
| P1-1 取消条件注入 | 与 KD-6 冲突，已否决恒注入版 |

---

## 12. 附录：路径速查

```
avrag-rs/modes/{chat,rag,search,orchestrator}.yaml
avrag-rs/prompts/orchestrators/*
avrag-rs/prompts/synthesis/{chat,rag-answer,search-answer,grounded-answer}.md
avrag-rs/prompts/clusters/codegen/SKILL.md
avrag-rs/crates/app-chat/src/mode_assemble.rs
avrag-rs/crates/app-chat/src/orchestrator/{host,brain,chat_exit,workers}.rs
avrag-rs/crates/agent-loop/src/react_loop/{answer_contract,synthesis}.rs
avrag-rs/crates/app/tests/e2e_output/logs/full_eval_failfast_*_from*.log
avrag-rs/crates/app/tests/e2e_output/realistic_corpus_full_eval/q*.json
avrag-rs/crates/app-chat/src/token_budget/{tests,simulate}.rs  # monomode 文件留盘引用
```

---

## 13. 落地 PR 范围备忘（实现时用）

### PR-A · P0 双轨拆除（优先）— **已落地 2026-07-20**

1. ~~Worker / dual：`synthesis_output.contract` + `loop_exit` 与 handoff final 对齐~~ → `apply_worker_handoff_loop_exit`（ProseOnly + early-stop）
2. ~~摘 `rag-answer` / `search-answer` mandatory~~ → yaml + assemble `mandatory.synthesis.clear()`
3. ~~Answer：显式 `mandatory.synthesis.clear()`~~ + pure chat 清空 chat mandatory（P0-2）
4. ~~单测~~ mode_assemble + `worker_channel_uses_handoff_prose_only_contract`
5. ~~同步 Option D §5.1 / KD-13~~
6. 附带 P2-1：capability-rag/search 删用户合成 / UUID cite 教程，改为 handoff 终点

### PR-B · P1 叠层（P0 后）— **已落地 2026-07-20**

1. ~~压缩 answer-from-* 行数；follow-brief → product-answer-base~~ → `answer-follow-brief.md` 删除并入；**保留 answer_rule_parts 条件分支（KD-6）**
2. ~~P1-2：Answer pack 不再叠挂完整 chat-base~~ → memory/voice 内嵌 `product-answer-base`；**KD-12 已修订**
3. ~~P1-3：E 合同单源~~ → from-workspace/from-web 删引用格式教学行，chat_exit 为唯一详细合同
4. ~~**同步改** Option D 锁定行与 host 文件名断言测试~~（§5.3/§5.4/§6.1/§8/§9.3/KD-12/§13；host/brain/pipeline 测试断言同步）

### PR-C · P2 文案

capability-rag/search、codegen 去重与删错阶段 cite 教程（可与 A/B 并行，风险低）。

---

## 14. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-20 | 初稿：代码路径 + full_eval 诊断 |
| 2026-07-20 | **复核并入**：① P0-1 扩至 contract/loop_exit/dual ② §3.2 代码硬编码双轨 ③ §4.2 Answer 继承 chat synthesis ④ P1-1 与 KD-6 限定 ⑤ §8 锁定冲突表 ⑥ §9 勾选与落地顺序 ⑦ §13 PR 范围备忘 |
| 2026-07-20 | **PR-A / PR-B 落地**：PR-A（P0 双轨拆除 + P2 正文）与 PR-B（P1 叠层合并：follow-brief 并入 product-answer-base、Answer pack 摘 chat-base 并修订 KD-12、引用合同单源）均完成；§13 备忘更新为落地状态 |
