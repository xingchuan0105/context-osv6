# Brooks-Lint Review

**Mode:** Test Quality Review
**Scope:** avrag-rs + frontend_next + contracts + desktop（全仓库深度复测，第六轮；Round 5 stale report 已归档）
**Health Score:** 68/100
**Trend:** Stable at 68 over last 6 runs

Product E2E 的编译回归已经修复，前端 Vitest 也保持全绿；但 PR smoke 脚本的模块清单守卫会在真正跑测试前误报退出，所以托管 smoke 门禁仍然不能保护 PR。

---

## Test Suite Map

```
Unit tests:
  Frontend Vitest:     263 tests / 63 files（实测全绿，26.11s）
  Rust targeted subset: app-admin 10 pass；avrag-billing 29 pass 后在 migration_0037 失败
  Product E2E unit guards: setup / e2e_gate / test_context / mock_routing 已列入 smoke 脚本

Integration tests:
  Product E2E compile: PASS
    cargo test --no-run -p app --test product_e2e --features product-e2e
  Product smoke list:  20 tests discovered under product_e2e::smoke::*
  Product smoke run:   FAIL before execution（script module coverage guard parses no modules）
  Billing sqlx tests:  FAIL locally without live PG（PoolTimedOut）

E2E tests:
  Playwright:          24 spec files（journey / billing / skills / visual；本轮未跑）
  llm_real:            ignored nightly tests（本轮未跑）

Ratio:
  Suite shape remains broadly healthy, but execution reliability is not:
  the highest-value Rust PR smoke path is currently blocked by runner parsing.
```

---

## Findings

### 🔴 Critical

**Coverage Illusion — PR smoke 脚本在清单守卫处失败，真正 smoke 测试没有执行**

Symptom: `cargo test --no-run -p app --test product_e2e --features product-e2e` 已通过；`cargo test --test product_e2e -p app --features product-e2e smoke:: -- --list` 也能列出 20 个 smoke 测试。但实际 `./scripts/run-product-smoke-e2e.sh` 退出在清单守卫：`ERROR: run-product-smoke-e2e.sh lists smoke module 'auth_boundary' but cargo --list found no tests under smoke::auth_boundary::`。根因是 `--list` 输出带完整前缀 `product_e2e::smoke::auth_boundary::...`，脚本的解析只接受行首 `smoke::...`。
Source: Google — *How Google Tests Software*, Ch. 11（change coverage：门禁必须实际执行才有保护）; Meszaros — *xUnit Test Patterns*, Erratic Test（测试运行器与发现机制不一致）
Consequence: `smoke-e2e.yml` 的核心命令当前会红在“模块发现”步骤，PR smoke 无法覆盖 auth/share/chat/search/RAG；文档里“module list single source of truth”的说法会给出错误安全感。
Remedy: 修正 `assert_smoke_module_coverage` 的解析，接受 `product_e2e::smoke::<module>::...` 前缀（或按 `::smoke::` 分割而不是行首匹配）；给脚本解析加一个轻量测试/检查，之后重跑 `./scripts/run-product-smoke-e2e.sh` 并确认至少进入非 RAG smoke 阶段。

---

### 🟡 Warning

**Test Obscurity — Billing migration 测试依赖隐式外部 PG，本地包级测试不可复现**

Symptom: `cargo test -p ingestion -p avrag-worker -p app-admin -p avrag-billing -p avrag-share` 编译完成后，`app-admin` 与大部分 billing 测试通过，但 `crates/billing/tests/test_migration_0037.rs` 的两个 `#[sqlx::test]` 均 `PoolTimedOut`。同目录部分测试注释写着 “CI-only (requires a live PG)”，但这些 `#[sqlx::test]` 没有本地 skip / precheck；相比之下，`storage-pg` 里的 PG 测试会显式检查 `DATABASE_URL`，缺失时直接返回。
Source: Meszaros — *xUnit Test Patterns*, Mystery Guest (p.411); Osherove — *The Art of Unit Testing*, 测试隔离原则
Consequence: 开发者运行 `cargo test -p avrag-billing` 会得到环境型红灯，难以区分真实迁移回归和数据库未启动；这会降低团队运行包级测试的意愿。
Remedy: 二选一：把这些 PG migration 测试移到明确的 CI/DB gate 脚本并在普通包级测试中跳过；或改为显式读取 `DATABASE_URL` / 启动测试 PG，失败时给出可操作的 precheck 信息，而不是等待 pool timeout。

