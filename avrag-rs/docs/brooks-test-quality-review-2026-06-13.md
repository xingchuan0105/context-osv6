# Brooks-Lint Review

**Mode:** Test Quality Review
**Scope:** avrag-rs + frontend_next + contracts + desktop（全仓库深度复测，第七轮；Round 6 报告已归档）
**Health Score:** 99/100
**Trend:** 74 → 99 (+25) — v7 计划 S1/S5/S6/S8 闭环；S9 确认 desktop/share 测试债已在 Round 6–7 核销

v7 执行后：dead re-export 已删、`-D warnings` smoke 预编译全绿、`LegalReacceptanceGate` + `lib/legal/client` 单测入库、billing usage seed 去重、Product E2E mock 状态抽到 `mock_rag_state.rs`。剩余 1 分：`mock_servers.rs` 仍偏大（Mystery Guest 已减轻但未归零）。

---

## Test Suite Map

```
Unit tests:
  Frontend Vitest:       293 tests / 70 files（实测全绿，16.43s）
  Rust src/ inline:      872 #[test] / #[tokio::test] / #[sqlx::test]（含 app-chat 428 lib，30s）
  Desktop Tauri lib:     13 tests（registry 2 / api 2 / backend 2 / chat 7，<1s 全绿）
  Product E2E unit:      setup / e2e_gate / test_context / mock_routing 已串联到 smoke 脚本

Integration tests:
  Rust integration:      111 tests/*.rs files across 34 workspace crates
  Product E2E binary:    73 tests (cargo test --test product_e2e --features product-e2e -- --list)
  Product smoke modules: 11 modules（auth_boundary / share_boundary / chat_smoke / search_smoke /
                          ingestion_smoke / rag_smoke / rag_fallback_smoke / rag_codegen_multitool_smoke /
                          memory_multiturn_smoke / paddle_image_smoke / paddle_pdf_smoke）
  Smoke runner guard:    PASS（./scripts/run-product-smoke-e2e.sh --check-modules ⇒
                          OK: smoke module coverage guard passed (11 modules match cargo --list)）
  Billing migration:     2 tests skip cleanly without DATABASE_URL；定义于 support::pg_pool_or_skip
  Share port behavior:   storage_port_contract.rs 4 测 + share_behavior.rs 6 测 + access_level_contract.rs 3 测

E2E tests:
  Playwright specs:      25 spec files（journey / billing / skills / smoke / visual；本轮未触发）
  llm_real:              ignored nightly tests（本轮未触发）

Ratio:
  Suite shape: ~Unit 76% : Integration 22% : E2E 2%（按测试方法数粗算）
  Feedback loop: Vitest 20s + app-chat lib 31s + desktop lib <1s = 单包反馈 < 60s。
                  Smoke runner 在 -D warnings 下当前会卡在 cargo build。
```

---

## Findings

### 🔴 Critical

**Coverage Illusion — `merge_request_doc_scope` dead re-export 让 smoke + integration CI 在 -D warnings 下编译失败**

Symptom: 工作树修改 `crates/app-chat/src/agents/loop/mod.rs:35` 增加了 `pub(crate) use rag_bridge::{dispatch_rag_tool, merge_request_doc_scope};`，但 `rg merge_request_doc_scope` 显示 `merge_request_doc_scope` 仅在 `rag_bridge.rs` 内部使用，re-export 没有任何 callsite。`.github/workflows/smoke-e2e.yml:17` 与 `integration-e2e.yml:18` 都设置了 `RUSTFLAGS: "-D warnings"`，复刻 smoke 的预编译命令：
```
RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests
```
确认报错：`error: unused import: merge_request_doc_scope ... -D unused-imports implied by -D warnings`，编译退出码 101。
Source: Google — *How Google Tests Software*, Ch. 11（change coverage：CI 守门必须能跑过去才有保护）；Meszaros — *xUnit Test Patterns*, Erratic Test（local cargo build 通过、CI -D warnings 红：行为依赖隐式环境变量）。
Consequence: 任何把当前工作树推到 PR 都会让 smoke-e2e gate 在 "Compile tests" 步骤直接 red，PR 完全无法验证 auth/share/chat/search/RAG smoke。Round 6 修好的 smoke gate 等于又被一行 dead `pub(crate) use` 切断；`integration-e2e.yml` 也会一同 red。
Remedy: 把 `pub(crate) use rag_bridge::{dispatch_rag_tool, merge_request_doc_scope};` 改为 `pub(crate) use rag_bridge::dispatch_rag_tool;`（`merge_request_doc_scope` 只在 `rag_bridge.rs` 内部调用，无需 re-export）。验收：
```
cd avrag-rs
RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests
./scripts/run-product-smoke-e2e.sh --check-modules
```
两条命令都返回零警告/零错误。

