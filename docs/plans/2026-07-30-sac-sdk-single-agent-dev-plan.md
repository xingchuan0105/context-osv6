# SaC SDK + 单 Agent 开发计划

**日期**：2026-07-30  
**状态**：实施中 — WP0–WP6 已落地（2026-07-30）；待 WP7 全量 149 + 可选 orchestrator 物理删除  
**设计锚点**：[`2026-07-30-sac-sdk-single-agent-design.md`](./2026-07-30-sac-sdk-single-agent-design.md)（A1–A8 不可偏离）  
**基线**：全量 149 v2 **PASS 135/149**（`avrag-rs/docs/engineering/2026-07-30-full149-process-budget-handover.md`）  
**策略**：按负责人决策**整体一步到位**——实施有序、每 WP 有单元门禁，但**不**做 W1→W5 独立全量验收；全量 149 放在体系接通后一次跑。

---

## 0. 一句话目标

把「编排器 → worker brief → codegen → handoff → synthesize」砍成 **一个 ReAct loop + 一个 capability 门控的 SaC SDK**；检索只进沙箱，跨 turn 用 filesystem，前端 capabilities 开关不变。

---

## 1. 现状地图（代码真实位置）

### 1.1 目标态 vs 现状

| 设计 | 现状 | 差距 |
|---|---|---|
| **A1** native 无检索 tool | RAG mode `tool_pool: []`（已靠 codegen）；**search** 仍 native `web_search`/`web_fetch`；catalog 仍有 `dense_retrieval` 等 | 删 LLM 面 native 检索；web 进 SDK |
| **A2** 单 agent | `dispatch_agent_mode`：pure chat 直进 loop；**rag/search 永远 `run_orchestrator_*`**（host / V2 brain） | 砍 orchestrator 分层 |
| **A3** mode → SDK 子集 | mode → `tool_pool` + skill clusters；**沙箱原语全集暴露**，无 capability 限 RPC | 加 allowed-methods 门控 |
| **A4** 原语极简 | shim：`dense_search(top_k)` / `lexical_search` / `graph_search` / `chunk_fetch` / `read_lines` / `grep` / doc_* | 去 topk/graph/chunk_fetch/read_lines；无聚合 |
| **A5** graph 绑 lexical | **已有** lexical force-augment + `graph_context` 侧车；同时仍暴露独立 `graph_search` | 保留绑法，删独立原语 |
| **A6** web 在 SDK | web 仅 native skill | 加 `web`/`fetch` bridge |
| **A7** filesystem 跨 turn | 每 block **全新进程**；无 save/load；handoff JSON 传状态 | 持久目录 + save/load |
| **A8** skill < 2000 tok | `codegen/SKILL.md` ≈ **3k tok**，仍写 handoff/SELECTED/graph/read_lines | 重写聚焦组合 |

### 1.2 关键路径（今日）

```
ChatRequest.capabilities [rag|search]
    → resolve_capabilities (CapabilitySet {rag, search})
    → pure chat?  → assemble_mode → ReActLoop (utility tools)
    → else        → run_orchestrator_v1
                      ├─ materialize channels (Rag / Search)
                      ├─ WorkerSession + briefs (codegen or native web)
                      ├─ WorkerHandoff (parse / SELECTED / compile)
                      ├─ EvidenceStore
                      └─ chat_exit: synthesize_handoff → Answer phase ReAct
```

关键代码量（量级）：

| 区域 | 路径 | ~LOC |
|---|---|---|
| Orchestrator 全家桶 | `app-chat/src/orchestrator/*` | ~9k |
| RuntimeBridge | `rag-core/src/runtime/bridge.rs` | ~1.3k |
| Python shim / 沙箱 | `code-interpreter/src/bridge.rs` | ~0.7k |
| Codegen 迭代 | `agent-loop/.../iteration_codegen.rs` | ~0.9k |
| Handoff 编译 | `agent-loop/output_compiler/handoff.rs` | ~0.3k |
| Mode YAML | `modes/{chat,rag,search,orchestrator}.yaml` | 小 |
| Skills | `prompts/clusters/codegen|search/SKILL.md` + orchestrator prompts | 提示词 |

