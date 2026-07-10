# 全量测试诊断报告（2026-07-10）

| 字段 | 值 |
|------|-----|
| 范围 | L1 扩展 + FE + L2 mechanisms/smoke/integration + L3 smoke + 若干 crate 契约 |
| 原则 | **只跑只记，不自动修复** |
| 分支 | 本地 `master` @ `d8f0c6c`（H1–H5 hardening 已合） |
| 原始日志 | `/tmp/context-osv6-test-diag-2026-07-10/`（`summary.txt`、`*.log`、`exit_codes.txt`） |
| 墙钟 | ~12:29–12:58 CST（约 28 分钟有效跑测 + 编译） |
| 修复计划 | [`E2E_TEST_FAILURE_REMEDIATION_PLAN_2026-07-10.md`](./E2E_TEST_FAILURE_REMEDIATION_PLAN_2026-07-10.md)（**Done**） |
| 修复状态 | **Remediated (2026-07-10)** — 原 14 失败项与 compile 债已按 F0–F6 关闭 |

**未跑（成本/范围）：** `JOURNEY=1` 全 journey（含真 LLM write ~6min）、`llm_real` / nightly quality、`test-l3-llm.sh`。

---

## 1. 总览

| 套件 | Exit | 结果 |
|------|------|------|
| **L1**（agent-tools, agent-loop, app-chat, app-bootstrap, avrag-llm, write-core + file-size + FE tsc） | 0 | **PASS** |
| **FE_TSC** | 0 | **PASS** |
| **FE_VITEST** | 0 | **PASS** |
| **L2_MECH**（`test-l2-mechanisms.sh`） | 101 | **FAIL** — lib 绿后 `run-product-smoke-e2e.sh` 因 **编译** 中断 |
| **L2_SMOKE_CHECK**（`--check-modules`） | 0 | **PASS**（16 modules） |
| **L2_SMOKE_NONRAG**（手跑 8 模块） | 1 | **1 模块失败**（search） |
| **L2_SMOKE_RAG**（手跑 6 模块） | 1 | **3 模块失败** |
| **L2_SMOKE_UNITS**（setup/e2e_gate/test_context/mock_routing） | 0 | **PASS** |
| **L2_INTEGRATION**（全 `product_e2e`） | 101 | **76 pass / 14 fail / 24 ignore**（~804s） |
| **L3_SMOKE**（默认 webServer） | 1 | **环境**：8081 端口占用 |
| **L3_SMOKE_REUSE**（`PLAYWRIGHT_REUSE_SERVER=1`） | 0 | **21 passed** |
| **APP_LIB** | 0 | **PASS**（20） |
| **TRANSPORT_HTTP**（含 tests） | 101 | **编译失败**（AppState 业务方法已移除） |
| **AGENT_TOOLS** write_refine | 0 | **PASS**（7） |
| **agent_catalog_contract** | 101 | **编译失败** |
| **unified_agent_contract** | 101 | **编译失败** |
| **write_mode_contract** | 0 | **PASS**（H5 后已修导入） |

### 与 H1–H5 本波关系

| 本波相关 | 状态 |
|----------|------|
| write_smoke（含 write_refine 400） | **PASS** |
| guardrails_smoke | **PASS** |
| chat_smoke / auth / share / workspace / billing | **PASS** |
| mock_routing / avrag-llm openai units（L1 内） | **PASS** |
| write_refine ∉ ToolCatalog | **PASS** |

**结论：** 本波 hardening **未引入** 上述回归；失败集中在 **既有架构漂移（测试仍调旧 AppState API）** 与 **mock RAG/Search 合成路径**。

---

## 2. 问题清单（按根因聚类）

### P1 — 编译债：测试仍引用已删除的 Product App 表面

| ID | 位置 | 现象 | 根因（诊断） |
|----|------|------|--------------|
| **C1** | `app/tests/agent_catalog_contract.rs` | `app::agents::capability` 不存在 | Capability 已迁 `agent_tools::capability`；agents 模块不再 re-export |
| **C2** | `app/tests/unified_agent_contract.rs` | `app::agents::events` / `runtime` 缺失；`UnifiedAgent::run` 需 `use agent_loop::runtime::Agent` | Product App / agent-loop 边界拆分后测试未跟 |
| **C3** | `transport-http/tests/chat_stream_contract.rs` | `AppState::create_workspace` / `create_document_upload` / `put_uploaded_document` / `transition_document_status` 不存在 | 应走 `state.workspace()` Product App；T1 清理后测试未改 |
| **C4** | `run-product-smoke-e2e.sh` | `cargo build -p app --tests` **整包失败** | 任一 integration test binary 编译失败即阻断 smoke 入口（即使 `product_e2e` 单独可编） |

