# Brooks-Lint Review — 技术债深度评估 v7

**Mode:** Tech Debt Assessment
**Scope:** `avrag-rs`（34 workspace members）+ `frontend_next` + `contracts` + `desktop`；v7 深度复查，重点验证 v6 罗列的偿还路线图是否全部闭环、以及 v6 之后引入的 legal 再确认功能与 M9 拆分尾期工作是否带来新债务。
**Config:** 无 `.brooks-lint.yaml`，六类衰减风险全部启用
**Health Score:** 86/100
**Trend:** 57 → 86 (+29) vs v6

**一句话结论：** v6 罗列的 6 个 Critical/Warning 几乎全部闭环（Paddle image E2E、LiteParse parse snapshot、execute_pdf_parse 拆分、文档对齐 P4、HTTP 统一、warning 门禁），`agents/loop/mod.rs` 与 `eval/framework.rs` 千行文件也分别降到 218 行与 8 行；剩余债务集中在 v6 偿还路线图未覆盖的几处（desktop Tauri 缺少 Rust 行为测试、share contract 行为测试只完成一半），以及最近 legal 再确认新功能引入的版本号硬编码双源。

> **归档：**
> - v1 → [`archive/brooks-tech-debt-assessment-2026-06-12-v1.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v1.md)（Health 34）
> - v2 → [`archive/brooks-tech-debt-assessment-2026-06-12-v2.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v2.md)（Health 58）
> - v3 → [`archive/brooks-tech-debt-assessment-2026-06-12-v3.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v3.md)（Health 70）
> - v4 → [`archive/brooks-tech-debt-assessment-2026-06-12-v4.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v4.md)（Health 59）
> - v5 → [`archive/brooks-tech-debt-assessment-2026-06-13-v5.md`](./archive/brooks-tech-debt-assessment-2026-06-13-v5.md)（Health 61）
> - v6 → [`archive/brooks-tech-debt-assessment-2026-06-13-v6.md`](./archive/brooks-tech-debt-assessment-2026-06-13-v6.md)（Health 57，P4 后首次深度评估，罗列 6 项偿还路线图）

---

## 1. 审计范围与方法

| 维度 | 说明 |
|------|------|
| 配置 | 无 `.brooks-lint.yaml`，六类衰减风险全部启用 |
| 重点变更 | v6→v7 期间：M0–M14 计划执行、agents loop/eval 千行文件拆分、shards.lst+build.rs 持久化分片、legal 再确认新功能（新增） |
| 工作树 | 28 modified + 10 untracked = 38 文件（v6 时 421 文件，下降 91%） |
| 证据来源 | `git status`、`cargo check --workspace`、`RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker`、源码 `rg`、文件行数实测、`crates/app/tests/product_e2e/` 测试清单核对 |
| 验证结果 | workspace check 通过仅 1 warning；`-D warnings` 门禁因为 M9 拆分余漏被 app-chat 阻塞；ingestion/worker/share/billing 单元测试可编译 |
| 优先级公式 | Pain × Spread（1–3）；7–9 Critical / 4–6 Scheduled / 1–3 Monitored |

### 1.1 v6 偿还路线图核销表