### 1.3 已可复用（不要重造）

- **Codegen 沙箱 + HostBridge RPC**（fd3/fd4）— 产品差异在原语层，执行环境不换。
- **单 block 多 `await`** — 机制已强制「一轮一个 python 块、块内并行」；SKILL 已写。
- **lexical → graph_context 侧车** — A5 现成机制，保留不破坏。
- **CapabilitySet 前端契约** — 仅 `rag` / `search` 布尔（可组合）；**无 table/cross_doc 前端 mode**。
- **cite / alias / bridge capture** — `RuntimeBridge` 已 capture；需在无 worker 路径上继续喂 citation。
- **hard gate / token 预算** — 近期 commit 的 loop_exit / max_tokens，单 agent 仍需要。

### 1.4 设计文档内部歧义（开工前钉死，见 WP0）

| # | 歧义 | 建议默认（写入 WP0 决策表） |
|---|---|---|
| D1 | §2.1 `search(method=…)` vs §2.3/§7 `dense()`/`lexical()` | **采用 `dense(query)` + `lexical(query)` 两个函数**（与现状命名接近、LLM 更不易混 method；合并名可后置） |
| D2 | §3 表仍写 `read` | **删除**；行级只留 `grep(..., context=)` |
| D3 | `table` / `cross_doc` capability | **非前端 mode**；作为 **rag skill 变体 / 参考段**（按问题渐进披露），SDK 子集仍是 rag 集 |
| D4 | 原语「10 个」计数 | 以清单为准：`dense, lexical, grep, web, fetch, doc_profile, doc_summary, history, user_profile` + **`save`/`load`（filesystem，可算基础设施非检索）** |
| D5 | Answer 证据圈选 | 单 agent 后 **无 worker handoff**；保留 **`SELECTED: #n` 或等价 cite 协议** 在同一 loop 末轮输出（产品 cite 不能断） |
| D6 | `dense_retrieval` auto_fallback | **LLM 不可见**；后端默认 topk 仍用于 bridge `dense`；预算触顶无证据时的系统 fallback 可保留为 **host 内部** 调用，不出现在 tool schema |

---

## 2. 目标架构（接通后）

```
前端 capabilities: [] | [rag] | [search] | [rag,search]   （不变）
        │
        ▼
resolve_capabilities → ModeConfig + SdkGate + SkillBody(<2000 tok)
        │
        ▼
单 ReActLoop（与今日 pure-chat 同形态，无 orchestrator）
  loop:
    LLM → 一个 <code language="python"> 块（组合开通原语，可 fan-out）
    沙箱 execute_with_bridge(allowed_methods, session_fs)
    observation: stdout + bridge capture → cite 流水
    LLM 继续 or 直接 prose 答案（含 SELECTED/#alias 若需要）
        │
        ▼
RuntimeBridge → RagRuntime tools / web ports（内部，非 native FC）
```

**删除或退役（产品路径不可达）**：

- `run_orchestrated_turn` / `run_llm_orchestrated_turn` 主路径
- `WorkerSession` / brief / channel materialize
- `WorkerHandoff` / `synthesize_handoff` / multi-format handoff
- native `web_search`/`web_fetch` 对 LLM 的 schema 披露
- SDK：`graph_search`、`chunk_fetch`、`read_lines`、任意 `top_k` 参数

**保留内部实现**：`dense_retrieval` 等 **RagRuntime 工具实现**（bridge 调用），只是 **不再进 ToolCatalog 的 LLM 面**。

---

## 3. 工作包（有序整体，非独立发版阶段）

每个 WP：**改什么 → 关键文件 → 完成定义 → 门禁命令**。  
WP 之间有依赖；可并行处已标。整体合并到 trunk 前跑全量门禁。

```
WP0 决策钉死
  └─► WP1 SDK 原语层 ─────────┬─► WP3 capability 门控 + skills
  └─► WP2 沙箱 FS save/load ──┤
                              └─► WP4 单 agent 主路径（砍编排）
                                    ├─► WP5 native 检索下架（A1）
                                    ├─► WP6 cite/progress/fallback 适配
                                    └─► WP7 锚点 grep + 定向 E2E + 全量 149
```