**Test Obscurity — Product E2E mock 层仍是 1,800+ 行 Mystery Guest**

Symptom: `mock_servers.rs`（约 1,180 行）+ `test_context/builder.rs`（约 620 行）承载 mock LLM 路由、RAG 合成、embedding/search 桩逻辑。路由注释和 `mock_routing_tests` 已补强，但 smoke 失败时仍要跨 mock server、builder、HTTP helper 才能理解“为什么返回这个答案”。
Source: Meszaros — *xUnit Test Patterns*, Mystery Guest; General Fixture
Consequence: 这类基础设施改动容易再次出现“编译通过但 smoke runner 不执行/误路由”的问题，新贡献者也很难判断 mock 行为是不是产品行为。
Remedy: 将 `MockLlmRoute` 路由表与 canned responses 拆成小模块，并让 smoke 测试显式声明所需 mock route；保留现有 `mock_routing_tests` 作为拆分后的 contract。

**Architecture Mismatch — 桌面 Rust command handler 仍只有 `cargo check`，没有行为测试**

Symptom: `frontend_next` 已有 `tauri-ipc.test.ts` 6 测和 `transport.test.ts` 4 测，验证前端 IPC 适配层；但 `desktop/src-tauri/src/` 仍没有 Rust 单测，CI 只做 `cargo check --manifest-path desktop/src-tauri/Cargo.toml`。
Source: Google — *How Google Tests Software*, 测试金字塔; Feathers — Seam Model
Consequence: 前端 mock 契约与 Rust command 实现可能漂移，尤其是 body 序列化、错误码映射、stream/cancel 行为；桌面端真实 bug 仍主要靠手测发现。
Remedy: 把 Tauri command handler 中可纯函数化的 body 构造、错误映射、stream event 转换提取出来，加 Rust `#[test]`；再把 CI 从 `cargo check` 提升到 `cargo test`。

---

### 🟢 Suggestion

**Coverage Illusion — Share port contract 已补 token 行为，但公开读/邀请路径仍缺快速单测**

Symptom: `crates/share/tests/storage_port_contract.rs` 现在覆盖 `create_share_token` → `validate_token` 和 unknown token，比上一轮“零行为测试”明显改善。但 fake store 里的 `load_shared_notebook`、`resolve_public_share_chat_context`、`invite_member` 仍返回 `None` / `not implemented`，这些 handler/service 映射主要靠 `smoke::share_boundary` 和前端 mock 间接覆盖；而当前 smoke 脚本又还未能执行。
Source: Feathers — Characterization Tests; Osherove — 测试完整性原则
Consequence: public share payload 映射、公共聊天上下文、invite 权限错误可能等到慢 E2E 或手测才暴露。
Remedy: 在现有 in-memory fake 上补 3–5 个 service-level tests：shared notebook payload mapping、public chat context mapping、owner invite 成功、非 owner invite 被拒。

**Test Obscurity（反馈噪声）— 编译 warning 常驻，遮住真正测试信号**

Symptom: Product E2E 编译和目标 Rust 测试输出仍包含 `ingestion` 10 条 warning、`product_e2e` 2 条 warning、`avrag-worker` 20+ 条 warning。上一轮 Product E2E 编译失败就是混在这些 warning 后面出现的。
Source: McConnell — *Code Complete*, construction cleanliness
Consequence: 测试失败时输出噪声很高，开发者容易跳过关键 error；也让 `-D warnings` 这类更强 CI 门禁暂时无法启用。
Remedy: 先清理本轮新增的 unused/dead warning；确认无噪声后再考虑在 smoke compile 步启用 `RUSTFLAGS=-D warnings`。

