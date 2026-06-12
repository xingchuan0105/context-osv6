# Brooks-Lint Review

**Mode:** Test Quality Review  
**Scope:** avrag-rs + frontend_next + contracts（全仓库深度审计，第二轮）  
**Health Score:** 62/100  
**Trend:** 52 → 62 (+10) over last 2 runs

相较上一轮，Rust 编译错误从约 68 个降至 4 个，`product_e2e` 已可单独编译；前端 mock 工厂已集中化，Workspace Surface 已去 stub 并新增 integration 用例。但 `app` 库内联测试仍无法编译，阻塞无过滤的 `cargo test -p app`。

---

## Test Suite Map

```
Unit tests:        ~1,099 cases
  Rust inline:     ~1,067 #[test]/#[tokio::test] attrs（app-chat agents 占大头）
  Frontend Vitest:  232 tests / 57 files（本地 ~17s 全绿）
  Python SDK:       2 files
  contracts:        9 tests（chat_json golden）

Integration tests: ~170 cases
  Rust tests/*.rs:  ~84 tests / 36+ files（contract + module_surface）
  Product E2E:      ~87 tests（smoke + integration，单线程）
  transport-http:   chat_stream / runtime_execute / rag_execute_plan contract

E2E tests:         ~60 cases
  Playwright:       24 spec files（journey / billing / skills）
  llm_real:         #[ignore] nightly（真实 LLM）
  avrag-rs/e2e:     2 visual specs

Ratio:             Unit ~83% : Integration ~13% : E2E ~5%
                   （接近 Google 70:20:10，单元层偏厚，可接受）

Coverage areas:
  强: app-chat agents/loop/skills、rag-core runtime、guardrails、
      ingestion parsers、product_e2e TestContext + streaming_chat、
      workspace-surface.integration（真实 chat/right-rail DOM 联动）
  弱: storage-pg↔ingestion 边界、retrieval-data-plane（3 behavioral tests）、
      8 个 crate 仅 module_surface、app lib_impl 内联测试编译失败
  盲区: app-bootstrap adapters 为 private module，lib 测试无法装配 PG adapter；
         Playwright citation 路径仍为 soft gate
```

---

## Findings

### 🔴 Critical

**Coverage Illusion — `app` 库内联测试无法编译，默认 `cargo test -p app` 门禁失效**

Symptom: `cargo test --no-run -p app` 失败，剩余 4 个 `E0603`：`app/src/lib_impl/tests.rs` 引用 `app_bootstrap::adapters::*`，但 `adapters` 为 private module。对比上一轮：错误从约 68 个（`app-bootstrap` 依赖链断裂）降至 4 个；`cargo test --no-run -p app --test product_e2e` 已可成功编译。  
Source: Feathers — *Working Effectively with Legacy Code*, Ch. 1; Google — *How Google Tests Software*, Ch. 11  
Consequence: 开发者运行 `cargo test -p app` 无过滤即失败；`lib_impl/tests.rs` 中 PG adapter 装配测试无法执行，app→app-bootstrap 边界 refactor 缺少 inline 回归网。Product E2E 可编译但无法替代数百个 inline unit tests 的快速反馈。  
Remedy: 在 `app-bootstrap` 公开 re-export adapter 类型（或提供 `test_support` 模块），或把 PG 装配测试迁到 `app-bootstrap/tests/`；验收：`cargo test --no-run -p app` 零错误。

---

### 🟡 Warning

**Coverage Illusion — 8 个 crate 的 `module_surface.rs` 只验证 lib.rs 无 impl**

Symptom: `common`、`ingestion`、`billing`、`admin`、`search`、`share`、`transport-http`、`storage-pg` 的 `tests/module_surface.rs` 仅断言 `lib.rs` 不含 `pub fn`/`impl`（见 `common/tests/module_surface.rs`）。  
Source: Google — *How Google Tests Software*, Ch. 11; Feathers — legacy code definition  
Consequence: crate 内部逻辑变更时 module_surface 仍绿，覆盖率数字高估真实保护力。  
Remedy: 保留 module_surface 作架构 guard，每 crate 至少补 1 个 behavioral contract test。

**Architecture Mismatch — Product E2E 集成层单线程跑满 45 分钟 CI 预算**

