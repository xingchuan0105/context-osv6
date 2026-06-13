# Brooks-Lint Review

**Mode:** Test Quality Review  
**Scope:** avrag-rs + frontend_next + contracts（全仓库深度审计，第三轮）  
**Health Score:** 82/100  
**Trend:** 62 → 82 (+20) over last 3 runs

Rust `cargo test --no-run -p app` 已恢复通过；前端 Vitest 增至 254 测 / 60 文件。上一轮 Critical（lib 测试编译失败）与多项 Warning（Surface stub、streaming 巨型文件、mock-providers 零引用）已消除或显著缓解。剩余债务集中在 module_surface 假覆盖、Product E2E 慢反馈环，以及少量命名与 E2E soft gate。

---

## Test Suite Map

```
Unit tests:        ~1,140 cases
  Rust inline:     ~1,086 #[test]/#[tokio::test] attrs
  Frontend Vitest:  254 tests / 60 files（本地 ~19s 全绿）
  Python SDK:       2 files
  contracts:        9 tests（chat_json golden）

Integration tests: ~170 cases
  Rust tests/*.rs:  ~84 tests / 36+ files（contract + module_surface）
  Product E2E:      ~87 tests（smoke + integration，单线程）
  transport-http:   chat_stream / runtime_execute / rag_execute_plan contract
  app-bootstrap:    bootstrap_contract（memory seam 验证）

E2E tests:         ~60 cases
  Playwright:       24 spec files（journey / billing / skills）
  llm_real:         #[ignore] nightly（真实 LLM）
  avrag-rs/e2e:     2 visual specs

Ratio:             Unit ~83% : Integration ~13% : E2E ~5%
                   （接近 Google 70:20:10，单元层偏厚，可接受）

Coverage areas:
  强: app-chat agents/loop/skills、rag-core runtime、guardrails、
      ingestion parsers、product_e2e TestContext + streaming_chat、
      workspace-surface.integration（真实 chat/right-rail DOM 联动）、
      workspace-chat-pane.shared-mocks（集中 mock 声明）
  弱: retrieval-data-plane（3 behavioral tests）、
      8 个 crate 仅 module_surface、storage-pg↔ingestion 边界
  盲区: Playwright web-search journey 对 citation 使用条件断言（外部 API 不稳定）
```

---

## Findings

### 🟡 Warning

**Coverage Illusion — 8 个 crate 的 `module_surface.rs` 只验证 lib.rs 无 impl**

Symptom: `common`、`ingestion`、`billing`、`admin`、`search`、`share`、`transport-http`、`storage-pg` 的 `tests/module_surface.rs` 仅断言 `lib.rs` 不含 `pub fn`/`impl`（见 `common/tests/module_surface.rs`）。  
Source: Google — *How Google Tests Software*, Ch. 11; Feathers — legacy code definition  
Consequence: crate 内部逻辑变更时 module_surface 仍绿，覆盖率数字高估真实保护力。  
Remedy: 保留 module_surface 作架构 guard，每 crate 至少补 1 个 behavioral contract test。

**Architecture Mismatch — Product E2E 集成层单线程跑满 45 分钟 CI 预算**

Symptom: [`.github/workflows/integration-e2e.yml`](../../.github/workflows/integration-e2e.yml) `timeout-minutes: 45`，`--test-threads=1`。约 87 用例各 bootstrap Docker PG/Milvus/worker/mock servers。Smoke PR 约 10 分钟。  
Source: Google — 70:20:10 pyramid; Meszaros — *xUnit Test Patterns*, Slow Tests (p. 253)  
Consequence: 开发者本地很少跑完整 integration；反馈环从 Vitest 19s 跳到 10–45 分钟。  
Remedy: 将 SSE event-order 等 protocol 断言下沉到 `transport-http` contract tests；Product E2E 保留需真实 PG/Milvus 的路径。

**Test Obscurity — `workspace-chat-pane.streaming.typewriter.test.tsx` 仍偏大**

Symptom: 上一轮 658 行单体已拆为 `streaming.typewriter`（270 行）、`streaming.search`（224 行）、`streaming.status`（152 行），并通过 `workspace-chat-pane.shared-mocks.ts` 共享 mock 声明。但 typewriter 文件仍覆盖多种 streaming 变体（done-only、long-done、reduce motion 等）。  
Source: Meszaros — *xUnit Test Patterns*, Eager Test (p. 228)  
Consequence: typewriter 相关失败仍需读较长 setup；是 chat-pane 套件中最大的单文件。  
Remedy: 将 done-only / long-done 等场景再拆为独立文件，或提取 `renderStreamingChatPane()` harness 压缩重复 arrange。

---

### 🟢 Suggestion

**Coverage Illusion — Playwright web-search journey 对 citation 使用条件断言**