---

### WP0 — 开工决策（0.5d，无代码）

**输出**：在本文件 §1.4 决策表打勾或修订设计 §2/§3 一行文字（避免实施分叉）。

- [ ] D1 API 形状：`dense`/`lexical`（推荐）或 `search(method=)`
- [ ] D2 去掉 `read`
- [ ] D3 table/cross_doc = rag 内 skill，不扩前端
- [ ] D5 cite 协议（SELECTED 留在单 loop）
- [ ] D6 auto_fallback 仅 host 内部

**门禁**：设计锚点 A1–A8 无新增偏离；若偏离须显式评审。

---

### WP1 — SDK 原语层（A4/A5/A6 核心）

**目标**：shim + host bridge 签名对齐设计；lexical 保留 graph 侧车；web/fetch 进 bridge。

| 动作 | 文件 |
|---|---|
| 重写 Python shim 方法表 | `code-interpreter/src/bridge.rs`（`bridge_shim_source` / `bridge_shim_client_method_names`） |
| 重写 `method_to_tool_call` / `supported_method_names` | `rag-core/src/runtime/bridge.rs` |
| 去掉 top_k 入参；host 用后端默认 topk | 同上 + `DenseRetrievalArgs` 构造处 |
| 删 RPC：`graph_search` / `chunk_fetch` / `read_lines` | 双侧 + 单测 |
| 加 RPC：`web` / `fetch`（调现有 web 端口，不走 LLM FC） | bridge → search/web 执行路径（复用 skill 实现，勿复制业务） |
| 加 RPC：`history` / `user_profile`（若今日在 native utility） | bridge 或 session 注入上下文 |
| 同步对外 Python SDK 文档/client（若 e2e/benchmark 用） | `python/avrag_sdk/` |
| 更新「shim↔host 方法名必须一致」测试 | `rag-core` bridge tests |

**目标 shim 面（推荐命名，D1）**：

```text
dense(query) | lexical(query) | grep(...) | web(query) | fetch(url)
doc_profile(...) | doc_summary(...) | history(...) | user_profile()
save(path, data) | load(path)   # WP2 可同批接线
```

兼容策略（减 code_gen_error 窗口）：

- **短窗**：shim 可对旧名 `dense_search` 做 alias → `dense`（日志 warn），全量 E2E 通过后删除 alias。
- **禁止** alias 回 `graph_search` / `read_lines` / `top_k`（与锚点冲突）。

**完成定义**：

- [ ] host 方法集 == shim 方法集（测试绿）
- [ ] dense/lexical **无** top_k 参数
- [ ] 无 graph 独立 method；lexical 结果仍可带 `graph_context`（现成 augment）
- [ ] web/fetch 可在沙箱内 RPC 成功（unit / 小集成）

**门禁**：

```bash
cargo test -p avrag-code-interpreter --lib
cargo test -p rag-core --lib bridge
# 若 web 接线在 agent-tools：
cargo test -p agent-tools --lib
```

---

### WP2 — 沙箱批量 + filesystem（A7）

**目标**：跨 turn `save`/`load`；明确每 session 持久根目录；**取代 handoff 传中间态**。

| 动作 | 说明 |
|---|---|
| per-session 工作目录 | `CodeInterpreter::execute_with_bridge` 注入可写目录（路径来自 session_id / run_id） |
| `save`/`load` | shim 写磁盘 JSON/text；**仅限 session 根下**，禁止 `..` |
| 跨 block 存活 | 今日每 block 新进程 → **目录在 host 侧持久**，新进程 mount 同一路径即可 |
| 安全 | 已 block `os`/`shutil` 等；save/load 走 bridge RPC 或受限 open 白名单，二选一（推荐 **RPC 更简单可控**） |
| 观测 | load 失败要 stderr 清晰，避免静默空 |

**完成定义**：

- [ ] 同 session 两轮：第一轮 save，第二轮 load 读到相同内容
- [ ] 越权路径被拒
- [ ] 产品路径无 `WorkerHandoff` 依赖中间态（WP4 后验收）