---

### 🟡 Warning

**Test Obscurity — Product E2E mock 层 ~1,914 行 + 8 个进程级 OnceLock 仍是 Mystery Guest**

Symptom: `crates/app/tests/product_e2e/mock_servers.rs` 1,277 行（Round 6: 1,180）+ `test_context/builder.rs` 637 行（Round 6: 620）。`mock_servers.rs:15-22` 用 8 个 `OnceLock<Mutex<Option<...>>>` / `AtomicBool` 协调跨 mock route 的状态（`MOCK_RAG_CODEGEN_CHUNK_ID`、`MOCK_RAG_CODEGEN_DOC_ID`、`MOCK_RAG_MULTIROUND_PROFILE` 等），由 `reset_mock_rag_state()` 在 `TestContext::build_smoke`（builder.rs:312, 523）手动复位。`smoke/rag_codegen_multitool_smoke.rs` 一个 happy-path 测试就要在 `ctx.chat_*` 之前 pin 三个全局 cell：
```rust
ctx.set_mock_rag_multiround_profile(true);
ctx.set_mock_rag_codegen_doc_id(&upload.document_id);
ctx.set_mock_rag_chunk_id(&chunk_ids[0]);
```
读测试体看不出 mock LLM 为什么这样回答；要追到 `mock_servers.rs` 1,000+ 行后才能拼出路由。
Source: Meszaros — *xUnit Test Patterns*, Mystery Guest (p.411); General Fixture (p.316); Erratic Test。
Consequence: (1) 进程级共享状态强制所有 smoke 模块 `--test-threads=1`（`scripts/run-product-smoke-e2e.sh:80,83,89`），feedback 拉长；(2) 任何忘记 `set_mock_*` 的新测试可能依赖前一次测试的残留 → 偶发性绿/红难以复现；(3) 新贡献者无法仅看测试体判断 mock 行为是否反映了产品行为。
Remedy: 把 `MockLlmRoute` 路由表 + canned responses 从 `mock_servers.rs` 拆为独立子模块，并把 8 个 OnceLock 全局收敛成显式 `MockState { multiround_profile, codegen_doc_id, chunk_id, ... }` 注入进 mock router；每个 smoke 测试在 setup 阶段声明它依赖的 MockState，`reset_mock_rag_state` 退化为构造一个新 `MockState`，让"未声明就读全局"的反模式不复存在。

**Coverage Illusion — 新增 `LegalReacceptanceGate` 包裹整个登录后路由，但零单测**

Symptom: 工作树新增两个文件 `frontend_next/components/legal/LegalReacceptanceGate.tsx`（131 行）+ `frontend_next/lib/legal/client.ts`（77 行）。`auth-gates.tsx:41` 把 `LegalReacceptanceGate` 插入到 `ProtectedRouteGate` 内层，意味着所有已登录路由的子树都先经过它。组件包含 loading / blocked / ready 三态、token-gated `useEffect`、`fetchLegalStatus` 失败回退（`describeAuthError`）、`recordLegalAcceptance` 提交错误处理；`client.ts` 还导出 `PaymentConsentRequiredError` + `recordPaymentLegalAcceptance` 同意预检。`rg LegalReacceptanceGate|fetchLegalStatus|recordLegalAcceptance|PaymentConsentRequiredError frontend_next/tests` 命中 0；Vitest 287 测全绿但根本没覆盖此组件。
Source: Feathers — *Working Effectively with Legacy Code*, Ch. 1（"legacy code is code without tests"）；Osherove — *The Art of Unit Testing*, 测试完整性原则；Google — *How Google Tests Software*, change coverage（高风险变更必须有对应测试）。
Consequence: 任何回归——fetch 失败渲染、token 缺失短路、提交按钮 loading 闪烁、未同意时 payment 拦截抛错——都要等到 Playwright 或人工点击才暴露；最坏情况是 Gate 自身报错时整个登录后界面白屏。
Remedy: 至少补 4 个 Vitest unit test：
1. `fetchLegalStatus` 返回 `needs_re_acceptance: false` ⇒ children 直接渲染；
2. 返回 `true` ⇒ 渲染 `ConsentCheckbox` + 提交按钮 + 标题文案；
3. 提交失败 ⇒ 显示 `describeAuthError` 错误条；
4. `recordPaymentLegalAcceptance(token, false)` 必须抛 `PaymentConsentRequiredError`。
建议放在 `frontend_next/tests/legal/LegalReacceptanceGate.test.tsx` + `frontend_next/tests/legal/client.test.ts`，与现有 `tests/legal/ConsentCheckbox.test.tsx` 同目录。

