# Brooks 四维审计 × Legal P1 Review 合并修复计划 — 2026-06-13（v7）

**目标：** 把 2026-06-13 最新四份 Brooks 报告与 Legal Phase 1 产品链路 review 结论**合并去重**，纠正跨报告漂移，补全 Brooks 未覆盖的 Legal P1 缺口，给出统一的优先级与可并行修复编排。

本计划替代 [`archive/brooks-merged-fix-plan-2026-06-13-v6.md`](./archive/brooks-merged-fix-plan-2026-06-13-v6.md)。v6 罗列的 M0–M14 大多已闭环（见 §1 交叉核销），本轮只处理**真正仍 open 的项** + **Legal P1 接线后新引入/暴露的缺口**。

---

## 0. 输入报告与当前分数

| 维度 | 最新报告 | 当前分 | findings |
|------|----------|--------|----------|
| PR | [`brooks-pr-review-2026-06-13-v6.md`](./brooks-pr-review-2026-06-13-v6.md) | **48** ⚠️过时 | 2 Critical + 4 Warning + 2 Suggestion |
| 架构 | [`brooks-architecture-audit-2026-06-13.md`](./brooks-architecture-audit-2026-06-13.md) | **87** | 0 Critical + 2 Warning + 3 Suggestion |
| 技术债 | [`brooks-tech-debt-assessment-2026-06-13.md`](./brooks-tech-debt-assessment-2026-06-13.md)（v7） | **86** | 0 Critical + 2 Warning + 4 Suggestion |
| 测试 | [`brooks-test-quality-review-2026-06-13.md`](./brooks-test-quality-review-2026-06-13.md)（Round 7） | **74** | 1 Critical + 2 Warning + 1 Suggestion |
| Legal P1 | 本次对话 review 结论 | — | 1 P0 新回归 + 4 P1 + 6 P2 |

**关键判断：PR 48 分已严重过时。** PR v6 的 6 条 findings 里有 5 条已被后续技术债 v7 / 测试 Round 7 核销（见 §1），唯一仍 open 的只剩“变更集分层”。PR 维度若重跑，实际分数应与技术债 / 测试同档（85+）。本计划以技术债 v7 + 测试 Round 7 + Legal review 为当前事实源。

---

## 1. 跨报告交叉核销（合并的第一价值）

### 1.1 PR v6 findings → 已被后续报告核销

| PR v6 finding | 当前状态 | 核销来源 |
|---------------|----------|----------|
| 🔴 smoke runner 清单解析为空 | ✅ 已闭环 | 技术债 v7 §1.2 + 测试 Round 7（`sed -n 's/.*::smoke::...'`，11 模块守卫绿） |
| 🟡 LiteParse PDF 重复 parse 3 次 | ✅ 已闭环 | 技术债 v7 路线图 #2（`ParsedPdfSnapshot` + `run_liteparse_snapshot_blocking`） |
| 🟡 `-D warnings` 噪音（ingestion 10 + e2e 2） | 🟡 收敛为单条 | 现仅剩 `merge_request_doc_scope` 一行（见 B1） |
| 🟡 `EdgeParse`/`Mineru*` 旧命名 | ✅ 已闭环 | 技术债 v7 路线图 #7（`rg` 0 命中） |
| 🟢 `billing/featureFlag.ts` 绕过 transport | ✅ 已闭环 | 技术债 v7 路线图 #6（已改 `request<…>`） |
| 🟢 独立图片 `PaddleOcrImage` 缺 E2E | ✅ 已闭环 | 技术债 v7 路线图 #1（`paddle_image_e2e.rs` + `paddle_image_smoke.rs`） |
| 🔴 变更集分裂（staged/unstaged/untracked） | 🟡 仍 open（缩小） | 见 B4 |

### 1.2 Legal 实现已闭环 Brooks 的 finding