| v6 路线图 | 当前状态 | 证据 |
|-----------|----------|------|
| **第一档** | | |
| 1. 独立图片 PaddleOcrImage E2E/asset contract | ✅ 闭环 | `crates/app/tests/product_e2e/integration/paddle_image_e2e.rs` + `smoke/paddle_image_smoke.rs` 已存在；断言 `ParseRoute::PaddleOcrImage` 与 `execute_paddle_ocr_image` 路径 |
| **第二档** | | |
| 2. LiteParse parse pass 缓存/合并 | ✅ 闭环 | `LiteParseService::parse_pdf_document` 返回 `ParsedPdfSnapshot`；`liteparse_probe_bridge.rs` 提供 `run_liteparse_snapshot_blocking` 让 router 与 worker 共享一次 parse 结果 |
| 3. 清理 ingestion/worker warning，恢复 `-D warnings` | ⚠️ 部分回退 | `cargo check --workspace` 仅 1 warning，但该 warning 来自 M9 拆分尾期 `mod.rs` 的死 re-export，导致 `RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker` 仍失败 |
| 4. LiteParse 文档/runbook 去 shadow/MinerU 现行描述 | ✅ 闭环 | `docs/runbooks/worker-dev.md` 已标 "已删除：MinerU、`LITEPARSE_ENABLED`"；`liteparse-paddle-ingestion-architecture-2026-06-13.md` 仅在配置阈值表保留 `LITEPARSE_*_THRESHOLD` 实际生效环境变量 |
| 5. 拆 `execute_pdf_parse` 阶段函数 | ✅ 闭环 | `bins/worker/src/pdf/parse.rs` 已拆出 `collect_page_routes` → `run_ocr_pages` → `apply_text_fallbacks` → `attach_ingest_metadata_and_status`，主函数仅做 4 步薄编排（文件 551 行，含完整阶段实现与单元测试） |
| 6. `billing/featureFlag.ts` 接统一 HTTP | ✅ 闭环 | `frontend_next/lib/billing/featureFlag.ts` 已改为 `import { ApiError, request } from "../http/request"`，原 `buildApiUrl + fetch` 被 `request<UsageWindowProbeEnvelope>` 替代 |
| **第三档** | | |
| 7. `EdgeParse` / `Mineru*` 历史枚举注释 / 命名迁移 | ✅ 充分核销 | `rg "Mineru\|EdgeParse\|LiteParseImage"` 在 `crates/ingestion/src/parser/*.rs` 与 `router/*.rs` 已 0 命中（旧 v7 路线图所担心的"留在主链"问题不存在） |
| 8. 删除 `parse_json`、`to_pdf_page_probe` 等 shadow-era API | ✅ 闭环 | `rg "shadow diff\|parse_json\|to_pdf_page_probe" crates/ingestion/src/parser` 0 命中 |
| 9. `agents/loop/mod.rs` 继续薄化 | ✅ 闭环并超额 | 实测 218 行（v6 报告 1289 行）；`run()` 仅 5 步：`normalize_query` → `prepare_run_request` → `run_retrieval_loop` → `resolve_synthesis_gate` → `run_synthesis_phase`。新增同层文件 `run_prepare.rs` / `run_retrieval.rs` / `run_synthesis.rs` / `run_fallback.rs` / `run_result.rs` |

### 1.2 v6 计划外的额外完成项

| 项 | 状态 | 证据 |
|----|------|------|
| `eval/framework.rs` 千行文件拆分（v6 架构审计提的 1633 行） | ✅ 几乎清零 | `eval/framework.rs` 当前 8 行（仅 deprecated re-export），实际逻辑已拆到 `types.rs` / `compare.rs` / `metrics.rs` / `llm_judge.rs` / `runner.rs` / `evaluator.rs`（untracked） |
| `app-core` Redis 限流器迁出（v6 架构 Warning） | ✅ 闭环 | `app-core/src/adapters/` 仅余 `memory.rs` / `mod.rs`；`redis_rate_limiter` 已迁到 `app-bootstrap/src/adapters/`，`app-core/Cargo.toml` 不再 `redis.workspace = true` |
| `avrag-share` handler 去 `axum::Json`（v6 架构 Suggestion） | ✅ 闭环并加门禁 | `crates/share/tests/storage_port_contract.rs::handlers_do_not_leak_http_framework_types` 用 `include_str!` 把 `axum::` 字串检查写成 compile-time-style 守护；`Cargo.toml` 不再依赖 axum |
| `pg_share_store` / `pg_admin_store` 拆分 | ✅ 闭环并加守护 | 每个业务域一个 .rs 文件（share 9 个 + 共用 mappers/mod，admin 7 个）；`build.rs` 按 `shards.lst` 拼装单一 `impl ...Port` 块写入 OUT_DIR（绕开 Rust 2024 `impl` 内 `include!` 限制）；`port_shard_guard` 测试守护：(a) `shards.lst` 存在、(b) 每个 shard 文件存在、(c) `port_impl.rs` 通过 `include!(concat!(env!("OUT_DIR"), ...))` 加载、(d) 无孤立 `.rs` 文件未列入清单 |
| smoke runner 模块清单解析 | ✅ 闭环 | `scripts/run-product-smoke-e2e.sh` 使用 `sed -n 's/.*::smoke::\([^:]*\)::.*/\1/p'` 正确剥离 `product_e2e::smoke::<module>::` 前缀；`assert_smoke_module_coverage` 双向校验注册 vs 发现 |