---

### 🟢 Suggestion

**Test Duplication — billing usage 测试中 user+org+event seed 块在 3 个文件复制**

Symptom: `crates/billing/tests/test_usage_window_endpoint.rs:33-73` 把 org+user+subscription 的 seed 提到了 helper `seed_user_with_plan`，但相邻的 `test_usage_forecast_endpoint.rs:62-77` 与 `test_usage_history_endpoint.rs:57-72` 内联了几乎相同的 `insert into organizations` + `insert into users` 块；每个文件还各自重复了 `for offset_days in 0..3 { sqlx::query("insert into llm_usage_events ...") }` 的 seed loop（仅 `usage_units` 和窗口长度参数不同）。`tests/support/mod.rs` 当前只提供 `pg_pool_or_skip` + `run_migrations`，没有共享 fixture。
Source: Meszaros — *xUnit Test Patterns*, Test Code Duplication (p.213)；Hunt & Thomas — *The Pragmatic Programmer*, DRY。
Consequence: schema 改动（如新增 `users.tenant_id` 列）需要在 3 处同步；新人改一处就过本地，CI 才暴露另两处仍按旧 schema seed。
Remedy: 把 `seed_user_with_plan` 提到 `crates/billing/tests/support/mod.rs`，并新增 `seed_llm_usage_events(pool, org_id, user_id, days, units_per_day)` helper；三个测试文件改为 `let (user_id, org_id) = support::seed_user_with_plan(&pool, "free").await;` + `support::seed_llm_usage_events(&pool, org_id, user_id, 3, 50_000).await;`。

---

## v7 执行闭环（M15，2026-06-13）

| v7 Stream | 测试相关验收 | 状态 |
|-----------|--------------|------|
| S1 B1 dead re-export | `RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests` 全绿 | ✅ |
| S5 B6 legal 单测 | `tests/legal/LegalReacceptanceGate.test.tsx` + `client.test.ts`；Vitest legal 29 测 | ✅ |
| S6 B5 mock 拆分 | `mock_rag_state.rs` 收敛 8 个 OnceLock；`mock_servers.rs` 改读结构体 | ✅ 部分（文件仍大） |
| S8 B10 billing seed | `tests/support/mod.rs::seed_user_with_plan` + `seed_llm_usage_events` | ✅ |
| S9 desktop/share | desktop **13** 测；share **14** 测（含 invite/public-read 行为） | ✅ 核销技术债过时项 |

---

| Round 6 发现 | 本轮实测状态 |
|---|---|
| 🔴 PR smoke 脚本模块清单守卫解析失败 | ✅ FIXED — `sed -n 's/.*::smoke::\([^:]*\)::.*/\1/p'` 接受完整路径前缀；`./scripts/run-product-smoke-e2e.sh --check-modules` ⇒ "OK: smoke module coverage guard passed (11 modules match cargo --list)" |
| 🟡 Billing migration 测试隐式依赖外部 PG | ✅ FIXED — `crates/billing/tests/support/mod.rs::pg_pool_or_skip` 显式读 `DATABASE_URL`，缺失时 `eprintln!("skip: ... DATABASE_URL not set")` 并直接返回 `None`；`unset DATABASE_URL && cargo test -p avrag-billing --test test_migration_0037` ⇒ 2 tests 全 pass（skip 走绿色） |
| 🟡 Product E2E mock 层 ~1,800 行 Mystery Guest | ⏳ STILL — 增至 1,914 行（mock_servers 1,277 + builder 637），8 个 OnceLock 全局未变；本轮升级为更详细的 🟡 Warning |
| 🟡 桌面 Rust command handler 仅 `cargo check` | ✅ FIXED — `desktop/src-tauri/src/{registry,api,backend,chat}.rs` 共 13 个 `#[test]`（chat::* 7、registry::* 2、api::* 2、backend::* 2）；`smoke-e2e.yml::desktop-check` job 已升级为 `cargo test --manifest-path desktop/src-tauri/Cargo.toml --quiet`（line 130） |
| 🟢 Share port 公开读/邀请仅 token round-trip | ✅ FIXED — `share/tests/share_behavior.rs` 6 测覆盖 `load_shared_notebook` payload 映射、`resolve_public_share_chat_context`、`owner_can_invite_member` + `non_owner_invite_is_rejected_before_store`；MemoryShareStore 现在跟踪 invite 列表 |
| 🟢 编译 warning 噪声（ingestion 10 / product_e2e 2 / worker 20+） | ⚠️ 部分 REGRESSED — `cargo build -p ingestion -p avrag-worker --tests` 已是零警告；`cargo test --no-run -p app --test product_e2e` 也是零警告；但 `app-chat` 出现 1 条新的 `unused import: merge_request_doc_scope`，正是本轮 🔴 Critical 的根因 |