| Brooks open finding | 实际状态 | 证据 |
|----------------------|----------|------|
| 架构 Suggestion + 技术债 Suggestion：**legal 版本号前后端双源、无 CI 守护** | ✅ **已被 Legal P0 闭环** | `scripts/verify-legal-p0.sh` 的 **P0-CON-5** 已跨语言比对 MDX / `versions.ts` / `legal_versions.rs` 三源字面值；本轮已把 `verify-legal-p0.sh` 加进 `license-check.yml` 的 `legal-p0-verify` job。Brooks 两报告的“无单一事实源守护”结论不准确，应核销。长期 typeshare 化仍可作为 P2 优化（见 L 增补外的可选项）。 |

> **结论：** Brooks 架构/技术债关于 legal 版本双源的 Suggestion 可视为已偿还，不再单列扣分项。

### 1.3 跨报告矛盾（需人工核实，勿重复派发）

| 项 | 技术债 v7 口径 | 测试 Round 7 口径 | 处置 |
|----|----------------|--------------------|------|
| desktop Tauri 行为测试 | 🟡 “0 行为测试，CI 仅 `cargo check`” | ✅ FIXED — `registry/api/backend/chat.rs` 共 **13** 个 `#[test]`，`desktop-check` job 已升 `cargo test` | 以测试 Round 7 为准（更晚、有命令实测）。技术债 v7 该 Warning 大概率过时，核实后核销 → 技术债再 +分 |
| share port contract 覆盖 | 🟡 “只 4 测，invite/public-read 缺” | ✅ FIXED — `share_behavior.rs` 6 + `storage_port_contract.rs` 4 + `access_level_contract.rs` 3 = **13** 测全绿 | 技术债 v7 可能只数了 `storage_port_contract.rs`。核实 `share_behavior.rs` 是否已覆盖 invite/public-read 行为；若是则核销 |

---

## 2. 合并后的统一 open 问题清单

去重后共 **19 个独立问题域**：Brooks 侧 10（B*）+ Legal 增补 9（L*）。

| ID | 优先级 | 来源 | 问题 | Stream |
|----|--------|------|------|--------|
| **B1** | **P0** | 测试 Critical / 技术债 Warning | `agents/loop/mod.rs:35` 死 re-export `merge_request_doc_scope` 击穿 `-D warnings`，smoke/integration 预编译红 | S1 |
| **L1** | **P0** | Legal review（新回归） | `pricing-page.spec.ts` 点升级未勾选同意 → 接线 `recordPaymentLegalAcceptance` 后会抛错，billing E2E 将 red | S2 |
| **B4** | **P0** | PR Critical / 技术债 Change Propagation | 工作树 legal 功能 + M9 拆分尾期混在同层未分批入库 | S0 |
| B2 | P1 | 架构 Warning | `avrag-test-kit` 死 workspace 成员（无真实消费者） | S3 |
| B3 | P1 | 架构 Warning | `app-chat → app-bootstrap` 未使用 dev-dep 制造 Cargo 环 | S3 |
| B5 | P1 | 测试 Warning | Product E2E mock 层 ~1914 行 + 8 个进程级 OnceLock（Mystery Guest） | S6 |
| B6 | P1 | 测试 Warning | `LegalReacceptanceGate.tsx`（131 行）+ `lib/legal/client.ts`（77 行）零单测 | S5 |
| L2 | P1 | Legal review | `integration-e2e.yml` 未配 PG，transport-http legal 测试在 integration CI 空跑 | S4 |
| L3 | P1 | Legal review | legal HTTP 测试薄：缺 `needs_re_acceptance:true` / `re_acceptance` context / stale privacy / DB 行断言 / 同事务负向 | S4 |
| L4 | P1 | Legal review | Playwright legal E2E 未跟上 P1：缺 `legal-status` / 重签门控 UI / 支付勾选 UI；版本号硬编码 `"2026-06-13"` 未引 `versions.ts` | S4 |
| B7 | P2 | 架构 Suggestion | `NotebookStore` 在 ports 双路径暴露 | S8 |
| B8 | P2 | 技术债 Domain Distortion | `LegalReacceptanceGate.tsx` 混用 i18n 与硬编码中文 + 内联 style | S7 |
| B9 | P2 | 架构 Suggestion | `docs/README.md` / architecture-baseline §2 文档漂移（仍指 v5 / `app-core` Redis 旧路径） | S8 |
| B10 | P2 | 测试 Suggestion | billing usage seed 块在 3 个测试文件复制 | S8 |
| L5 | P2 | Legal review | `handle_create_checkout` 无服务端二次同意校验，绕过 UI 可直接发起 checkout | S7 |
| L6 | P2 | Legal review | `lib/legal/client.ts` 在 `success===false` 抛普通 `Error`，不走 `ApiError`，`describeAuthError` 对该路径失效 | S7 |
| L7 | P2 | Legal review | `transport-http/src/middleware.rs::normalize_route` 未收录 `/api/auth/legal-acceptance`、`/api/auth/legal-status`，metrics 落通配 bucket | S7 |
| L8 | P2 | Legal review | `RegisterRequest.terms_version/privacy_version` 类型为 `Option` 但 handler 实际必填，API 契约不清 | S7 |
| L9 | P2 | Legal review | 设计文档仍写“33 项 P0”，未记录 verify-legal-p0.sh 已扩到 40 项（含 P1 段） | S7 |