---

## Findings

### 🟡 Warning

**Accidental Complexity — `agents/loop/mod.rs` 残留 dead re-export 击穿 `-D warnings` 门禁（M9 拆分回退）**

Symptom: `cargo check --workspace` 仅 1 warning：
```
warning: unused import: `merge_request_doc_scope`
  --> crates/app-chat/src/agents/loop/mod.rs:35:48
   |
35 | pub(crate) use rag_bridge::{dispatch_rag_tool, merge_request_doc_scope};
```
`merge_request_doc_scope` 在 `rag_bridge.rs` 内部被 `dispatch_rag_tool` 调用；M9 把 `iteration.rs` / `policy/config.rs` 等拆出来后，外部不再有调用方，但 mod.rs 这一行 re-export 没同步清理。`RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker` 因为 app-chat 是其 lib 依赖链，命中此 warning 即失败。

Source: McConnell — *Code Complete*, construction hygiene；Winters et al. — *Software Engineering at Google*, code sustainability

Consequence: v6 第二档第 3 项明确要求"恢复 `-D warnings` 可用性"；当前回退状态会让 M2 的验收命令再次失败，掩盖未来更多 unused/dead 项。

Remedy: 把第 35 行改成 `pub(crate) use rag_bridge::dispatch_rag_tool;`（不再 re-export `merge_request_doc_scope`，因为它是 `rag_bridge.rs` 模块内部的私有 helper），随后跑一次 `RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker -p app-chat` 收尾。

Priority: Pain 2 × Spread 1 = **2**（Monitored），按 v6 偿还路线图判定为 **Warning**（因为回退了已闭环的 gate） | Intent: **[accidental]**

---

**Coverage Illusion — desktop Tauri 命令处理器 0 行为测试，CI 仍仅 `cargo check`**

Symptom: `find desktop/src-tauri/src -name "*.rs" -exec grep -l "#[test]"` 完全为空；`desktop/src-tauri/src/` 下没有任何 `#[test]` / `#[tokio::test]`。M12 计划要求"抽纯函数测试 body 构造、错误映射、stream event 转换"，未执行。

Source: Feathers — *Working Effectively with Legacy Code*, Sensing & Separation；Google — *How Google Tests Software*, change coverage

Consequence: desktop 走 Tauri IPC 而非 HTTP；任何 IPC body 构造、SSE 转换、错误映射变化，只在手动启动 Tauri shell 时才能发现。一旦回归，调试成本远高于纯函数 unit test。

Remedy: 按 v6 M12 计划：(a) 把 IPC payload 构造、`reqwest` 错误到 IPC 错误的映射、SSE event → Tauri event 的转换抽成纯函数；(b) 在同 crate 加 `#[test]`；(c) `smoke-e2e.yml` 的 desktop job 把 `cargo check` 升级到 `cargo test --manifest-path desktop/src-tauri/Cargo.toml`。

Priority: Pain 2 × Spread 2 = **4**（Scheduled） | Intent: **[accidental]**

---

### 🟢 Suggestion

**Knowledge Duplication — legal 协议版本号在前后端各硬编码一份，无单一事实源**