**影响：** 官方 L2 入口 `test-l2-mechanisms.sh` 红；需手跑 `cargo test --test product_e2e` 才能测 smoke。

**建议修复方向（未实施）：** 同 `write_mode_contract` 模式改导入；transport-http 契约改 `WorkspaceApp`；或 smoke 脚本只 build `--test product_e2e` 降低耦合。

---

### P2 — mock LLM / Search 执行 500：`llm completion failed`

| ID | 测试 | 表象 | 线索 |
|----|------|------|------|
| **R1** | `smoke::search_smoke` | HTTP **500** `internal_error` / `llm completion failed` | Search agent 有 guide，但 completion 失败（非 degrade 路径） |
| **R2** | `failure::provider_down::search_429_*` | 期望 200 degrade，得 **500** | 同上：mock/provider 异常未转 degrade |
| **R3** | `failure::search_degrade::*`（empty / timeout） | 期望 200 degrade，得 **500** | 同上 |
| **R4** | `integration::format_output::*`（html / ppt） | 期望 200，得 **500**（chat + format） | 共用 LLM 完成路径失败 |
| **R5** | `integration::concurrent_query::concurrent_rag_*` | 期望 200，得 **500** | RAG 路径 |

**共性：** 响应体常带完整 `agent_operation_guide`，`error=internal_error`，`message` 含 **`llm completion failed: Failed to complete chat request`**。  
说明 **preflight/session 已过**，失败在 **mock LLM 调用或响应解析** 阶段，且 **未降级为 200+degrade_trace**。

**与 H1 无关：** write mock 路由与 tool-only 协议在本跑中 **绿**；问题在 search/RAG synthesis 分支。

---

### P3 — mock RAG synthesis 无法解析 `chunk_id`

| ID | 测试 | 现象 |
|----|------|------|
| **R6** | `smoke::rag_smoke` | mock 线程 panic：`mock_llm_server.rs:453` **`mock RAG synthesis could not resolve chunk_id`**；随后 HTTP 500 |
| **R7** | `memory_multiturn` 首轮 | 同上 panic + HTTP 500 |

**机制：** `mock_synthesis_json_rag` 从 transcript / mock_rag_state 取不到 `chunk_id` 时 **panic**（而非返回可解析 JSON），导致 LLM 客户端失败 → 500。

**可能原因（待查，未证实）：**

1. codegen 检索 observation 未写入 transcript 中 mock 可识别的 `<tool_results>` / `<code_execution_result>` 形态；  
2. `pin_mock_chunk_ids` / `set_mock_rag_codegen_chunk_id` 未在该路径生效；  
3. 系统 prompt 被识别为 synthesis（`internal_answer_v1`）时 transcript 仍是「## 角色 / RAG agent」骨架而无检索结果。

---

### P4 — RAG multitool 行为与断言不匹配

| ID | 测试 | 现象 |
|----|------|------|
| **R8** | `rag_codegen_multitool_smoke` | `expected successful tool result 'doc_profile', got: [("dense_retrieval", Ok)]` |

**诊断：** mock/codegen 路径只产出 `dense_retrieval`，未跑到 `doc_profile`；测试仍期望 multi-tool 序列。可能是 mock codegen 固定只发 dense，或 multi-round 未触发 profile 轮。

---

### P5 — memory multitool 未出现在 tool_results

| ID | 测试 | 现象 |
|----|------|------|
| **R9** | `memory_multiturn_smoke`（3 条，除 first_turn 外） | panic：`conversation_history_load in tool_results` / `user_profile_load in tool_results` |

**诊断：** 请求成功（或至少未以 500 为主因）但 **tool_results 缺 memory 工具**。与 mock 强制 memory skill_request / tool 注入有关；可能 mock 未走到 memory 轮，或 tool 名/状态不匹配。

---

### P6 — Quota 边界期望与实现不一致

| ID | 测试 | 现象 |
|----|------|------|
| **R10** | `integration::quota_boundary::exhausted_quota_blocks_chat_with_quota_exceeded` | 期望 **429**，实际 **HTTP 200** 且正常 chat answer（mock-llm） |

**诊断：** 耗尽配额后仍放行 chat，或测试未真正耗尽/计费未挂钩 mock 路径。属 **产品计费边界 vs 测试假设** 漂移，非 mock panic 类。

---