Symptom: [`.github/workflows/integration-e2e.yml`](../../.github/workflows/integration-e2e.yml) `timeout-minutes: 45`，`--test-threads=1`。约 87 用例各 bootstrap Docker PG/Milvus/worker/mock servers。Smoke PR 约 10 分钟。  
Source: Google — 70:20:10 pyramid; Meszaros — *xUnit Test Patterns*, Slow Tests (p. 253)  
Consequence: 开发者本地很少跑完整 integration；反馈环从 Vitest 17s 跳到 10–45 分钟。  
Remedy: 将 SSE event-order 等 protocol 断言下沉到 `transport-http` contract tests；Product E2E 保留需真实 PG/Milvus 的路径。

**Test Obscurity — `workspace-chat-pane.streaming.test.tsx` 仍为 Eager Test**

Symptom: 658 行 / 多场景混于单文件（streaming、done-only、long-done、reduce motion 等）。`beforeEach` 经 `createWorkspaceChatPaneMocks()` 重置大量 stream/client mock。上一轮 1237 行单体已拆为 transcript/composer/modes/markdown，但 streaming 仍过大。  
Source: Meszaros — *xUnit Test Patterns*, Eager Test (p. 228); General Fixture (p. 316)  
Consequence: 失败需读大量 setup 才能定位；该文件是 workspace 测试 suite 中最慢的维护热点之一。  
Remedy: 按 SSE 事件类型或用户故事再拆（如 `streaming-events.test.tsx`、`streaming-done.test.tsx`）；提取 `renderChatPane()` harness。

**Mock Abuse — Admin/Settings Surface 测试以 client mock 为主断言**

Symptom: [`admin-surfaces.test.tsx`](../../frontend_next/tests/admin/admin-surfaces.test.tsx) 一次性 mock 14 个 admin client 函数；[`settings-surface.test.tsx`](../../frontend_next/tests/settings/settings-surface.test.tsx) mock 整个 settings client 层。多处主断言为 `toHaveBeenCalledWith`（settings 约 14 处、admin 约 9 处）。  
Source: Osherove — *The Art of Unit Testing*, Mock usage guidelines; Meszaros — Behavior Verification (p. 544)  
Consequence: API client 签名变更时测试大面积红，但 UI 可见行为（表格渲染、错误提示、禁用状态）可能未被覆盖；测试通过不等于用户看到正确界面。  
Remedy: 保留 client mock 作边界，主断言改为 DOM 可见结果（表格行、错误 banner、按钮 disabled）；`toHaveBeenCalled` 仅用于「调用本身即行为」场景。

**Test Brittleness — `app` lib 测试与 `app-bootstrap` 私有模块耦合**

Symptom: `lib_impl/tests.rs:679–691` 直接构造 `app_bootstrap::adapters::Pg*Adapter`，依赖 private module 而非公开 seam。  
Source: Osherove — Test isolation principle; Feathers — *Working Effectively with Legacy Code*, Ch. 4 Seam Model  
Consequence: 将 adapters 设为 private（正确封装）会打断测试；团队可能为通过测试而扩大 public API，或放弃 inline PG 测试。  
Remedy: 通过 `app-bootstrap` 公开 test seam 或 integration test 装配；测试不应依赖 private module 路径。

---

### 🟢 Suggestion

**Coverage Illusion — Playwright citation 路径使用 soft gate**

Symptom: [`workspace-chat.spec.ts`](../../frontend_next/e2e/specs/journey/workspace-chat.spec.ts) 与 [`workspace-upload-rag.spec.ts`](../../frontend_next/e2e/specs/journey/workspace-upload-rag.spec.ts) 使用 `if (await citationButton.count() > 0)`，无 citation 时跳过断言。  
Source: Google — *How Google Tests Software*, change coverage vs line coverage  
Consequence: CI 绿但 citation 回归可能静默漏过；仅 nightly/staging 能捕获。  
Remedy: mock/staging 子集使用 hard assert；soft gate 仅保留在 flaky 环境或 nightly。

**Test Obscurity — `billing/api.test.ts` 命名缺少 subject**

Symptom: 用例名如 `"throws on server error"`、`"throws on network failure"`，未标明被测函数（如 `getUsageWindow`）。  
Source: Osherove — *The Art of Unit Testing*, method_scenario_expected naming  
Consequence: 失败日志难以快速定位 API 面；新成员扫测试名无法建立 mental map。  
Remedy: 改为 `getUsageWindow_throwsOnServerError` 等 subject-first 命名。

**Test Duplication — `vi.hoisted` 块仍在每文件重复 mock 声明**