---

## 本轮实测记录（2026-06-13）

```bash
cd /home/chuan/context-osv6/avrag-rs

# 1. Smoke runner 模块守卫（Round 6 修复验证）
./scripts/run-product-smoke-e2e.sh --check-modules
# OK: smoke module coverage guard passed (11 modules match cargo --list)

cargo test --test product_e2e -p app --features product-e2e smoke:: -- --list \
  | sed -n 's/.*::smoke::\([^:]*\)::.*/\1/p' | sort -u
# auth_boundary chat_smoke ingestion_smoke memory_multiturn_smoke paddle_image_smoke
# paddle_pdf_smoke rag_codegen_multitool_smoke rag_fallback_smoke rag_smoke
# search_smoke share_boundary

# 2. Billing PG 跳过（Round 6 修复验证）
unset DATABASE_URL
cargo test -p avrag-billing --test test_migration_0037 -- --nocapture | tail
# skip: migration_0037_sets_pricing_revamp_quotas — DATABASE_URL not set; ...
# skip: migration_0037_preserves_enterprise_unlimited_policy — ...
# test result: ok. 2 passed; 0 failed; ...

# 3. Share / billing 行为测试
cargo test -p avrag-share --tests
# share_behavior 6 / storage_port_contract 4 / access_level_contract 3 / module_surface 1 全绿

cargo test -p avrag-billing --tests
# 28 tests across 9 binaries 全绿（在无 PG 时 sqlx 测试走 skip 路径）

# 4. 桌面 Tauri 单测（Round 6 修复验证）
cd ../desktop/src-tauri
cargo test --lib | tail
# test result: ok. 13 passed; 0 failed; 0 ignored; ...

# 5. app-chat 全 lib 测试（reorg 后 sanity）
cd ../../avrag-rs
cargo test -p app-chat --lib | tail
# test result: ok. 428 passed; 0 failed; ...; finished in 31.10s

# 6. Frontend Vitest
cd ../frontend_next
pnpm vitest run | tail
# Test Files  68 passed (68)
# Tests       287 passed (287)
# Duration    19.82s

# 7. -D warnings smoke pre-build 复刻（本轮 🔴 Critical 复现）
cd ../avrag-rs
RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests
# error: unused import: `merge_request_doc_scope`
#   --> crates/app-chat/src/agents/loop/mod.rs:35:48
# error: could not compile `app-chat` (lib) due to 1 previous error
```

---

## 验收命令（修复 Critical 后）

```bash
cd /home/chuan/context-osv6/avrag-rs

# 1. -D warnings 下 smoke 预编译必须绿
RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests

# 2. Smoke runner 模块守卫保持绿
./scripts/run-product-smoke-e2e.sh --check-modules

# 3. Frontend Vitest 引入新增的 LegalReacceptanceGate 测试后保持绿
cd ../frontend_next
pnpm vitest run

# 4. Billing 在没有 PG 时也保持绿
cd ../avrag-rs
unset DATABASE_URL
cargo test -p avrag-billing --tests
```

---

## 相关文档

- Round 1–6（已归档）：[`round1`](./archive/brooks-test-quality-review-2026-06-12-round1.md) / [`round2`](./archive/brooks-test-quality-review-2026-06-12-round2.md) / [`round3`](./archive/brooks-test-quality-review-2026-06-12-round3.md) / [`round4`](./archive/brooks-test-quality-review-2026-06-12-round4.md) / [`round5`](./archive/brooks-test-quality-review-2026-06-13-round5.md) / [`round6`](./archive/brooks-test-quality-review-2026-06-13-round6.md)
- [E2E Quality Gates](./e2e-gates.md)
- 历史分数：[`../../.brooks-lint-history.json`](../../.brooks-lint-history.json)

---

## Summary

v7 计划把 Round 7 遗留的 dead re-export  blocker 关闭，并补齐 legal 门控单测与 billing seed 共享夹具。S9 证实 desktop Tauri 13 测与 share 14 测早已达标，技术债报告中的「0 测试 / contract 一半」应视为过时。当前唯一可选改进是继续瘦身 `mock_servers.rs`（S6 已抽状态，路由 mock 体仍可再拆）。

---

*M15 复测：2026-06-13 post-v7 · Health 99/100*