Symptom: [`workspace-chat.spec.ts`](../../frontend_next/e2e/specs/journey/workspace-chat.spec.ts) 在 web search 场景用 `if (citationCount > 0)` 才断言 citation 按钮可见，注释说明 Brave API 不稳定。结构性断言（消息非空、mode-indicator）仍为 hard assert。  
Source: Google — *How Google Tests Software*, change coverage vs line coverage  
Consequence: citation 回归在无 citation 的 CI run 中可能静默漏过；属有文档化的外部依赖 tradeoff。  
Remedy: nightly/staging 子集对 citation 使用 hard assert；PR smoke 保留当前条件逻辑并记录于 `e2e-gates.md`。

**Test Obscurity — `billing/api.test.ts` 命名缺少 subject**

Symptom: 用例名如 `"throws on server error"`、`"throws on network failure"`，未标明被测函数（describe 块有 `billingApi.getPlans` 但 it 名不含 method）。  
Source: Osherove — *The Art of Unit Testing*, method_scenario_expected naming  
Consequence: 失败日志难以快速定位 API 面。  
Remedy: 改为 `getPlans_throwsOnServerError` 等 subject-first 命名。

**Test Duplication — Surface 测试仍重复 `vi.mock` 声明块**

Symptom: chat-pane 已通过 `workspace-chat-pane.shared-mocks.ts` 集中 mock；但 `workspace-surface.test.tsx` 等仍各自重复 8 行 `vi.mock`（auth/router/client）。`globalThis.__mockProviders` 工厂已存在，声明块未完全抽象。  
Source: Meszaros — Test Code Duplication (p. 213); Hunt & Thomas — DRY  
Consequence: 修改 mock 边界时需批量同步 surface 文件。  
Remedy: 提供 `installWorkspaceSurfaceMocks()` 封装标准 `vi.mock` 声明，与 chat-pane shared-mocks 对齐。

---

## 亮点（可复制模式）

| 文件 | 说明 |
|------|------|
| [`workspace-chat-pane.shared-mocks.ts`](../../frontend_next/tests/workspace/workspace-chat-pane.shared-mocks.ts) | 单文件 `vi.mock` + hoisted harness，7 个 chat-pane 测试共享 |
| [`workspace-surface.integration.test.tsx`](../../frontend_next/tests/workspace/workspace-surface.integration.test.tsx) | 真实 ChatPane + RightRail DOM 联动，citation modal、source 选择 |
| [`admin-surfaces.test.tsx`](../../frontend_next/tests/admin/admin-surfaces.test.tsx) | 主断言以 DOM 可见结果为主（表格数字、按钮 disabled、审核状态文案） |
| [`settings-surface.test.tsx`](../../frontend_next/tests/settings/settings-surface.test.tsx) | tab 导航、保存反馈、主题切换均以 screen 断言为主 |
| [`bootstrap_contract.rs`](../../crates/app-bootstrap/tests/bootstrap_contract.rs) | memory bootstrap seam 验证，无 private adapter 耦合 |
| [`delegate_contract.rs`](../../crates/app/tests/delegate_contract.rs) | 通过 `AppState` 公开 API 测 citation delegate |
| [`streaming_chat.rs`](../../crates/app/tests/product_e2e/integration/streaming_chat.rs) | 文档化覆盖 8 种 SSE event variant |
| [`stream.test.ts`](../../frontend_next/tests/workspace/stream.test.ts) | 只 mock `fetch`，断言 SSE 解析后的完整 event 序列 |

---

## 相对上一轮的变化（Round 2 → Round 3）

| 维度 | Round 2 (62) | Round 3 (82) |
|------|--------------|--------------|
| `cargo test --no-run -p app` | 4× E0603 失败 | **通过**（全 workspace 通过） |
| Frontend Vitest | 232 tests / 57 files | 254 tests / 60 files |
| streaming 测试 | 658 行单文件 | 拆为 3 文件 + shared-mocks |
| Critical findings | 1（lib 编译） | **0** |
| Admin/Settings 断言 | 以 mock 调用为主 | 以 DOM 可见结果为主 |
| mock-providers | 工厂集中，声明仍分散 | chat-pane shared-mocks 落地 |

---

## 验收命令

```bash
# Rust
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

- Round 1（已归档）：[`archive/brooks-test-quality-review-2026-06-12-round1.md`](./archive/brooks-test-quality-review-2026-06-12-round1.md)
- Round 2（已归档）：[`archive/brooks-test-quality-review-2026-06-12-round2.md`](./archive/brooks-test-quality-review-2026-06-12-round2.md)
- [E2E Quality Gates](./e2e-gates.md)
- [Product E2E Plan](./product-e2e-plan.md)
- 历史分数：[`../../.brooks-lint-history.json`](../../.brooks-lint-history.json)

---

## Summary

测试套件健康度显著提升：Rust 默认门禁已恢复，前端 mock 基础设施与测试拆分趋于成熟，Surface 层断言从「验 mock 调用」转向「验用户可见行为」。当前最高优先级是 **module_surface 假覆盖** 与 **Product E2E 45 分钟反馈环**——前者用 behavioral contract 补强，后者继续将 protocol 断言下沉到 `transport-http`。

剩余 Suggestion 级问题（billing 命名、Playwright citation 条件断言、surface mock 声明重复）可在日常改动中顺手消化，不阻塞 refactor 安全网。