---

## 3. 满分差额账本

> 注：分值为按各报告自身记分口径的**估算**，最终以重跑对应 `/brooks-*` 为准。

### PR 48 → 100（实测应已在 ~85，重跑确认）

| 分值 | 消除项 | Stream |
|------|--------|--------|
| +35 | 重跑刷新：smoke/parse/命名/featureFlag/图片 E2E 五条已闭环（§1.1） | M15 重测 |
| +10 | B4 变更集按主题分层入库 | S0 |
| +5 | B1 dead re-export 清除，`-D warnings` 恢复 | S1 |
| +2 | L1 billing E2E 修复 | S2 |
| **=100** | | |

### 技术债 86 → 100

| 分值 | 消除项 | Stream |
|------|--------|--------|
| +5 | B1 `-D warnings` gate 恢复 | S1 |
| +4 | B4 工作树分层入库 | S0 |
| +3 | §1.3 desktop / share contract 矛盾核实后核销 | S9 |
| +2 | B8 LegalReacceptanceGate i18n 收敛 | S7 |
| **=100** | | |

### 测试 74 → 100

| 分值 | 消除项 | Stream |
|------|--------|--------|
| +12 | B1 dead re-export（Critical，CI 红）清除 | S1 |
| +6 | B6 LegalReacceptanceGate + client.ts 单测 | S5 |
| +4 | L1 billing E2E 修复 + L4 Playwright legal E2E 补齐 | S2 / S4 |
| +3 | L2 integration PG + L3 HTTP 测试补强 | S4 |
| +1 | B5 mock 层拆分 / B10 seed 去重 | S6 / S8 |
| **=100** | | |

### 架构 87 → 100

| 分值 | 消除项 | Stream |
|------|--------|--------|
| +6 | B3 删除 `app-chat → app-bootstrap` dev-dep（消除全图唯一 Cargo 环） | S3 |
| +4 | B2 `avrag-test-kit` 死成员去留 | S3 |
| +2 | B7 `NotebookStore` 单一公共出口 | S8 |
| +1 | B9 README/baseline 文档对齐 | S8 |
| **=100** | | |

---

## 4. 修复编排（Stream，文件互斥可并行）

```mermaid
flowchart TD
  S0["S0 变更集分层 + 入库<br/>串行前置 P0"]
  subgraph W1["Wave 1: CI 红灯（P0）"]
    S1["S1 删 dead re-export（1 行）"]
    S2["S2 billing E2E 修复"]
  end
  subgraph W2["Wave 2: P1 主攻"]
    S3["S3 架构卫生：删 test-kit + dev-dep"]
    S4["S4 Legal 测试补强：integration PG + HTTP + Playwright"]
    S5["S5 LegalReacceptanceGate/client 单测"]
    S6["S6 Product E2E mock 拆分"]
  end
  subgraph W3["Wave 3: P2 收尾"]
    S7["S7 Legal 防御/契约/i18n 收尾"]
    S8["S8 文档/卫生：NotebookStore + README + seed"]
    S9["S9 核实 desktop/share 跨报告矛盾"]
  end
  M15["M15 四维重跑记分"]
  S0 --> W1 --> W2 --> W3 --> M15
```