### P7 — L3 Playwright 环境

| ID | 现象 | 诊断 |
|----|------|------|
| **E1** | 默认 `test-l3-journey.sh`：`http://127.0.0.1:8081/health is already used` | 本机已有 worker/API；`reuseExistingServer` 仅当 `PLAYWRIGHT_REUSE_SERVER=1` |
| **E2** | 同配置下 **21/21 smoke pass** | 功能本身 OK；默认入口对环境敏感 |

---

## 3. product_e2e integration 失败清单（14）

```
failure::provider_down::search_429_returns_degraded_answer
failure::search_degrade::search_empty_results_returns_degraded_answer
failure::search_degrade::search_timeout_returns_degraded_answer
integration::concurrent_query::concurrent_rag_queries_are_safe_on_codegen_bridge
integration::format_output::chat_html_renderer_returns_valid_html
integration::format_output::chat_presentation_html_returns_structured_slides
integration::quota_boundary::exhausted_quota_blocks_chat_with_quota_exceeded
smoke::memory_multiturn_smoke::first_turn_memory_tool_works_with_resolved_session_id
smoke::memory_multiturn_smoke::notebook_scope_conversation_history_load_spans_sessions
smoke::memory_multiturn_smoke::on_demand_conversation_history_load_returns_pg_messages
smoke::memory_multiturn_smoke::on_demand_user_profile_load_returns_profile_shape
smoke::rag_codegen_multitool_smoke::rag_multiround_profile_codegen_doc_profile_then_chunk_fetch
smoke::rag_smoke::rag_document_qa_returns_citation
smoke::search_smoke::open_query_returns_web_citation
```

**通过侧（摘录）：** write_smoke×2、guardrails、chat、auth、share、billing、workspace_crud、ingestion、rag_fallback、paddle_image、多数 integration 非 LLM 路径、76/114 runnable。

---

## 4. Smoke 模块矩阵（手跑）

| 模块 | 结果 |
|------|------|
| chat_smoke | PASS |
| search_smoke | **FAIL** (500 / llm completion) |
| write_smoke | PASS (2) |
| auth_boundary | PASS (6) |
| share_boundary | PASS (5) |
| workspace_crud | PASS |
| billing_boundary | PASS |
| guardrails_smoke | PASS |
| ingestion_smoke | PASS |
| rag_smoke | **FAIL** (chunk_id / 500) |
| rag_fallback_smoke | PASS |
| rag_codegen_multitool_smoke | **FAIL** (缺 doc_profile) |
| memory_multiturn_smoke | **FAIL** (4) |
| paddle_image_smoke | PASS |

---

## 5. 优先级建议（仅诊断，不实施）

| 优先级 | 项 | 理由 |
|--------|-----|------|
| **P0** | C1+C4：修 `agent_catalog_contract` 或 smoke 只 build `product_e2e` | 恢复官方 L2 入口 |
| **P0** | R6：mock synthesis 禁止 panic；无 chunk_id 时 fallback / 明确 4xx | 解除 RAG/memory 500 连锁 |
| **P0** | R1–R3：search 路径 `Failed to complete chat request` 根因（mock tool 轮 vs protocol） | smoke + failure 套件一起绿 |
| **P1** | C3：transport-http contract → WorkspaceApp | 契约层与 T1 对齐 |
| **P1** | C2：unified_agent_contract 导入/Agent trait | 架构契约可编译 |
| **P1** | R8：codegen multitool mock 序列 | 与测试契约对齐或降断言 |
| **P2** | R9：memory tool 注入 | mock skill_request 路径 |
| **P2** | R10：quota 200 vs 429 | 计费 mock 或测试数据 |
| **P2** | E1：L3 脚本默认 `PLAYWRIGHT_REUSE_SERVER=1` 或文档 | 减本地假红 |

**明确非本波回归：** write / guardrails / chat / 边界 smoke / L1 / FE / L3（reuse）均绿。

---

## 6. 复现命令

```bash
# 日志目录
ls /tmp/context-osv6-test-diag-2026-07-10/

# 官方 L2（当前预期红在 compile）
bash scripts/test-l2-mechanisms.sh

# 可跑的 product smoke 子集
cd avrag-rs && E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::write_smoke -- --test-threads=1

# 全 integration
cd avrag-rs && E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  -- --test-threads=1

# L3（本机已有 API/worker）
PLAYWRIGHT_REUSE_SERVER=1 bash scripts/test-l3-journey.sh
```

---

## 7. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：全量门禁串行跑测后诊断；**未改生产/测试代码** |
