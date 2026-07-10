# E2E / 契约失败修复计划（全量诊断后）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done** — F0–F6 落地；全量 smoke 14 模块绿；诊断 14 失败项已销 |
| 上游诊断 | [`E2E_FULL_TEST_DIAGNOSIS_2026-07-10.md`](./E2E_FULL_TEST_DIAGNOSIS_2026-07-10.md) |
| 原始日志 | `/tmp/context-osv6-test-diag-2026-07-10/` |
| 基线 commit | `d8f0c6c`（H1–H5 hardening Done；本失败**非**该波引入） |
| 上游已关 | 覆盖修复 Done、Hardening Done |
| 门禁 | [`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md) |
| Solo | [`SOLO_DISCIPLINE.md`](./SOLO_DISCIPLINE.md)、根 `AGENTS.md` §7–§8 |

---

## 1. 一句话

**恢复官方 L2 入口可编译可跑，并修 mock Search/RAG 导致的 HTTP 500 连锁；契约测试跟 Product App 表面对齐。**  
不恢复 PR smoke 必跑、不扩 CI、不改 Write∉ToolCatalog、日常仍 L1 only。

---

## 2. 诊断锚点 → 本计划波次

| 诊断 ID | 问题摘要 | 本计划 Wave |
|---------|----------|-------------|
| **C1** | `agent_catalog_contract` → `app::agents::capability` 不存在 | **F0** |
| **C2** | `unified_agent_contract` 旧 path + 缺 `Agent` trait | **F0** |
| **C3** | `chat_stream_contract` 仍调 `AppState::create_workspace` 等 | **F0** |
| **C4** | `run-product-smoke-e2e.sh` `cargo build --tests` 整包失败阻断 smoke | **F0** |
| **R1–R5** | Search/format/concurrent：500 `llm completion failed` | **F1** |
| **R6–R7** | mock RAG synthesis 无 `chunk_id` **panic** → 500 | **F2** |
| **R8** | multitool 期望 `doc_profile`，只有 `dense_retrieval` | **F3** |
| **R9** | memory tool 未进 `tool_results` | **F3** |
| **R10** | quota 期望 429 得 200 | **F4** |
| **E1–E2** | L3 默认端口占用；reuse 后 21/21 绿 | **F5** |

**基线已绿（勿回归）：** L1、FE、write_smoke、guardrails、chat_smoke、边界 smoke、ingestion、rag_fallback、paddle_image、product_e2e units、L3 smoke（reuse）。

---

## 3. 目标与非目标

### 3.1 目标

1. **`bash scripts/test-l2-mechanisms.sh` 可完整跑完**（lib + product smoke runner 不因无关 test binary 编译失败而中断）。  
2. **P0 smoke 模块全绿**：`search_smoke`、`rag_smoke`、`rag_codegen_multitool_smoke`、`memory_multiturn_smoke` 与现有已绿模块。  
3. **L2 integration 失败数从 14 显著下降**（目标 ≤3 或 0；残留须登记）。  
4. **transport-http / app contract 测试可编译且语义对齐 Product Apps（T1）**。  
5. **L3 默认本地入口不因 8081 占用假红**（文档或脚本一层）。

### 3.2 非目标

- 恢复 GitHub PR 必跑 smoke / 扩 CI 剧场  
- Write journey mock 化（H3-C 已决；另开计划）  
- 真 LLM / rag_quality / full JOURNEY  
- 重写 mock_llm_server 架构  
- 在 `AppState` 上**恢复**业务方法以迁就旧测试（**禁止**；只能改测试/走 `*App`）

### 3.3 成功标准（DoD）

| ID | 标准 | 验证 |
|----|------|------|
| **D0** | `cargo test -p app --tests --features product-e2e -- --list` 无编译错误（或 smoke 脚本不再 build 全量 `--tests` 且文档说明） | 命令 exit 0 |
| **D1** | `bash scripts/test-l2-mechanisms.sh` 绿 **或** 等价：`--check-modules` + 全 NON_RAG + RAG_SERIAL smoke 绿 | 脚本 exit 0 |
| **D2** | `smoke::search_smoke` + `smoke::rag_smoke` 绿 | `E2E_MODE=smoke … --test-threads=1` |
| **D3** | `rag_codegen_multitool_smoke` + `memory_multiturn_smoke` 绿 | 同上 |
| **D4** | `cargo test -p transport-http` 绿（至少 `chat_stream_contract` 可编译可跑） | crate tests |
| **D5** | L2 integration：诊断中 14 失败项清零或剩余 ≤3 且每项有「接受/跟进」注记 | `test-l2-integration.sh` 或定向 filter |
| **D6** | L3：默认路径文档或 `PLAYWRIGHT_REUSE_SERVER` 说明；reuse 下 smoke 仍 21 pass | `test-l3-journey.sh` |
| **D7** | 铁律：T1/T2/Solo 不回退 | review |

**最小可关波：** F0 + F1 + F2 → **D0–D2**（L2 入口 + search/rag 主路径）。  
**完整关波：** + F3 + F4 + F5 → D3–D6。

---

## 4. 波次编排（DAG）

```text
F0  编译债 + smoke 入口解耦          ──P0──►  D0, D1(入口可跑)
        │
        ▼
F1  Search / 通用 LLM 500 根因        ──P0──►  R1–R5 类  (spike 可与 F2 并行读码)
        │
        ▼
F2  mock RAG chunk_id / 禁 panic      ──P0──►  D2 (rag_smoke)
        │
        ▼
F3  multitool + memory tool 对齐      ──P1──►  D3
        │
        ├──────────────────────────────────►  最小关波后可选
        ▼
F4  quota 429 语义                     ──P2──►  R10
F5  L3 本地入口 / 文档                 ──P2──►  D6
F6  integration 清扫 + 诊断关单        ──P1──►  D5（可与 F3–F5 交错）
```

**并行规则**

| 规则 | 说明 |
|------|------|
| F0 阻塞官方 L2 脚本 | 先做；可与 F1/F2 **读码 spike** 并行，**合码**建议 F0 先落地 |
| F1 ∥ F2 spike | 不同路径（search tool 轮 vs RAG synthesis）；合码前各跑定向测 |
| F3 依赖 F2 | memory/rag 常共用 mock_rag_state / codegen |
| F4 独立 | 计费边界，勿与 mock panic 混 commit |
| F5 独立 | 纯脚本/文档，可随时 |
| F6 收尾 | 全量 integration 复跑后关单 |

**预估（solo 人日）**

| Wave | 人日 | 风险 |
|------|------|------|
| F0 | 0.5–1 | 低（路径/导入）；C3 中（fixture 重写） |
| F1 | 0.5–1.5 | **中**（需定位 completion 失败真实原因） |
| F2 | 0.5–1 | 中（改 mock 过宽会掩盖真 bug） |
| F3 | 0.5–1 | 中 |
| F4 | 0.25–0.5 | 低–中（产品语义） |
| F5 | 0.1 | 低 |
| F6 | 0.5 | 低（复跑长） |
| **最小 F0–F2** | **~1.5–3.5** | |
| **完整** | **~3–5.5** | |

---

## 5. Wave F0 — 编译债 + smoke 入口（P0）

### 5.1 目标

让 **product smoke 官方入口** 与 **契约测试** 在 Product App 架构下可编译；**禁止** 为绿测把业务方法加回 `AppState`。

### 5.2 任务

| # | 任务 | 文件（预期） | 动作 |
|---|------|--------------|------|
| F0.1 | agent_catalog | `crates/app/tests/agent_catalog_contract.rs` | `agent_tools::capability::{CapabilityRegistry, *_mode_schema}`（对齐已修的 `write_mode_contract`） |
| F0.2 | app dev-dep | `crates/app/Cargo.toml` | 若尚无：`agent-tools` dev-dep（H5 已可能存在） |
| F0.3 | unified_agent | `crates/app/tests/unified_agent_contract.rs` | 修正 `events`/`runtime` 导入到 `agent_loop` / `app_chat` 真路径；`use agent_loop::runtime::Agent`（或当前公开 trait 路径） |
| F0.4 | stream contract | `crates/transport-http/tests/chat_stream_contract.rs` | `state.workspace().…` 或测试用 helper；**禁止** `AppState::create_workspace` |
| F0.5 | smoke 入口解耦 | `avrag-rs/scripts/run-product-smoke-e2e.sh` | `cargo build … --tests` → **仅** `--test product_e2e`（+ 仍需要的 worker bin）；注释说明「勿因无关 contract binary 阻断 smoke」 |
| F0.6 | （可选）CI 注释 | `e2e-gates.md` 一句 | smoke 只依赖 product_e2e binary |

### 5.3 验证

```bash
export CARGO_BUILD_JOBS=2
cd avrag-rs
cargo test -p app --test agent_catalog_contract -- --list
cargo test -p app --test unified_agent_contract -- --list
cargo test -p app --test write_mode_contract -- --list
cargo test -p transport-http --test chat_stream_contract -- --list
./scripts/run-product-smoke-e2e.sh --check-modules
# 入口可启动即可；全绿留给 F1–F3
bash ../scripts/test-l2-mechanisms.sh   # F0 后至少应越过 compile，进 smoke 执行
```

### 5.4 完成定义

- D0；`test-l2-mechanisms.sh` **不再**因 `agent_catalog_contract` 编译死在门口（smoke 执行失败属 F1+）。

### 5.5 风险

| 风险 | 缓解 |
|------|------|
| unified_agent 行为断言过时 | 先 compile 绿，再按失败断言逐个改；不扩测 |
| WorkspaceApp API 与旧 helper 参数不同 | 读 `product_apps/workspace` 与现有 e2e `create_workspace` 对齐 |

---

## 6. Wave F1 — Search / 通用 LLM 500（P0）

### 6.1 目标

消除 **`llm completion failed: Failed to complete chat request` → HTTP 500** 在 search（及同类 chat/format）mock 路径上的主因；degrade 用例恢复 **200 + degrade 语义**（非 500）。

### 6.2 Spike（F1.0，0.25–0.5d，阻塞实现）

**复现：**

```bash
cd avrag-rs
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::search_smoke -- --test-threads=1 --nocapture
# 可选：RUST_LOG=avrag_llm=debug,app_chat=debug
```

**必答：**

1. 失败发生在第几轮 LLM 调用？（tool_call / 合成 / 首轮）  
2. mock 是否收到请求？响应体是否合法 OpenAI JSON？  
3. 客户端错误是 EmptyStream、parse、HTTP、还是 panic in mock？  
4. 与 H1 tool-only 修复是否交互？（search 空 content + tool_calls 应已合法）  

**产出：** 写回本计划 §6.5 根因一行（panic / route 错 / tool 名 / 协议 / 其他）。

### 6.3 实现策略（按 spike 选一，勿全上）

| 路径 | 若根因是… | 动作 |
|------|-----------|------|
| A | mock 对 search tools 响应不合法 | 修 `mock_native_tool_call` / 合成 JSON |
| B | mock panic 被当作连接中断 | 改 panic 为 JSON error 或合法 fallback（与 F2 协同） |
| C | 生产把可恢复错误打成 500 | 在 agent/search 路径对齐 degrade（**谨慎**，要单测锁语义） |
| D | 路由把 search 判成 RAG synthesis | 修 `from_system_prompt` / synthesis 检测顺序 |

### 6.4 验证

```bash
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::search_smoke -- --test-threads=1 --nocapture
E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  failure::search_degrade failure::provider_down -- --test-threads=1 --nocapture
# 回归：write_smoke 仍绿
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::write_smoke -- --test-threads=1 --nocapture
```

### 6.5 Spike 产出（实施时填写）

- [ ] 根因：  
- [ ] 选定路径 A/B/C/D：  
- [ ] 附 1 条日志证据路径：  

### 6.6 完成定义

- search_smoke 绿；至少 2/3 search failure 用例绿（或注明仍红原因）。

---

## 7. Wave F2 — mock RAG `chunk_id` / 禁 panic（P0）

### 7.1 目标

`mock_synthesis_json_rag` **永不 panic**；`rag_smoke` 在 mock 下稳定 200 + citation 契约。

### 7.2 任务

| # | 任务 | 说明 |
|---|------|------|
| F2.1 | 根因 | 对照 `pin_mock_chunk_ids`、codegen observation 格式、transcript 是否含 chunk_id |
| F2.2 | 禁 panic | 无 id 时：优先 `codegen_chunk_id` state → 再 fallback 合法占位 UUID **或** 返回可解析但带 degrade 的 JSON；**禁止** `panic!` 打崩 mock 线程 |
| F2.3 | pin 路径 | 确认 smoke `chat`/`post_rag` 在 doc_scope 非空时仍 pin mock chunk（`http.rs`） |
| F2.4 | 单测 | 若可：mock 路由 unit「空 transcript + pinned id → 合法 synthesis」 |

### 7.3 原则

- Fallback **仅测试 mock**，不得改变生产 citation 硬门语义。  
- 优先 **修数据流（让 transcript 真有 id）**，fallback 作安全网。

### 7.4 验证

```bash
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::rag_smoke -- --test-threads=1 --nocapture
# 回归
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::rag_fallback_smoke smoke::chat_smoke -- --test-threads=1 --nocapture
```

### 7.5 完成定义

- D2（rag 侧）；mock 线程日志无 `mock RAG synthesis could not resolve chunk_id` panic。

---

## 8. Wave F3 — multitool + memory（P1）

### 8.1 目标

- `rag_codegen_multitool_smoke`：出现成功 `doc_profile`（或测试改为锁「当前产品默认路径」并更新 registry note）。  
- `memory_multiturn_smoke`：`conversation_history_load` / `user_profile_load` 出现在 `tool_results`（或 mock 注入路径修复）。

### 8.2 决策门（实施前）

| 选项 | 何时选 |
|------|--------|
| **修 mock/codegen** | 产品仍承诺 multiround profile→chunk；测试契约正确 |
| **收窄测试** | 产品已改为 dense-only 默认；文档/registry 同步 |

默认 **修 mock**（诊断倾向 mock 只吐 dense）。

### 8.3 任务

| # | 任务 | 说明 |
|---|------|------|
| F3.1 | multitool | `mock_rag_codegen` / retrieve 多轮：按 user query 或固定序列发出 profile 再 chunk |
| F3.2 | memory | `try_memory_tool_response` / `skill_request_memory` 与 smoke 开关对齐；确认 tool 名与断言一致 |
| F3.3 | 验证四测 | memory 模块 4 tests + multitool 1 |

### 8.4 验证

```bash
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::rag_codegen_multitool_smoke smoke::memory_multiturn_smoke \
  -- --test-threads=1 --nocapture
```

### 8.5 完成定义

- D3。

---

## 9. Wave F4 — Quota 边界（P2）

### 9.1 目标

`exhausted_quota_blocks_chat_with_quota_exceeded`：耗尽后 **429（或项目约定的 quota 错误码）**，而非 200 正常 answer。

### 9.2 决策门

| 选项 | 含义 |
|------|------|
| **A 修产品** | mock/e2e 路径也应 enforce quota（与生产一致） |
| **B 修测试** | e2e 未真正耗尽；fixture 设错；或 e2e 故意 bypass 需改测预期 |

实施前读 `quota_boundary.rs` + billing/e2e bypass 配置，**二选一写进 commit message**。

### 9.3 验证

```bash
E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  integration::quota_boundary -- --test-threads=1 --nocapture
```

---

## 10. Wave F5 — L3 本地入口（P2）

### 10.1 目标

减少「本机已起 avrag-api/worker → Playwright 假红」。

### 10.2 任务（选最轻）

| 选项 | 动作 |
|------|------|
| **A（推荐）** | `scripts/test-l3-journey.sh` 默认 `export PLAYWRIGHT_REUSE_SERVER="${PLAYWRIGHT_REUSE_SERVER:-1}"`（仅非 CI） |
| **B** | 仅文档：`e2e-gates` / SOLO 写清必须 reuse |
| **C** | 检测 8080/8081 已占用则自动 reuse |

### 10.3 验证

```bash
# 本机已有栈时：
bash scripts/test-l3-journey.sh   # 不应再因 port in use 秒死
# 或显式：
PLAYWRIGHT_REUSE_SERVER=1 bash scripts/test-l3-journey.sh
```

### 10.4 完成定义

- D6；**不**要求本波跑全 journey。

---

## 11. Wave F6 — Integration 清扫 + 关单（P1 收尾）

### 11.1 目标

复跑 `test-l2-integration.sh`，对照诊断 14 失败项销号。

### 11.2 清单（F1–F4 后勾选）

| 测试 | 依赖 Wave | 关 |
|------|-----------|-----|
| search_smoke + search_degrade + search_429 | F1 | [ ] |
| concurrent_rag | F1/F2 | [ ] |
| format_output ×2 | F1 | [ ] |
| rag_smoke | F2 | [ ] |
| rag_codegen_multitool | F3 | [ ] |
| memory_multiturn ×4 | F2/F3 | [ ] |
| quota_boundary | F4 | [ ] |

### 11.3 验证

```bash
export CARGO_BUILD_JOBS=2
bash scripts/test-l2-mechanisms.sh
bash scripts/test-l2-integration.sh
# 更新诊断文档状态 → Closed / residual
```

### 11.4 完成定义

- D1 + D5；诊断文档顶部状态改为 **Remediated** 并链到本计划 commit。

---

## 12. 提交切片（Solo）

```text
test: fix contract imports and smoke build scope (F0)
fix/test(e2e): restore search mock completion path (F1)
test(e2e): mock RAG synthesis without chunk_id panic (F2)
test(e2e): rag multitool and memory tool smoke (F3)
fix/test: quota exhausted returns 429 under e2e (F4)   # 或 test-only
chore(e2e): L3 reuse existing server by default locally (F5)
docs: close full-test diagnosis after F0–F6 (F6)
```

- 本地 `master`；**不**默认 push / 开 PR。  
- 每波只跑**相关**验证；F6 再全量。  
- **禁止** `--no-verify` 掩盖。

---

## 13. 验证矩阵（速查）

| 变更 | 必跑 | 可选 |
|------|------|------|
| F0 | contract `--list`；`--check-modules`；mechanisms 能进 smoke | write_smoke |
| F1 | search_smoke；search failure 子集 | write_output |
| F2 | rag_smoke | rag_fallback |
| F3 | multitool + memory | — |
| F4 | quota_boundary | billing_boundary |
| F5 | L3 smoke short | — |
| F6 | mechanisms + integration | L1 |

日常开发：**仍** `bash scripts/test-l1.sh` only。

---

## 14. 风险与回退

| 风险 | 缓解 |
|------|------|
| F2 fallback 掩盖「检索真没跑」 | 优先修 pin/observation；fallback 打 `eprintln!` 警告 |
| F1 修生产 degrade 过宽 | 仅限已约定可 degrade 错误；加 crate 单测 |
| F0 改 smoke 只 build product_e2e 漏依赖 | worker bin 仍 prebuild；check-modules 仍 list product_e2e |
| 长 integration 占资源 | `CARGO_BUILD_JOBS=2`；串行；勿叠两次 full suite |
| 误把业务方法加回 AppState | **T1 否决**；PR/commit 自检 |

---

## 15. 铁律（勿回退）

| # | 规则 |
|---|------|
| T1 | 不在 `AppState` 恢复/新加业务方法 |
| T2 | Write / `write_refine_*` ∉ ReAct ToolCatalog |
| T3 | Chat/RAG/Search 执行经 `dispatch_tool` |
| Solo | 日常 L1 only；smoke/integration 波次末 |
| CI | 勿擅自恢复 PR smoke 必跑 |
| Mock | 测试 mock **禁止 panic 打崩 handler** 作为控制流 |

---

## 16. 执行检查清单

- [ ] 读诊断 §2 + 本计划 §2–§4  
- [ ] **F0** 编译 + smoke 入口  
- [ ] **F1** spike 填 §6.5 → 修 search 500  
- [ ] **F2** rag chunk_id / 禁 panic  
- [ ] 最小关波：勾 D0–D2  
- [ ] **F3** multitool + memory  
- [ ] **F4** quota（可选）  
- [ ] **F5** L3 入口（可选）  
- [ ] **F6** 全量复跑 + 关诊断文档  
- [ ] 本地 commit；用户要求再 push  

---

## 17. 相关链接

| 文档 | 用途 |
|------|------|
| [全量诊断](./E2E_FULL_TEST_DIAGNOSIS_2026-07-10.md) | 失败清单与日志路径 |
| [Hardening 计划 Done](./E2E_HARDENING_PLAN_2026-07-10.md) | 前序；本失败非其回归 |
| [覆盖修复交接](./E2E_COVERAGE_REMEDIATION_HANDOFF_2026-07-10.md) | Write/guardrails 触点 |
| [e2e-gates](../../avrag-rs/docs/e2e-gates.md) | 金字塔与 smoke 列表 |
| [SOLO](./SOLO_DISCIPLINE.md) | 本地 trunk / 验证分层 |

---

## 18. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：基于全量门禁诊断，编排 F0–F6 修复计划 |
| 2026-07-10 | 实施完成：F0 contracts+smoke build；F1 mock post-tool 非空；F2 RAG 指纹/synthesis 检测；F3 multitool round 计数+memory workspace scope；F4 quota phase+enforcement；F5 L3 reuse；F6 全 smoke + failure + 原 14 失败项复验绿。另 AGENTS/CLAUDE T7 workspace 唯一真相 |