---

## Round 5 闭环核对

| Round 5 发现 | 本轮实测状态 |
|---|---|
| 🔴 Product E2E 测试二进制编译失败 | ✅ 已修复；`cargo test --no-run -p app --test product_e2e --features product-e2e` 通过 |
| 🟡 share 模块仅枚举契约 | ✅ 部分修复；新增 in-memory `ShareStorePort` token round-trip，但公开读/邀请仍是缺口 |
| 🟡 Product E2E mock 层 Mystery Guest | ⏳ 仍存在；已有 route doc 与 `mock_routing_tests`，但体量/隐式预条件仍高 |
| 🟡 桌面 Rust command 零单测 | ⏳ 仍存在；前端 IPC 单测通过，Rust handler 仍仅 `cargo check` |
| 🟢 编译 warning 噪声 | ⏳ 仍存在，且本轮 Rust 目标输出更明显 |
| 🟢 storage-local 零 inline 测试 | ⏳ 本轮未复测，低优先级保留 |

---

## 本轮实测记录（2026-06-13）

```bash
cd avrag-rs
cargo test --no-run -p app --test product_e2e --features product-e2e
# PASS, 17.47s；仍有 ingestion/product_e2e warnings

./scripts/run-product-smoke-e2e.sh
# FAIL before smoke execution:
# ERROR: run-product-smoke-e2e.sh lists smoke module 'auth_boundary' but cargo --list found no tests under smoke::auth_boundary::

cargo test --test product_e2e -p app --features product-e2e smoke:: -- --list
# PASS；20 tests listed as product_e2e::smoke::<module>::<test>

cargo test -p ingestion -p avrag-worker -p app-admin -p avrag-billing -p avrag-share
# FAIL in avrag-billing migration_0037: 2 PoolTimedOut failures after app-admin/billing subset passes

cd ../frontend_next && pnpm vitest run
# Test Files 63 passed / Tests 263 passed / Duration 26.11s

docker ps --filter name=avrag-test- --format '{{.Names}}'
# empty after smoke-script trap
```

---

## 验收命令（修复 Critical 后）

```bash
cd avrag-rs

# 1. Smoke runner must reach actual tests
./scripts/run-product-smoke-e2e.sh

# 2. Keep compile gate green
cargo test --no-run -p app --test product_e2e --features product-e2e

# 3. Billing DB tests must be explicitly gated or backed by a live PG
cargo test -p avrag-billing

# 4. Frontend regression baseline
cd ../frontend_next && pnpm vitest run
```

---

## 相关文档

- Round 1–5（已归档）：[`round1`](./archive/brooks-test-quality-review-2026-06-12-round1.md) / [`round2`](./archive/brooks-test-quality-review-2026-06-12-round2.md) / [`round3`](./archive/brooks-test-quality-review-2026-06-12-round3.md) / [`round4`](./archive/brooks-test-quality-review-2026-06-12-round4.md) / [`round5`](./archive/brooks-test-quality-review-2026-06-13-round5.md)
- [E2E Quality Gates](./e2e-gates.md)
- 历史分数：[`../../.brooks-lint-history.json`](../../.brooks-lint-history.json)

---

## Summary

本轮不是“测试覆盖倒退”，而是“门禁执行链还没闭合”：Product E2E 编译恢复了，前端 263 个 Vitest 全绿，share 也补上了第一批 service-level contract；但 PR smoke runner 现在会被自己的模块清单解析误伤，导致最重要的 Rust smoke gate 仍然不可用。

优先级很明确：先修 `run-product-smoke-e2e.sh` 的 `--list` 解析并跑通 smoke；随后处理 billing 的 PG 依赖测试，让本地包级测试不要因为隐式外部状态而红。