**并行原则：** S1/S2 在 S0 后并行；W2 四条文件域基本独立（S3 改 `Cargo.toml`、S4 改测试与 CI、S5 改前端测试、S6 改 E2E mock）；W3 可多路并行；M15 串行收尾。

### S0 — 变更集分层入库（串行，P0）= B4

按技术债 v7 §2.3 给出的清单分两个主题提交：

1. **Legal 再确认 PR**：`frontend_next/{components/legal,lib/legal,lib/auth/errors.ts,lib/i18n/messages/auth.ts,app/(app)/upgrade/paywall,app/(marketing)/pricing,components/auth-gates,components/settings/settings-billing-panel}` + `avrag-rs/crates/{app-bootstrap/.../pg_auth_store.rs,app-core/src/{auth_store.rs,lib.rs},transport-http/src/{auth_types.rs,lib_impl/auth/profile.rs,lib_impl/router_core.rs,lib_impl/tests.rs,routes/auth.rs}}` + `.github/workflows/{license-check,smoke-e2e}.yml` + `scripts/verify-legal-p0.sh`
2. **M9 拆分尾期 PR**：`app-chat/src/agents/loop/{mod.rs,iteration/,policy/config/,tests.rs,message_format.rs,rag_bridge.rs}` + `app-chat/src/eval/{mod.rs,llm_judge.rs,runner.rs,evaluator.rs,runner_tests.rs}`

**验收：** `git status --short | grep '^??' | wc -l` → 0；每个 PR 独立可编译。

### S1 — 删除 dead re-export（P0）= B1

`crates/app-chat/src/agents/loop/mod.rs:35` 改为 `pub(crate) use rag_bridge::dispatch_rag_tool;`（`merge_request_doc_scope` 仅 `rag_bridge.rs` 内部使用）。

**验收：** `RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests` 零警告。

### S2 — billing E2E 修复（P0）= L1

`frontend_next/e2e/specs/billing/pricing-page.spec.ts`「点升级 Plus」用例：登录态下先勾选 `ConsentCheckbox`（或 mock `POST /api/auth/legal-acceptance` 返回成功）再点升级；未登录态保持原跳转断言。同时核对 `tests/billing/pricing-page.test.tsx` / `PaywallModal.test.tsx` 是否需同步加“未勾选不能 checkout”断言。

**验收：** `pnpm exec playwright test --project=billing` 绿；`pnpm vitest run tests/billing` 绿。

### S3 — 架构卫生（P1）= B2 + B3

- 删除 `app-chat` 对 `app-bootstrap` 的未使用 dev-dependency（消除全图唯一 Cargo 环）。
- `avrag-test-kit`（`crates/test-kit`）二选一：补真实消费者或从 workspace members 删除。

**验收：** `cargo metadata --no-deps` 无 `app-bootstrap ↔ app-chat` 环；`cargo check --workspace` 绿。

### S4 — Legal 测试补强（P1）= L2 + L3 + L4

1. **L2**：`integration-e2e.yml` 比照本轮 `smoke-e2e.yml`，在 `cargo test -p transport-http` 前起 Postgres 并设 `DATABASE_URL`。
2. **L3**：`transport-http/src/lib_impl/tests.rs` 补：`needs_re_acceptance:true`（落旧版本后查 status）、`re_acceptance` context 201、stale `privacy_version` 400、payment 后查 `legal_acceptances` 行断言、注册同事务负向。
3. **L4**：`e2e/specs/smoke/legal-consent.spec.ts` 补 `GET /legal-status`、`re_acceptance` 成功、重签门控 UI、支付勾选 UI；版本号改 `import { PUBLISHED_*_VERSION } from "@/lib/legal/versions"`。

**验收：** 有 PG 时 `cargo test -p transport-http` 实跑断言；`legal-consent.spec.ts` 绿。

### S5 — Legal 前端单测（P1）= B6