Symptom: 两处独立常量：
- `frontend_next/lib/legal/versions.ts`: `export const PUBLISHED_TERMS_VERSION = "2026-06-13"`
- `avrag-rs/crates/app-core/src/legal_versions.rs`: `pub const PUBLISHED_TERMS_VERSION: &str = "2026-06-13"`

前端在 `lib/legal/client.ts::recordLegalAcceptance` 把 `PUBLISHED_TERMS_VERSION` 作为 `terms_version` 字段提交给 `/api/auth/legal-acceptance`；后端 `auth_legal_status_handler` 又把自己的 `PUBLISHED_TERMS_VERSION` 通过 `LegalStatusPayload.published_terms_version` 返给前端。两侧常量本应永远一致但靠人手同步。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY；Ousterhout — *A Philosophy of Software Design*, Information Leakage

Consequence: 升级协议时一边改、另一边漏改，会出现 (a) 前端显示新版本但记录旧版本，或 (b) 后端认为已是新版本但前端拒不显示再确认页。`validate_published_legal_versions`（已在 `app-core/lib.rs` re-export）只能验证后端自洽，无法跨语言守护。

Remedy: 短期 — 加 frontend lint：CI 跑 `node` 小脚本读 `crates/app-core/src/legal_versions.rs` 与 `frontend_next/lib/legal/versions.ts`，断言两个常量字面值相等。长期 — 把版本号搬到 `contracts/` 并用 `typeshare` 同步生成（与现有 `contracts → frontend_next/lib/contracts/` 流程一致）。

Priority: Pain 1 × Spread 2 = **2**（Monitored） | Intent: **[accidental]**

---

**Coverage Illusion — share port contract 只完成一半（v6 M11 partial）**

Symptom: `crates/share/tests/storage_port_contract.rs` 只有 4 个测试：
1. `handlers_do_not_leak_http_framework_types`（M13 静态守护）
2. `share_modules_do_not_call_storage_pg_escape_hatch`（端口纪律守护）
3. `create_share_token_round_trips_through_validate_token`（行为）
4. `validate_token_returns_none_for_unknown_token`（行为）

v6 M11 计划要求 4 个行为测试：(a) shared notebook payload mapping、(b) public chat context mapping、(c) owner invite 成功、(d) non-owner invite 被拒。当前只覆盖 (a) 的 token round-trip 一面，invite 路径与 public read 路径完全没有 in-memory fake 行为测试。

Source: Meszaros — *xUnit Test Patterns*, Test Code Duplication / Behavior Verification

Consequence: PG adapter 已经按业务域拆分（`access.rs` / `invite.rs` / `invite_accept.rs` / `invite_decline.rs` / `members_mutate.rs` / `public_read.rs`），但 contract 层没有对应行为锚。任何 invite/public read 行为变更，只能依赖 PG 集成测试发现。

Remedy: 在现有 `support::MemoryShareStore` 上补 3 个 `#[tokio::test]`：(a) `owner_can_invite_member`、(b) `non_owner_invite_is_rejected_with_forbidden`、(c) `public_read_returns_chat_context_for_valid_token`。

Priority: Pain 1 × Spread 2 = **2**（Monitored） | Intent: **[accidental]**

---

**Change Propagation — 工作树仍有 10 个 untracked 与 28 个 modified 文件未分层（legal 新功能 + M9 拆分尾期）**

Symptom: `git status --short` 显示：
- 28 modified（auth/profile/lib.rs、pg_auth_store、transport-http auth/router_core、share contract test、frontend auth/billing/pricing/legal-gate 等）
- 10 untracked（`iteration/` 目录、`policy/config/` 目录、`tests.rs`、`message_format.rs`、`rag_bridge.rs`、`eval/evaluator.rs`、`eval/runner_tests.rs`、`legal/LegalReacceptanceGate.tsx`、`legal/client.ts`、`playwright/`）

