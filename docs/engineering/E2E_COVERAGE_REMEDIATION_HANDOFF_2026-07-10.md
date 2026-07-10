# E2E 覆盖修复交接文档（2026-07-10）

| 字段 | 值 |
|------|-----|
| 状态 | **Done** — W0–W3 + W5 已关；W4 取消（D5 由 W3 满足） |
| 分支 | 本地 `master`（solo trunk；**未要求 push**） |
| 范围 | `avrag-rs` product_e2e / mock LLM / llm protocol；`frontend_next` Playwright journey；docs + registry + smoke 脚本 |
| 主方案 | [`E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md`](./E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md) |
| 门禁语义 | [`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md) |
| 全功能矩阵 | [`avrag-rs/docs/full-functional-e2e-guide.md`](../../avrag-rs/docs/full-functional-e2e-guide.md) |
| 金字塔 inventory | [`TEST_PYRAMID_INVENTORY_2026-07-09.md`](./TEST_PYRAMID_INVENTORY_2026-07-09.md) |
| Solo 纪律 | [`SOLO_DISCIPLINE.md`](./SOLO_DISCIPLINE.md)、根目录 `AGENTS.md` §7–§8 |
| 执行方式 | Subagent 分波（W0→W1→W2→W3→W5）；W1 后只读 review **APPROVE_WITH_NOTES** |

---

## 1. 一句话

**审查发现：分层齐全，但 Write 缺 mock/UI 门禁，文档/registry 漂移。**  
本波已把 **Write 补进 L2 mock smoke + L3 Playwright journey**，并补 **Guardrails HTTP 黑盒**、统一层编号与 registry；**不扩 CI、不恢复 PR smoke 必跑**。

---

## 2. 会话成果（相对修前）

| 维度 | 修前 | 修后 |
|------|------|------|
| Chat / RAG / Search | L2+L3 有 | 不变 |
| **Write** | 仅 `write_mode_contract` + `llm_real::write_real` | + **`smoke::write_smoke`**（mock）+ **`workspace-write.spec.ts`** |
| Guardrails | 主要 crate 单测 | + **`smoke::guardrails_smoke`**（HTTP 400 黑盒） |
| 层编号 | 旧 L1–L6 与 `test-l*.sh` 易混 | full-functional §1 + e2e-gates 交叉说明 |
| registry | 缺 `workspace_crud` / CAP-WRITE | 已对齐 smoke 脚本；CAP-WRITE / CAP-GUARD |

### DoD

| ID | 标准 | 结果 |
|----|------|------|
| D1 | Write L2 mock smoke | ✅ |
| D2 | Write Playwright journey | ✅（本地真 LLM ~6.2m） |
| D3 | registry CAP-WRITE + 模块列表 | ✅ |
| D4 | 层映射文档 | ✅ |
| D5 | P1 至少一项 | ✅ Guardrails（W4 Admin/Prefs 未做） |

---

## 3. 提交链（本地，未 push）

```text
3ccdd05 docs: align E2E layer map and registry with pyramid (W0)
0ff0284 test(e2e): mock write routes and write_smoke (W1)
8715909 test(e2e): playwright workspace-write journey (W2)
6734a2f test: guardrails HTTP blackbox (W3)
bb0dac3 docs: close E2E coverage remediation plan (W5)
```

范围：`3ccdd05^..bb0dac3`（5 个提交）。

---

## 4. 关键代码触点

### 4.1 新增 / 主改测试

| 路径 | 作用 |
|------|------|
| `avrag-rs/crates/app/tests/product_e2e/smoke/write_smoke.rs` | mock 下 Write 全链路烟测（`agent_type=write`，activity ≥2 阶段，正文非空） |
| `avrag-rs/crates/app/tests/product_e2e/smoke/guardrails_smoke.rs` | 注入样例 → HTTP 400 `input_guard_blocked` |
| `frontend_next/e2e/specs/journey/workspace-write.spec.ts` | UI 选 write → 等待完成 → 非空回答 + mode 指示 |
| `avrag-rs/crates/app/tests/product_e2e/mock_llm_server.rs` | `WriteSkeletonJson` / `WriteDraftProse` / `WriteRefineFinish`（`write_refine_finish` 一轮结束） |
| `avrag-rs/scripts/run-product-smoke-e2e.sh` | `NON_RAG_MODULES` 含 `write_smoke`、`guardrails_smoke` |

### 4.2 生产路径修复（随 W1，非仅测试）

| 路径 | 说明 |
|------|------|
| `avrag-rs/crates/llm/src/protocols/openai_chat/protocol.rs` | **tool-only** 响应（空 content + `tool_calls`）不再误报 `EmptyStream`。OpenAI 合法形态；WriteRefine 与 mock search tool 轮需要。 |
| product_e2e `WorkspaceResponse` | 字段 `notebook` → **`workspace`**（对齐 API rename 残留） |

### 4.3 文档 / 登记

| 路径 | 说明 |
|------|------|
| `docs/engineering/E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md` | 主计划，状态 **Done** |
| `avrag-rs/docs/e2e-test-registry.yaml` + `generate-e2e-test-registry.py` | CAP-WRITE / CAP-GUARD / smoke lists / Playwright 索引 |
| `avrag-rs/docs/full-functional-e2e-guide.md` | 新旧层映射 |
| `scripts/test-l1.sh`、`SOLO_DISCIPLINE.md`、`TEST_PYRAMID_INVENTORY_…` | 入口注释与资产表 |

---

## 5. Write mock 技术备忘（接盘必读）

```text
run_write_mode
  research  → Search worker + mock search/LLM（可 degrade）
  skeleton  → system 含「大纲编辑」/「只返回 JSON」→ Mock WriteSkeletonJson
  draft     → system 含「中文长文写作助手」→ WriteDraftProse
  refine    → tools 含 write_refine_finish → mock 首轮 tool call 结束
  validate  → 本地指纹（允许 validation_warning）
```

- Write **不进** ReAct `ToolCatalog`（T2）；`write_refine_*` 仅 `write_refine::tool_specs_for_pool`。
- Smoke **不强制** citation / fingerprint band 全过。
- Playwright journey 在本机验证时走了 **真 LLM**（write 不流式 token，进度卡停留到 `done`），超时设为约 **10 分钟**；mock 后端路径可再压时，但当前 journey 默认依赖真实栈配置。

---

## 6. 验证命令（接盘复现）

### 6.1 日常 L1（默认）

```bash
bash scripts/test-l1.sh
# 改 transport-http 时：
# bash scripts/test-l1.sh transport-http
```

### 6.2 波次 / 机制：Write + Guardrails smoke

```bash
export CARGO_BUILD_JOBS=2
cd avrag-rs
./scripts/run-product-smoke-e2e.sh --check-modules   # 须含 write_smoke + guardrails_smoke
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::write_smoke -- --test-threads=1 --nocapture
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::guardrails_smoke -- --test-threads=1 --nocapture
# 或：
# bash ../scripts/test-l2-mechanisms.sh
```

**已知通过（实施时）：**  
- `write_smoke` ~16–17s  
- `guardrails_smoke` ~7s  
- `--check-modules` 16 modules OK  

### 6.3 Write UI journey（慢，真 LLM）

```bash
cd frontend_next
# 需 PG/api/worker/next 就绪；可 PLAYWRIGHT_REUSE_SERVER=1
pnpm exec playwright test e2e/specs/journey/workspace-write.spec.ts --reporter=list
# 或：
# JOURNEY=1 bash ../scripts/test-l3-journey.sh
```

**已知通过（实施时）：** 1 passed ~6.2m（真 LLM）。

### 6.4 协议回归（若再动 openai 协议）

```bash
cd avrag-rs && cargo test -p avrag-llm --lib openai
```

---

## 7. 明确未做 / 残留

| 项 | 说明 | 建议优先级 |
|----|------|------------|
| **W4 Admin / Prefs 薄测** | 计划可选项；D5 已由 guardrails 满足 | 改 Admin/Prefs 时再补 |
| **`agent_type=write_refine` → 4xx** | 文档曾称不可选；当前可能落 chat 而非专用错误（W1 review 注，**既有缝隙**） | 产品修 + 一条 smoke |
| **OpenAI tool-only 单测** | 行为已在 product smoke 间接覆盖；protocol 缺专用 unit | 低：补 `avrag-llm` 单测锁行为 |
| **Journey 默认 mock 后端** | journey 验证用了真 LLM，墙钟长 | 中：对齐 smoke mock 栈减 flaky/成本 |
| **registry 全量 regenerate** | lists 已手改/同步；discovered test id 列表可能不全 | 低：跑 generator 全量 |
| **指南矩阵行仍用旧 L1/L2 用语** | 仅 §1 有显式映射 | 低：按需改行，勿大重写 |
| **CI PR smoke** | 仍为 `workflow_dispatch` only（Solo 有意） | 勿擅自恢复必跑 |
| **push / 远端** | 本波 **未 push** | 用户要备份时再 `git push` |

---

## 8. 铁律（勿回退）

| # | 规则 |
|---|------|
| T1 | 不在 `AppState` 加业务方法 |
| T2 | Write / `write_refine_*` **∉** ReAct ToolCatalog |
| T3 | Chat/RAG/Search 执行经 `dispatch_tool` |
| Solo | 日常 **L1 only**；smoke/journey/真 LLM 波次末或手跑 |
| 层编号 | 指南历史「L1=smoke」≠ `scripts/test-l1.sh`；以 e2e-gates 金字塔 + `test-l*.sh` 为准 |

---

## 9. 接盘检查清单

- [ ] `git log --oneline -6` 见上述 5 个 commit  
- [ ] `run-product-smoke-e2e.sh --check-modules` 绿  
- [ ] 定向跑 `smoke::write_smoke` + `smoke::guardrails_smoke`  
- [ ] （可选）Playwright `workspace-write.spec.ts`  
- [ ] 读本文件 §5–§7 再改 Write / mock / openai 协议  
- [ ] 需要远端备份时再 push；不要默认开 PR CI 剧场  

---

## 10. 相关链接

| 文档 | 用途 |
|------|------|
| [修复计划（Done）](./E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md) | 波次定义、DoD、技术策略 |
| [Hardening 计划（Ready）](./E2E_HARDENING_PLAN_2026-07-10.md) | 审查后：软断言 / 协议 unit / journey mock / 小债（H1–H5） |
| [e2e-gates](../../avrag-rs/docs/e2e-gates.md) | 门禁语义与 smoke 模块列表 |
| [full-functional-e2e-guide](../../avrag-rs/docs/full-functional-e2e-guide.md) | 能力矩阵 + 新旧层映射 |
| [Write 上线计划](../../avrag-rs/docs/plans/2026-07-08-write-mode-launch-plan.md) | 产品文档/UI（与测试门禁部分重叠；UI 模式按钮已存在） |
| [TN 交接](./TN_REMEDIATION_HANDOFF_2026-07-09.md) | 架构/金字塔上游上下文 |

---

## 11. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：W0–W5 实施后交接；本地 commit `3ccdd05`…`bb0dac3` |