**门禁**：

```bash
cargo test -p avrag-code-interpreter --lib
# 加 integration：两 block 共享 dir
```

---

### WP3 — Capability → SDK 子集 + 提示词（A3/A8）

**目标**：沙箱只暴露开通原语；skill 体量 < 2000 tok；前端 mode 不变。

#### 3.1 SDK 门控

| CapabilitySet | 开通原语 |
|---|---|
| pure chat | `history`, `user_profile`（+ 现有 utility native：calculator/weather 可暂留 native，**不**塞检索） |
| rag | `dense`, `lexical`, `grep`, `doc_profile`, `doc_summary`, `history`, `user_profile`, save/load |
| search | `web`, `fetch`, `dense`（设计表），save/load；**无** grep 除非双开 |
| rag+search | 并集 |

实现要点：

- `RuntimeBridge` / execute 路径传入 `allowed_methods: HashSet`；未开通 → RPC error `"capability_denied"`。
- `assemble_mode` / `ModeConfig` 增加 `sdk_primitives` 字段（或从 CapabilitySet 推导函数，单一真相）。
- `modes/search.yaml`：`tool_pool: []`（web 改 SDK 后）。
- `modes/rag.yaml`：保持空 pool；skill_catalog 仍 mandatory codegen。

#### 3.2 提示词

| 产物 | 要求 |
|---|---|
| `prompts/clusters/codegen/SKILL.md` | **重写 < 2000 tok**：如何用 dense/lexical/grep(+context) 读表；**信 total_hits 禁加工**；无 handoff/graph_search/read_lines 教程 |
| table 素养 | 压缩进主 skill 或极短 reference（仍可 skill_request，但默认不靠大段） |
| search skill | web fan-out + fetch 去噪；无 native tool 点名 |
| orchestrator prompts | **退役主路径引用**（`capability-rag.md` 等改为单 agent system base 或合并进 mode system） |
| chat-base | history/user_profile 用法轻量 |

**完成定义**：

- [ ] `wc`/估算各 capability skill **< 2000 tok**（A8）
- [ ] unit：chat 门控下 `grep` RPC → denied；rag 下 `web` denied（除非 search 开）
- [ ] 前端 capabilities API 契约无破坏

**门禁**：

```bash
cargo test -p app-chat --lib capabilities mode_assemble
cargo test -p agent-loop --lib
# 手工/脚本检查 skill 体积
```

---

### WP4 — 单 Agent 主路径（A2，最大 diff）

**目标**：rag/search **不再进 orchestrator**；一个 ReAct loop 从用户问题到最终答案。

| 动作 | 文件 |
|---|---|
| 改 `dispatch_agent_mode` | `app-chat/src/chat/pipeline_steps.rs`：rag/search 与 pure-chat 同构，走 `run_general_mode` / 等价单 loop，带 assembled mode + SdkGate |
| 退役主路径调用 | `run_orchestrator_v1` / V2 brain **默认关闭**；保留模块可编译但 `#[cfg]` 或 dead 标注，**下一批再物理删除**（降低一次 diff 爆炸）——若编译依赖太重则同批删 |
| Answer 协议 | 取消「retrieve handoff → 二次 synthesize」；`allow_content_early_stop` / prose 直接出答案；保留 evidence hard gate |
| Worker/handoff | 产品路径不再 `parse_worker_handoff`；cite 从 bridge capture + SELECTED 在同 loop 收尾 |
| 预算 | 沿用 token 主预算；去掉 **channel 10 vs brief 12** 双闸（根因见交接文档） |
| 测试 | orchestrator 单测大量会红：改为测「单 loop + codegen」或 mark `#[ignore]` 后删 |

**推荐切流策略（仍属整体交付，非多阶段产品验证）**：

1. 新路径 feature flag 仅本地默认 on：`AGENT_SINGLE_LOOP=1`（或直接替换，solo trunk 可直切）。
2. 先让 rag-only 绿，再 search / dual。
3. orchestrator 代码删除可与路径切换同批或紧随 WP5，避免两套真相长期并存。