两个独立主题（legal 再确认 + M9 拆分尾期）混在同一工作树。比起 v6 的 421 文件已大幅收敛（-91%），但 v6 D0 担心的"删旧录新不在同一层"问题以更小规模再现。

Source: Brooks — *The Mythical Man-Month*, coordination cost；Feathers — change set safety

Consequence: 当前 commit 历史会让 reviewer 看到不完整的 legal 功能或不完整的 M9 拆分；CI 也只能针对当前 staged 状态跑（譬如 `agents/loop/iteration/`、`policy/config/` 是 untracked，意味着 master 还没有这些目录的合规 commit）。

Remedy: 按主题分批：
1. **Legal 再确认 PR**：`frontend_next/{components/legal,lib/legal,lib/auth/errors.ts,lib/i18n/messages/auth.ts,app/(app)/upgrade/paywall,app/(marketing)/pricing,components/auth-gates,components/settings/settings-billing-panel}.tsx` + `avrag-rs/crates/{app-bootstrap/src/adapters/pg_auth_store.rs,app-core/src/auth_store.rs,app-core/src/lib.rs,transport-http/src/{auth_types.rs,lib_impl/auth/profile.rs,lib_impl/router_core.rs,lib_impl/tests.rs,routes/auth.rs}}` + `.github/workflows/*` + `scripts/verify-legal-p0.sh`。
2. **M9 拆分尾期 PR**：`avrag-rs/crates/app-chat/src/agents/loop/{mod.rs,iteration/,policy/config/,tests.rs,message_format.rs,rag_bridge.rs}` + `avrag-rs/crates/app-chat/src/eval/{mod.rs,llm_judge.rs,runner.rs,evaluator.rs,runner_tests.rs}`。
3. **graphify-out/manifest.json** 跟随对应 PR 自动更新。
4. **frontend_next/playwright/** 若为空目录，先 `rmdir` 或加 `.gitkeep` + 子文件。

Priority: Pain 1 × Spread 2 = **2**（Monitored） | Intent: **[intentional]**（功能未完成）

---

**Domain Model Distortion — `LegalReacceptanceGate.tsx` 混用 i18n 与硬编码中文字符串**

Symptom: 同一组件内：
- ✅ 用 `formatUiMessage(locale, "gateCheckingSession")` / `formatUiMessage(locale, "authRegisterFailed")` / `describeAuthError(formatUiMessage(...), error, locale)`
- ❌ 硬编码：`"协议已更新"`、`"我们更新了用户服务协议或隐私政策。继续使用前，请阅读并确认最新版本。"`、`"提交中..."`、`"确认并继续"`、`"请先阅读并同意最新版用户协议与隐私政策"`

`lib/legal/client.ts::PaymentConsentRequiredError` 也硬编码默认 `"请先阅读并同意用户协议与隐私政策"`。同时该组件内联 `style={{ maxWidth: "28rem", textAlign: "center" }}` 与 `className="app-surface-card"` 并存，与项目其他组件用 Tailwind/CSS Module 的风格不一致。

Source: Evans — *Domain-Driven Design*, Ubiquitous Language；Fowler — *Refactoring*, Inconsistent Naming（这里是 i18n 通道不一致）

Consequence: 多语言切换时这一面板永远显示中文；后续如果产品做 zh/en 切换，gate 是阻断登录后首屏的组件，会成为最显眼的不一致点。

Remedy: 把所有硬编码字符串落到 `lib/i18n/messages/auth.ts`（或新建 `messages/legal.ts`），通过 `formatUiMessage(locale, key)` 渲染；`PaymentConsentRequiredError` 接受外部 message 注入，不在错误类内硬编码默认中文。

Priority: Pain 1 × Spread 1 = **1**（Monitored） | Intent: **[accidental]**

---

## Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|--------------|----------------|--------|
| Cognitive Overload | 0 | — | Clean | — |
| Change Propagation | 1 | 2.0 | Monitored | intentional |
| Knowledge Duplication | 1 | 2.0 | Monitored | accidental |
| Accidental Complexity | 1 | 2.0 | Monitored（gate 回退） | accidental |
| Dependency Disorder | 0 | — | Clean | — |
| Domain Model Distortion | 1 | 1.0 | Monitored | accidental |
| Coverage Illusion（跨多类） | 2 | 3.0 | Monitored/Scheduled | accidental |

**Recommended focus:** 先 1 行修 `mod.rs` 的 dead re-export 把 `-D warnings` gate 重新关上；接着把 desktop Tauri 命令补行为测试（M12 一直没动），并把 legal 版本号双源加 CI 校验。share contract 行为测试与工作树分层属于卫生项，可在下一个 PR 顺手完成。

---

## 2. 偿还路线图（v7，目标 86 → 100）

### 2.1 第一档：必须先修（86 → 91）

| # | 任务 | 验收 |
|---|------|------|
| 1 | 移除 `agents/loop/mod.rs:35` 对 `merge_request_doc_scope` 的死 re-export | `RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker -p app-chat` 全绿 |

### 2.2 第二档：测试与版本守护（91 → 96）

| # | 任务 | 验收 |
|---|------|------|
| 2 | 补 desktop Tauri Rust 行为测试（M12） | `cargo test --manifest-path desktop/src-tauri/Cargo.toml` 至少含 3 个 `#[test]`；CI desktop job 从 `cargo check` 升 `cargo test` |
| 3 | legal 版本号前后端一致性 CI 守护 | 加一段 `scripts/verify-legal-versions.sh` 或 `frontend_next` vitest，比较前端 `versions.ts` 与后端 `app-core::legal_versions` 字面值；CI 失败时给出明确 diff |

### 2.3 第三档：卫生收尾（96 → 100）

| # | 任务 | 验收 |
|---|------|------|
| 4 | `share` contract 补 invite / public-read 行为测试 | `cargo test -p avrag-share storage_port_contract` 含 3 个新 tokio test |
| 5 | 把当前工作树按"Legal 再确认 PR" / "M9 拆分尾期 PR" 拆 commit | `git status --short` 在每个 PR 落地后 untracked=0、modified 与该 PR 主题完全对应 |
| 6 | `LegalReacceptanceGate.tsx` 全部走 `formatUiMessage`；删除内联 style 与硬编码中文 | `rg "协议已更新\|请先阅读\|提交中" frontend_next/components/legal` 0 命中 |

---

## 3. 验证记录

```bash
cd avrag-rs
cargo check --workspace
# Finished `dev` profile in 23.55s
# warning (1): unused import `merge_request_doc_scope` at crates/app-chat/src/agents/loop/mod.rs:35:48

RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker
# error: unused import: `merge_request_doc_scope`
# error: could not compile `app-chat` (lib) due to 1 previous error

# 文件大小实测
wc -l crates/app-chat/src/agents/loop/mod.rs
#  218 crates/app-chat/src/agents/loop/mod.rs    （v6 报告 1289 行）

wc -l crates/app-chat/src/eval/framework.rs
#    8 crates/app-chat/src/eval/framework.rs     （v6 架构审计 1633 行）

wc -l bins/worker/src/pdf/parse.rs
#  551 bins/worker/src/pdf/parse.rs              （含 4 个阶段函数 + 单元测试）

# v6 偿还核销证据
rg "execute_paddle_ocr_image|PaddleOcrImage" crates/app/tests/product_e2e/
# crates/app/tests/product_e2e/integration/paddle_image_e2e.rs:1
# crates/app/tests/product_e2e/smoke/paddle_image_smoke.rs:1

rg "Mineru|EdgeParse|LiteParseImage" crates/ingestion/src/parser/*.rs crates/ingestion/src/parser/router/*.rs
# 0 hits（M14 已闭环）

rg "shadow diff|parse_json|to_pdf_page_probe" crates/ingestion/src/parser
# 0 hits（M14 shadow-era API 已删除）

# 工作树规模
git status --short | wc -l    # 38（v6 报告 421）
git status --short | grep -c '^??'   # 10
```

---

## 4. 附录：关键代码锚点

| 路径 | 观察 |
|------|------|
| `crates/app-chat/src/agents/loop/mod.rs` | 218 行；`ReActLoop::run` 极简 5 步编排；唯一 warning 来源在第 35 行 |
| `crates/app-chat/src/agents/loop/rag_bridge.rs` | `merge_request_doc_scope` 在此文件内被 `dispatch_rag_tool` 使用，无需对外 re-export |
| `crates/app-chat/src/eval/framework.rs` | 8 行 deprecated re-export；真实实现在同目录 `types.rs` / `compare.rs` / `metrics.rs` / `runner.rs` / `llm_judge.rs` |
| `bins/worker/src/pdf/parse.rs` | `execute_pdf_parse` 主函数仅做 4 步薄编排，每步对应一个 `pub async fn` |
| `crates/ingestion/src/parser/liteparse.rs` | `parse_pdf_document` 返回 `ParsedPdfSnapshot`；`probe` / `extract_blocks` / `page_dimensions` 可基于 snapshot 复用 |
| `crates/ingestion/src/parser/liteparse_probe_bridge.rs` | `run_liteparse_snapshot_blocking` 让 router 与 worker 共享一次 parse |
| `crates/app-bootstrap/src/adapters/pg_share_store/` | 9 业务域 shard + `mappers.rs` + `mod.rs` + 1-line `port_impl.rs` + `shards.lst`；`build.rs` 按清单拼装 trait impl 到 OUT_DIR；`tests` 守护无孤儿 |
| `crates/app-bootstrap/src/adapters/pg_admin_store/` | 7 业务域 shard，结构同上 |
| `crates/app-bootstrap/src/adapters/redis_rate_limiter.rs` | Redis 限流器从 `app-core` 迁出后的当前归属 |
| `crates/share/tests/storage_port_contract.rs` | 2 静态守护 + 2 行为测试；M11 4 个行为测试只完成 1 个 |
| `frontend_next/lib/billing/featureFlag.ts` | 已统一走 `lib/http/request::request<>` |
| `frontend_next/lib/legal/versions.ts` ↔ `avrag-rs/crates/app-core/src/legal_versions.rs` | 协议版本号双源，靠人手同步 |
| `frontend_next/components/legal/LegalReacceptanceGate.tsx` | i18n 通道与硬编码中文混用 |
| `desktop/src-tauri/src/` | 0 `#[test]`；CI 仅 `cargo check` |
| `scripts/run-product-smoke-e2e.sh` | 模块清单解析正确（`.*::smoke::\([^:]*\)::.*`）；`assert_smoke_module_coverage` 双向守护 |

---

## Summary

v6→v7 期间 Brooks 第二档/第三档 6 项偿还任务（独立图片 E2E、LiteParse parse 缓存、`execute_pdf_parse` 拆分、LiteParse 文档对齐、`billing/featureFlag.ts` HTTP 统一、`agents/loop/mod.rs` 薄化）全部闭环；附带完成 v6 架构审计提的 `app-core` Redis 迁出、`avrag-share` 去 axum、`eval/framework.rs` 千行拆分、`pg_*_store` shards.lst+build.rs 持久化分片。工作树从 421 文件降到 38 文件，trend +29 分。剩余债务全部是"v6 计划未覆盖的卫生项"+"legal 新功能小漂移"：1 个 `-D warnings` 回退（1 行修复）、desktop Tauri 测试缺口（结构性，待 M12 完成）、legal 版本号双源、share contract 行为测试完成度一半、工作树分层、legal i18n 一致性。无 Critical、无 Dependency Disorder、无 Cognitive Overload。

---

*生成工具：Brooks-Lint Tech Debt Assessment · 2026-06-13 v7（v6 偿还路线图深度复查 + legal 新功能债务扫描）*