新建 `tests/legal/LegalReacceptanceGate.test.tsx` + `tests/legal/client.test.ts`，覆盖测试 Round 7 建议的 4 例：status=false 直渲染 children、status=true 渲染 ConsentCheckbox+提交、提交失败显示 `describeAuthError`、`recordPaymentLegalAcceptance(token,false)` 抛 `PaymentConsentRequiredError`。

**验收：** `pnpm vitest run tests/legal` 覆盖新组件，绿。

### S6 — Product E2E mock 拆分（P1）= B5

把 `mock_servers.rs` 路由表 + canned responses 拆为子模块，8 个 OnceLock 收敛成显式注入的 `MockState`，`reset_mock_rag_state` 退化为构造新 `MockState`。

**验收：** smoke 全绿；`reset_mock_rag_state` 不再依赖进程级全局残留。

### S7 — Legal 防御/契约/i18n 收尾（P2）= B8 + L5 + L6 + L7 + L8 + L9

- **L5**：`handle_create_checkout` 在发起 checkout 前校验用户最新 `payment`/同意状态（或文档明确“同意以前端为准 + 注册已落库”的安全边界）。
- **L6**：`lib/legal/client.ts` 失败路径走 `ApiError`，使 `describeAuthError` 生效。
- **L7**：`transport-http/src/middleware.rs::normalize_route` 增加 `/api/auth/legal-acceptance`、`/api/auth/legal-status`。
- **L8**：`RegisterRequest` 版本字段在 OpenAPI/注释标 required（保持 JSON 兼容）。
- **B8**：`LegalReacceptanceGate.tsx` 硬编码中文落 `messages/auth.ts`（或新建 `messages/legal.ts`），`PaymentConsentRequiredError` 改为外部注入文案。
- **L9**：设计文档更新“33 项 → 40 项”并补 P1 验收段说明。

### S8 — 文档/卫生（P2）= B7 + B9 + B10

- **B7**：`NotebookStore` 选 `ports::notebooks::notebook_store::NotebookStore` 为唯一出口，删另一路径。
- **B9**：`docs/README.md` 指向本计划/最新四维报告；architecture-baseline §2 Redis 路径改 `app-bootstrap`。
- **B10**：billing `seed_user_with_plan` + `seed_llm_usage_events` 提到 `tests/support/mod.rs`。

### S9 — 核实跨报告矛盾（P2）= §1.3

实测 desktop Tauri 测试数与 share contract 覆盖，确认技术债 v7 两条 Warning/Suggestion 是否已被测试 Round 7 核销；若是，更新技术债报告并 +分。

---

## 5. 验收命令（全部 Stream 完成后）

```bash
cd /home/chuan/context-osv6/avrag-rs
# 1. -D warnings gate（S1）
RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests
cargo check --workspace                                   # 无 Cargo 环（S3）
# 2. legal HTTP 测试（S4，有 PG）
DATABASE_URL=postgres://test:test@127.0.0.1:5432/test cargo test -p transport-http auth_legal
# 3. smoke 守卫
./scripts/run-product-smoke-e2e.sh --check-modules

cd ../frontend_next
pnpm vitest run tests/legal tests/billing                 # S5 + S2
pnpm exec playwright test --project=billing               # S2
pnpm exec playwright test --project=functional            # legal-consent（S4）

cd ..
bash scripts/verify-legal-p0.sh                           # 维持 40/40
```

---

## 6. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-06-13 v7 | 本轮：合并 Legal P1 review × Brooks 四维；v6 → archive；§1 核销 PR v6 五条 + legal 版本双源（已被 P0-CON-5 闭环）；§1.3 标注 desktop/share 跨报告矛盾；新增 9 项 Legal 增补（L1–L9），其中 L1 为支付同意接线引入的 billing E2E 回归 |
| 2026-06-13 v6 | [archive/brooks-merged-fix-plan-2026-06-13-v6.md](./archive/brooks-merged-fix-plan-2026-06-13-v6.md)（P4 后满分冲刺，M0–M15） |
| 2026-06-12 v3 | [archive/brooks-merged-fix-plan-2026-06-12-v3.md](./archive/brooks-merged-fix-plan-2026-06-12-v3.md) |
