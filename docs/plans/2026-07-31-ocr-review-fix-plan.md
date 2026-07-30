# OCR 审查修复计划（2026-07-31）

| 项 | 内容 |
|---|---|
| 日期 | 2026-07-31 |
| 状态 | **已完成**（WP-0～WP-3 Option R；WP-4/WP-5 未做） |
| 来源 | `docs/agent/ocr-review-2026-07-31.md` + 人工对照源码元审核 |
| 范围 | 当前 master 工作区未提交变更中的 **Rust/YAML 侧** 清理与小修 |
| 不做 | 按 OCR 原文给 `ALIASED_TOOLS` 强加 `graph_retrieval`；改 SaC 别名语义；扩前端；push/PR |
| 约束 | T5 行为保持切片；prompts-in-md；solo 本地 trunk；验证 `cargo test -p <pkg> --lib` |

---

## 0. 一句话

OCR 31 条里 **没有已证实的生产路由级 HIGH bug**；主债是 **skill-owned stop 下线后的死路径未清理**、**几处测试/标签/文案漂移**、以及 **1–2 处 prompts-in-md 违规**。按 **P0 真小修 → P1 死路径决策清理 → P2 债与 nit** 执行，避免把 telemetry/graph 别名当功能 bug 修歪。

---

## 1. 元审核结论（计划前提）