**完成定义**：

- [ ] grep 产品路径：无 `WorkerSession` / `synthesize_handoff` 调用
- [ ] 一次用户 turn = 一个 agent run（可多 iteration），无 second-phase Answer agent
- [ ] 流式事件仍可用（progress 可降级为 codegen bridge 进度）

**门禁**：

```bash
cargo test -p app-chat --lib
cargo test -p agent-loop --lib
# 冒烟 product_e2e（非全量）
E2E_MODE=nightly E2E_QUESTIONS="58,88" cargo test -p app --test product_e2e \
  realistic_corpus_full_eval --features product-e2e -- --ignored --test-threads=1 --nocapture
```

---

### WP5 — Native 检索下架（A1）

**目标**：LLM function-calling 面无检索类 tool。

| 动作 | 说明 |
|---|---|
| Catalog / disclosure | `web_search`/`web_fetch`/`dense_retrieval`/`lexical_*` **不对 LLM 披露**（执行体可留） |
| `reject_codegen_method_as_native` | 扩展：旧 SDK 名 + web 名若被当 native 调则拒 |
| search mode | pool 空 + codegen skill 含 web |
| Fallback | `run_fallback` 若仍按 tool 名分支，改为 host 内部 bridge 调，不注入 tool schema |
| 测试清理 | `capability/api`、enforcement、eval routing 中「期望 web_search 为 native」的 case 改写 |

**完成定义**：

```bash
# A1 验收 grep（示例）
rg -n 'tool_pool:.*web_search|tool_pool:.*dense_retrieval' avrag-rs/modes/
rg -n '"dense_retrieval"|"web_search"|"web_fetch"' avrag-rs/modes/
# ToolCatalog 对 LLM 的 schema 列表中无上述 id（按项目实际断言测）
```

**门禁**：`cargo test -p agent-tools --lib` + modes 单测。

---

### WP6 — Cite / Progress / Exit 适配

**目标**：砍编排后引用与停机行为不回退。

| 主题 | 要点 |
|---|---|
| Citations | `bridge_tool_results_to_observation_stdout` + `tool_result_from_code_execution_observation` 继续产出 cite 源；工具名可统一映射为内部 `dense_retrieval` **telemetry** 名（非 LLM tool） |
| SELECTED / alias | 单 loop 末轮仍解析 `#n`；`alias_counter` 从 worker 生命周期改为 **session/run 生命周期** |
| Exit policy | `RAG_ANSWER_CHUNK_TOOLS` 含 bridge 回灌的伪 tool 名；require_evidence 保持 |
| Progress UI | 去掉 multi-brief 叙事；改为「执行代码 / 检索方法 / query」事件（bridge capture 已有 method+query） |
| 记忆 | `history`/`user_profile` 走 SDK 后，chat mode 行为对齐 |

**门禁**：citation 单测 + 1–2 题 E2E 有 cite。

---

### WP7 — 锚点验收 + 全量 149

**顺序**：

1. **静态锚点**（分钟级）

| 锚点 | 检查 |
|---|---|
| A1 | modes + LLM tool schema 无检索 native |
| A2 | 产品路径无 orchestrator dispatch |
| A3 | capability_denied 单测 |
| A4 | shim 签名无 topk/count/dedupe/read_lines |
| A5 | 无 graph method；lexical 集成测 graph_context |
| A6 | 沙箱 web RPC 测 |
| A7 | 无 WorkerHandoff 主路径；save/load 测 |
| A8 | skill tok 估算 < 2000 |

2. **定向 E2E**（设计 §8 对应簇）

| 簇 | 题号建议 | 期望 |
|---|---|---|
| 表格 | q088 | PASS，计数不二次去重 |
| 双文档半载 | q058, 100, 101… | 单 agent fan-out 有双源 cite |
| 过程故障 | 原 handoff 半成品题 | 无 handoff 编译错误类 |

```bash
E2E_MODE=nightly E2E_QUESTIONS="58,88,..." cargo test -p app --test product_e2e \
  realistic_corpus_full_eval --features product-e2e -- --ignored --test-threads=1 --nocapture
```