Symptom: `mock-providers.ts` 已通过 `globalThis.__mockProviders` 集中工厂（[`setup.ts`](../../frontend_next/tests/setup.ts)），但 25+ 测试文件仍各自重复 `vi.mock("next/navigation")` + auth/ui-preferences 块（约 8–15 行/文件）。  
Source: Meszaros — Test Code Duplication (p. 213); Hunt & Thomas — DRY  
Consequence: 修改 mock 边界时需批量同步；遗漏产生 Incomplete Mock。比上一轮「零引用」已改善，但未完全消除。  
Remedy: 提供 `installSurfaceMocks()` helper 封装标准 `vi.mock` 声明；每测试文件 mock 声明减至 ≤3 行。

---

## 亮点（可复制模式）

| 文件 | 说明 |
|------|------|
| [`workspace-surface.integration.test.tsx`](../../frontend_next/tests/workspace/workspace-surface.integration.test.tsx) | 真实 ChatPane + RightRail DOM 联动，citation modal、source 选择、history 切换 |
| [`workspace-surface.harness.tsx`](../../frontend_next/tests/workspace/helpers/workspace-surface.harness.tsx) | 无子组件 stub，直接 render `WorkspaceSurface` |
| [`mock-providers.ts`](../../frontend_next/tests/helpers/mock-providers.ts) | 集中 mock 工厂 + `createWorkspaceSurfaceMocks` 等场景 bundle |
| [`stream.test.ts`](../../frontend_next/tests/workspace/stream.test.ts) | 只 mock `fetch`，断言 SSE 解析后的完整 event 序列 |
| [`delegate_contract.rs`](../../crates/app/tests/delegate_contract.rs) | 通过 `AppState` 公开 API 测 citation delegate |
| [`chat_service_contract.rs`](../../crates/app/tests/chat_service_contract.rs) | port fake（`FakeRagExecutor`），非 mock 调用链 |
| [`streaming_chat.rs`](../../crates/app/tests/product_e2e/integration/streaming_chat.rs) | 文档化覆盖 8 种 SSE event variant |
| [`concurrent_query.rs`](../../crates/app/tests/product_e2e/integration/concurrent_query.rs) | 并发 RAG 查询独立 citation 断言 + chunk_count 前置检查 |

---

## 相对上一轮的变化（Round 1 → Round 2）

| 维度 | Round 1 (52) | Round 2 (62) |
|------|--------------|--------------|
| Rust 编译 | ~68 errors，`app-bootstrap` 依赖链断裂 | 4 errors，`product_e2e` 可编译 |
| mock-providers | 零引用，14 文件各自 hoisted | `globalThis.__mockProviders` 全局注入 |
| Workspace Surface | stub ChatPane/RightRail | 真实子组件 + integration 用例 |
| right-rail 测试 | 797 行单文件 | 拆为 5 个专题文件 |
| chat-pane 测试 | 1237 行单文件 | 拆为 5 文件，streaming 仍 658 行 |

---

## 验收命令

```bash
# Rust（修复 lib 测试后）
cd avrag-rs
cargo test --no-run -p app
cargo test -p app --test delegate_contract -- --nocapture
cargo test -p app --test product_e2e --no-run
cargo test -p app -p transport-http -p retrieval-data-plane

# Product E2E smoke
E2E_MODE=smoke cargo test -p app --test product_e2e smoke:: -- --test-threads=1

# Frontend
cd frontend_next && pnpm vitest run
```

---

## 相关文档

- 上一轮报告（已归档）：[`archive/brooks-test-quality-review-2026-06-12-round1.md`](./archive/brooks-test-quality-review-2026-06-12-round1.md)
- [E2E Quality Gates](./e2e-gates.md)
- [Product E2E Plan](./product-e2e-plan.md)
- 历史分数：[`../../.brooks-lint-history.json`](../../.brooks-lint-history.json)

---

## Summary

本轮最紧迫问题已从「整个 Rust 测试套件失明」收窄为「`app` lib 内联测试因 private adapters 无法编译」——修复公开 seam 或迁移测试即可恢复 `cargo test -p app` 默认门禁。前端债务明显收敛：mock 工厂集中化、Surface destub、right-rail 拆分、integration 用例覆盖 shell→chat→citation 联动。

后续优先：① 修复 4 个 `E0603`；② 继续拆分 `streaming.test.tsx`；③ Admin/Settings Surface 主断言从 mock 调用转向 DOM；④ Playwright citation hard gate。Product E2E 45 分钟预算仍需持续将 protocol 断言下沉到 `transport-http` contract 层。