| OCR 结论 | 元审核 | 计划动作 |
|---|---|---|
| HIGH：`graph_retrieval` 缺于 `ALIASED_TOOLS` | **严重度过高**；SaC 别名仅 dense/lexical/grep；现网 graph 多为 `graph_augment` telemetry | **不**加回别名列表；删死守卫或注释意图 |
| MEDIUM：doc_* 双列表 → 错误码不同 | 双路径均拒绝执行，非放行 | 可选统一 hint；不进 P0 |
| MEDIUM：`rag+search` degraded 漏分支 | 属实，但 degraded 路径整体不可达 | 并入 P1 死路径清理 |
| MEDIUM：`execute_codegen_bridged` 空 allowlist | 零调用者；test/legacy 语义 | P0 删或 `cfg(test)` |
| MEDIUM：reject_sac 内联指令 | prompts-in-md 违规属实；**codegen reject 同罪漏报** | P0 两处一起外置 |
| MEDIUM：exit_policy 两变体死 | 同一设计决策连锁 | P1 一次清完 |
| session_fs poison 修复 diff | **报告 patch 类型错误** | 若修用 `unwrap_or_else`+warn，不抄 OCR |
| 覆盖盲区 prompts/** | 属实 | **本计划范围外**；单列后续 WP |

**死路径全景（P1 一次处理）：**

```text
decide_synthesis_gate  不再返回 RunFallbackThenCheck
        ↓
run_synthesis fallback arm 不可达
        ↓
trigger_auto_fallback* / run_auto_fallback 挂空
        ↓
decide_post_loop 恒 EnterSynthesis
        ↓
DegradedNoEvidence / finish_degraded_no_evidence*
/ degraded_no_evidence_answer（含 rag+search）均 latent
```

与 AGENTS.md「host 不做 evidence hard gate」一致 → **退役清理**或 **显式预留文档**，二选一。

---

## 2. 工作包

### WP-0 — 安全小修与测试收紧（P0，低风险）

**目标**：修已核实的一致性/断言问题，不改产品控制流。

| # | 项 | 文件 | 改动 | 验证 |
|---|---|---|---|---|
| 0.1 | `user_profile` 进度标签统一 | `agent-loop/.../progress/mod.rs`、`labels.rs` | 两边同文案（建议统一为 labels 侧「回忆相关上下文」，或两边都改成产品选定的一条） | `cargo test -p agent-loop --lib progress`（若无则整 lib） |
| 0.2 | codegen skill 断言去掉裸 `"dense"` | `assembler.rs` 测试 | 仅 `client.dense` / `dense(query)`，与 round-1 一致 | `cargo test -p agent-loop --lib assembler` |
| 0.3 | worker exit reason 去掉 `reason.is_some()` | `iteration/tests.rs` | 仅允许 `compile_feedback` \| `direct_content` | `cargo test -p agent-loop --lib worker_handoff` |
| 0.4 | blocks-skipped 死分支 | `iteration/tests.rs` | 对齐 `prompts/loop/blocks-skipped.nudge.md` 实文 | 同上 |
| 0.5 | pipeline 答案断言 | `app-chat/.../pipeline_tests.rs` | `answer.contains("test")`（EchoAgent 回显 query） | `cargo test -p app-chat --lib pipeline` |
| 0.6 | 测试函数改名 | 同上 | `dispatch_phase_loads_capability_manuals_only`（或等价） | 编译通过即可 |

**门禁：** 上述包内测试全绿；无生产行为 diff（仅 UI 文案 0.1 对用户可见进度字）。

**不做：** 改 SELECTED 别名集合；改 exit_policy 语义。

---

### WP-1 — 死代码与危险便利 API（P0）

**目标**：去掉会误导维护者或误用的残留。

| # | 项 | 文件 | 改动 |
|---|---|---|---|
| 1.1 | 删 search guide 死 IO | `app-chat/external_agent_guide.rs` | 去掉 `let _ = load_mode_config("search")` 与 `CapabilityRegistry::standard_cached()`；清无用 import |
| 1.2 | `merge_tool_pool` | `app-chat/mode_assemble.rs` | **删除**（无调用方）；若担心回滚参考可改为 git history，不留 `allow(dead_code)` 空壳 |
| 1.3 | `execute_codegen_bridged` | `agent-loop/.../deps.rs` | **优先删除** pub 便利方法；生产只保留 `execute_codegen_bridged_with_session`。若测试仍要空 allowlist，在测试里显式传 `HashSet::new()` |
| 1.4 | graph_augment 死守卫 | `agent-loop/helpers/selected.rs` | **二选一（推荐 A）**：<br>**A.** 删除 `graph_retrieval`+`graph_augment` 守卫 + 注释写明 SaC 别名仅 dense/lexical/grep/index/grep 工具名，telemetry 本就不进 `ALIASED_TOOLS`<br>**B.** 保留守卫并 `// unreachable unless graph enters ALIASED_TOOLS` — 仅当预期未来加原生 graph 主结果时<br>**禁止** OCR 建议的无条件 `+ "graph_retrieval"` |
| 1.5 |（可选）rag-core 不可达 arm | `rag-core/.../bridge.rs` | 删 `extract_query` 的 web/fetch 与 `tool_result_to_bridge_data` 的 web/history arm，或加 forward-compat 注释。可并入 WP-1 或 WP-3 |

**门禁：**

```bash
cargo test -p agent-loop --lib
cargo test -p app-chat --lib
# 若动 1.5：
cargo test -p avrag-rag-core --lib
```

**风险：** 1.3 若 crate 外仍有调用（元审核时工作区内仅定义）— 删前再 `rg execute_codegen_bridged\(` 全仓确认。

---

### WP-2 — prompts-in-md：SaC/codegen 拒绝文案外置（P0）

**目标**：LLM 可见的 reject hint 不再硬编码在 Rust。

| # | 项 | 说明 |
|---|---|---|
| 2.1 | 新增 loop 资产 | `avrag-rs/prompts/loop/sac-superseded-rejection.tmpl.md`、`codegen-method-as-native-rejection.tmpl.md`（或单文件两段）。体裁：**第三人称环境事实** + 示例形态，避免「禁止/必须」命令腔（与 AGENTS.md voice 一致）。占位符：`{tool}` / `{sac_hint}` / `{method}` |
| 2.2 | 加载 | `prompt_assets` 或 `agent-tools` 内 `include_str!` + 现有 `subst` 风格；**注意** agent-tools 是否已依赖 prompt 路径——若不宜拉 agent-loop，可在 agent-tools 内 `include_str!("../../../prompts/loop/...")` 与 `loop_prompt!` 同构，或抽极薄 shared。选 **最小 diff** 的一种 |
| 2.3 | 接线 | `reject_sac_superseded_native_tool`、`reject_codegen_method_as_native_tool` 只填占位符，不拼中英长句 |
| 2.4 | 测试 | 现有 reject 单测改断言：error code 仍 `sac_sdk_only` / `not_a_native_tool`；hint 含 tool 名与关键 SDK 形态。可选：遍历 `SAC_SUPERSEDED_NATIVE_TOOLS` 参数化（OCR low，顺手可做） |

**门禁：**

```bash
cargo test -p agent-tools --lib
# 若 prompt 加载在 agent-loop：
cargo test -p agent-loop --lib prompt_assets
```

**不做：** 改 skill/capability 长文（属后续 prompt 专项）。

---

### WP-3 — exit_policy / degraded 死路径决策（P1，需显式选型）

**开工前二选一（默认推荐 Option R = Retire）：**

| 选项 | 含义 | 改动面 |
|---|---|---|
| **R — Retire（推荐）** | host 永久不做 no-evidence degraded 硬退 | 删 `RunFallbackThenCheck`、`DegradedNoEvidence`（若再无引用）、`run_synthesis` 对应 arm；`trigger_auto_fallback_and_check_degraded` / `finish_degraded_no_evidence_run` 若仅被该死 arm 调用则删或收成 `run_auto_fallback` 独立入口（若 auto_fallback 预算耗尽仍要跑，保留 **纯 fallback 注入** 不含 degraded 判断）；`degraded_no_evidence_answer` 与 `prompts/loop/degraded-no-evidence-*.md` 若无引用则删或移 `prompts/deprecated/` |
| **K — Keep latent** | 预留未来 soft 回退 | enum 变体保留；`decide_*` 加注释「intentionally never returned」；`degraded_no_evidence_answer` 补 `"rag+search"` 臂与 dual 一致；**不删** call site |

**默认执行 R 的理由：** 与 AGENTS.md 现行 stop 模型一致；K 会长期留不可测分支。

**若选 R 的验收：**

- `rg RunFallbackThenCheck|DegradedNoEvidence|finish_degraded_no_evidence|degraded_no_evidence_answer` 仅剩注释/deprecated 或零命中
- `cargo test -p agent-loop --lib`
- 确认 budget 耗尽后的 **auto_fallback 注入**（若产品仍要）仍有可达入口；若 auto_fallback 也仅挂在死 arm 上，需在 synthesis/budget 路径另接 `run_auto_fallback`（**行为变化** → 单独 commit + 说明）

**若选 K 的验收：**

- `"rag+search"` 进 degraded 路由
- 注释写清不可达原因与重开条件

**门禁后决策记录：** 在本文件或 `exit_policy.rs` 顶注释写「R|K + 日期」。

---

### WP-4 — 债与 nit（P2，可延后）

| # | 项 | 建议 |
|---|---|---|
| 4.1 | `SessionFs` poison | 可选 `unwrap_or_else` + `tracing::warn!`；**禁止** OCR 的 `map_err` 写法 |
| 4.2 | `normalize_key` 规范化 `./` `//` 尾 `/` | 小测覆盖 `save("./a")`/`load("a")` |
| 4.3 | `api.rs` auto_fallback 字段旧注释 | 改成 host-internal 不披露 |
| 4.4 | `load_mode_disclosed_tools` 与 `load_mode_tool_pool` | 合并或一行注释「预留分叉」 |
| 4.5 | `record_extra` 双 Mutex | 单线程 bridge 可不动；以后并发再并锁 |
| 4.6 | O(n²) contains | **不做** |
| 4.7 | `subst` 脆弱 / `loop_prompt!` 相对路径 | **不做**（除非另开 prompt 基建） |
| 4.8 | doc_* 双列表错误码 | 可选：SAC 检查先于 codegen，或从 CODEGEN 列表去掉与 native 同名的三项，使 `doc_profile` 走 `sac_sdk_only` |

---

### WP-5 — 范围外：Prompt 面专项（登记，本计划不执行）

OCR 默认排除 `.md`。对本波 SaC/单 agent 变更，行为源大量在：

- `avrag-rs/prompts/loop/**`
- `avrag-rs/prompts/clusters/**`
- `avrag-rs/prompts/orchestrators/**`

**后续单开计划**，检查项：

1. 是否仍有命令腔（禁止/必须）违反 third-person observation  
2. skill 与「model 决定 DirectAnswer」是否一致  
3. dual `rag+search` 文案是否齐全（contract / degraded / capability）  
4. 与 WP-3 R 退役后是否残留「host 会 degraded」的误导句  

---

## 3. 执行顺序与 commit 切片

```text
WP-0  测试+标签          → 1 commit
WP-1  死代码/便利 API    → 1 commit
WP-2  reject 文案外置    → 1 commit（可与 WP-1 合并若 diff 小）
WP-3  exit_policy 决策   → 1 commit（R 或 K；可能含 auto_fallback 重挂）
WP-4  按需               → 可选 squash 进上列或单独 chore
WP-5  不在本计划开工
```

Solo 纪律：本地 `master` 提交；不 push。每 WP 过门禁再进下一包。

---

## 4. 验证矩阵

| 包 | 命令 |
|---|---|
| WP-0/1/3 | `cargo test -p agent-loop --lib` |
| WP-0/1 | `cargo test -p app-chat --lib` |
| WP-2 | `cargo test -p agent-tools --lib` |
| WP-1.5 | `cargo test -p avrag-rag-core --lib`（crate 名以 workspace 为准） |
| 波次结束（可选） | `bash scripts/test-l1.sh`（勿与其他全量 cargo 叠跑） |

结构若动模块边界：同会话 `graphify update .`（勿提交 `graphify-out/`）。

---

## 5. 明确拒绝的 OCR 动作

| OCR 建议 | 拒绝原因 |
|---|---|
| `ALIASED_TOOLS` + `"graph_retrieval"` 作 HIGH 修复 | 与 SaC 别名设计冲突；telemetry 会被拉进 SELECTED 流 |
| session_fs `map_err` + `into_inner` | 类型错误，不能编译 |
| 把 doc_* 双列表当执行 bug 优先修 | 仅 hint 路径差，不阻塞正确拒绝 |
| 把 `rag+search` degraded 当独立 live bug | 路径已死；并入 WP-3 |
| 本计划内改 54 个 prompt md | 需 WP-5 专项，避免与代码清理混 commit |

---

## 6. 完成定义（DoD）

- [x] WP-0～WP-2 已合入本地 trunk  
- [x] WP-3 已选 **R（Retire）**：删 `RunFallbackThenCheck` / `PostLoopAction` / degraded 收口；`run_auto_fallback` 保留未挂接 + `allow(dead_code)` 注释  
- [x] 未把 `graph_retrieval` 加入 `ALIASED_TOOLS`（删死守卫 + 注释）  
- [x] reject 文案外置 `prompts/loop/*-rejection.tmpl.md`  
- [x] `cargo test -p agent-loop --lib`（267）、`-p agent-tools --lib`（152）、`-p app-chat --lib`（230）绿  
- [x] 本计划状态 **已完成**；未做：WP-4 nit、WP-5 prompt 专项  

### 顺手修复（非 OCR 正文，为过门禁）

- `brain.rs` 断言对齐现行 skill 文案（公网搜索/网页检索）  
- `registry` 测试 `capability-rag.dispatch` 已删 → 改用 `index` 测 inference

---

## 7. 工作量粗估

| 包 | 粗估 |
|---|---|
| WP-0 | 0.5–1 h |
| WP-1 | 0.5–1 h |
| WP-2 | 1–2 h（含路径/crate 边界选择） |
| WP-3-R | 1–3 h（取决于 auto_fallback 是否需重挂） |
| WP-3-K | 0.5 h |
| WP-4 | 0.5–1 h 可选 |

合计（R 路径、含 WP-2）：约 **半天内可完成**；不含 WP-5 prompt 专项。