3. **全量 149**（不灌库，对齐交接）

```bash
E2E_MODE=nightly cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture
```

**业务门槛**：v2 PASS **≥ 135/149**，且 q088 + 跨文档簇不低于现状。

4. **结构后处理**：`graphify update .`（若改了模块边界）；**不** commit `graphify-out/`。

---

## 4. 依赖与并行

| 可并行 | 说明 |
|---|---|
| WP1 ∥ WP2 | 原语与 FS 可两人/两会话并行，合并点在 bridge |
| WP3 skills 文案 ∥ WP1 后半 | 签名表以 WP1 为准，文案可先按 WP0 草案写 |
| WP5 后半 ∥ WP4 | native 下架依赖 search 已能 SDK 调 web |
| **不可并行** | WP4 依赖 WP1–3；WP7 依赖 WP4–6 |

Solo 建议主链：

`WP0 → WP1 → WP2 → WP3 → WP4 → WP5 → WP6 → WP7`

---

## 5. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| Orchestrator 删除面过大（~9k LOC + 测试） | 一次 PR 不可审 | 主路径先切单 loop；orchestrator 标 dead 后第二 commit 物理删除 |
| Skill 过瘦导致 code_gen_error 反升 | PASS 掉 | 原语极简 + 签名表唯一；保留短 alias 窗；盯 q code_gen 簇 |
| web 进沙箱的安全/超时 | 卡住 loop | 复用现有 web client 超时；沙箱仍禁 raw socket |
| cite 断链 | UNGROUNDED 升 | WP6 先于全量；定向题必看 cite |
| table/cross_doc 被做成新 mode | 前端 scope creep | D3：仅 skill，不扩 CapabilitySet |
| 双路径长期并存 | 行为分叉 | 全量通过后删 orchestrator 调用点与 flag |

---

## 6. 明确不做（本波）

- 换 e2b 等新沙箱运行时（除非现有 interpreter 证伪隔离不足）
- 新聚合原语 count/dedupe/extract
- 独立 graph 原语
- 新前端 capability（table/cross_doc 开关）
- notebook/org 回流 API（T7/T8）
- Push/PR/CI 剧场（solo 本地 trunk）

---

## 7. 建议 commit 切片（本地 master）

行为保持、可回滚粒度（名称示意）：

1. `feat(sdk): SaC primitives dense/lexical/grep/web + drop graph/read_lines/topk`
2. `feat(sandbox): session filesystem save/load`
3. `feat(capability): sdk gate by CapabilitySet + slim skills`
4. `feat(chat): single ReAct path for rag/search (bypass orchestrator)`
5. `refactor(tools): delist native retrieval from LLM surface`
6. `fix(cite,progress): single-loop evidence + progress`
7. `test(e2e): full149 gate + anchor checklist`
8. `chore: remove dead orchestrator path`（可紧随 4–7）

每 slice：`cargo test -p <touched>`；slice 4 后起跑定向 E2E。

---

## 8. 验收清单（对照设计 §0 / §7）

- [ ] **A1** native 无检索 tool  
- [ ] **A2** 单 ReAct loop，无 orchestrator/worker/brief/handoff/synthesize 产品路径  
- [ ] **A3** 沙箱按 mode 限原语  
- [ ] **A4** dense/lexical 仅 query；无聚合原语  
- [ ] **A5** 无 graph 独立；lexical 带 graph_context  
- [ ] **A6** web/fetch 在 SDK  
- [ ] **A7** save/load；无 handoff 数据结构主路径  
- [ ] **A8** 各 capability skill < 2000 tok  
- [ ] **业务** 149 PASS ≥ 135，q088 + 跨文档簇回升或持平  

---

## 9. 下一步（立即）

1. 确认 WP0 决策表（D1–D6），尤其 **API 命名** 与 **cite 协议**。  
2. 从 **WP1** 开写（bridge 双侧 + 单测）——最小可运行差异、不碰编排。  
3. WP1–3 绿后 **WP4 切主路径**，当天内接定向 58/88，再排全量 149。

---

*本计划服从设计 8 锚点；实施偏离须提请评审。*
