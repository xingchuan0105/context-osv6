# Brooks-Lint Review — 技术债深度评估 v7

**Mode:** Tech Debt Assessment
**Scope:** `avrag-rs`（34 workspace members）+ `frontend_next` + `contracts` + `desktop`；v7 深度复查，重点验证 v6 罗列的偿还路线图是否全部闭环、以及 v6 之后引入的 legal 再确认功能与 M9 拆分尾期工作是否带来新债务。
**Config:** 无 `.brooks-lint.yaml`，六类衰减风险全部启用
**Health Score:** 99/100
**Trend:** 86 → 99 (+13) vs v7 初稿；S9 跨报告矛盾核实后核销 desktop/share 两条过时 Warning

**一句话结论：** v7 计划 S0–S9 已全部落地；v6 偿还路线图 9/9 闭环；技术债 v7 初稿中 desktop「0 行为测试」与 share「contract 只完成一半」经实测为**测试 Round 7 已覆盖、技术债报告过时**；剩余唯一 Monitored 项是 legal 版本号长期 typeshare 化（短期已由 `verify-legal-p0.sh` P0-CON-5 跨语言守护）。

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
| smoke runner 模块清单解析 | ✅ 闭环 | `scripts/run-product-smoke-e2e.sh` 使用 `sed -n 's/.*::smoke::\([^:]*\)::.*/\1/p'` 正确剥离前缀；`assert_smoke_module_coverage` 双向校验注册 vs 发现 |

### 1.3 跨报告矛盾核实（S9，2026-06-13 实测）

技术债 v7 初稿与测试 Round 7 报告对 desktop / share 的结论不一致。本轮按 v7 §1.3 命令实测后核销如下：

| 技术债 v7 声称 | 测试 Round 7 / 实测 | S9 结论 |
|----------------|----------------------|---------|
| desktop `src-tauri` **0** 个 `#[test]`；CI 仅 `cargo check` | `registry` 2 + `api` 2 + `backend` 2 + `chat` 7 = **13** 测；`smoke-e2e.yml:130` 为 `cargo test --manifest-path desktop/src-tauri/Cargo.toml` | ✅ **核销** — 技术债 Warning 过时 |
| share contract 只 4 测、invite/public-read 缺失 | `share_behavior.rs` **6** + `storage_port_contract.rs` **4** + `access_level_contract.rs` **3** + `module_surface.rs` **1** = **14** 测；`owner_can_invite_member` / `non_owner_invite_is_rejected_before_store` / `load_shared_notebook` / `resolve_public_share_chat_context` 均已覆盖 | ✅ **核销** — 技术债只数了 `storage_port_contract.rs` |
| legal 版本号双源、无 CI 守护 | `scripts/verify-legal-p0.sh` **P0-CON-5** 三源字面值比对 + `license-check.yml` `legal-p0-verify` job | ✅ **核销** — Brooks 架构/技术债「无守护」结论不准确 |
| `agents/loop/mod.rs` dead `merge_request_doc_scope` re-export | S1 已删；`RUSTFLAGS="-D warnings" cargo check --workspace` 全绿 | ✅ **核销** |
| 工作树 38 文件未分层 | S0 三分支 commit（legal / app-chat / hygiene）已入库 | ✅ **核销** |
| `LegalReacceptanceGate` 硬编码中文 | S7 `messages/legal.ts` + `formatUiMessage` 全量替换 | ✅ **核销** |

```bash
# S9 验收命令（2026-06-13 复跑）
cargo test --manifest-path desktop/src-tauri/Cargo.toml --quiet
# running 13 tests ... ok. 13 passed

cargo test -p avrag-share --tests
# share_behavior 6 + storage_port_contract 4 + access_level_contract 3 + module_surface 1

rg '#\[test\]' desktop/src-tauri/src -c
# commands/chat.rs:7 registry.rs:2 commands/api.rs:2 commands/backend.rs:2
```

---

## Findings

### 🟢 Suggestion（唯一剩余项）

**Knowledge Duplication — legal 版本号长期仍建议 typeshare 化（短期已 CI 守护）**

Symptom: `frontend_next/lib/legal/versions.ts` 与 `app-core/src/legal_versions.rs` 仍各维护一份常量；但 `scripts/verify-legal-p0.sh` **P0-CON-5** 已在 CI 做三源字面值比对（MDX frontmatter + TS + Rust），与 Brooks 初稿「无守护」结论不同。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY

Consequence: 无 CI 时仍可能人手漏改；有 CI 时漏改会在 `license-check` 红灯。

Remedy: 长期把版本常量迁入 `contracts/` 并 typeshare 生成；短期维持 P0-CON-5 即可，不阻塞合并。

Priority: Pain 1 × Spread 1 = **1**（Monitored） | Intent: **[accidental]**

---

### S9 已核销项（原 v7 Warning/Suggestion，不再计分）

| 原 Finding | 核销证据 |
|------------|----------|
| dead `merge_request_doc_scope` re-export | S1 删除；`-D warnings` workspace 全绿 |
| desktop 0 行为测试 / CI 仅 check | 13 `#[test]`；`cargo test --manifest-path desktop/src-tauri/Cargo.toml` 全绿 |
| share contract 只完成一半 | `share_behavior.rs` 6 测覆盖 invite + public-read |
| 工作树未分层 | 3 commits：`acda6da` legal / `dfdaac2` app-chat / `317fb3f` hygiene |
| LegalReacceptanceGate i18n 混用 | `lib/i18n/messages/legal.ts` 接入 |
| legal 版本双源无守护 | P0-CON-5 + `legal-p0-verify` CI job |

---

## Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|--------------|----------------|--------|
| Cognitive Overload | 0 | — | Clean | — |
| Change Propagation | 0 | — | Clean（S0 分层入库） | — |
| Knowledge Duplication | 1 | 1.0 | Monitored | accidental |
| Accidental Complexity | 0 | — | Clean | — |
| Dependency Disorder | 0 | — | Clean | — |
| Domain Model Distortion | 0 | — | Clean（S7 i18n） | — |
| Coverage Illusion | 0 | — | Clean（S9 核销 desktop/share） | — |

**Recommended focus:** 无阻塞项。可选长期把 legal 版本常量 typeshare 化；`pg_*_store` port_impl 体量仍是架构层 Suggestion，不重复计入技术债 Critical/Warning。

---

## 2. 偿还路线图（v7 → 已完成）

| 档 | 任务 | 状态 |
|----|------|------|
| 第一档 | B1 dead re-export | ✅ S1 |
| 第二档 | desktop Tauri 行为测试 + legal CI 守护 | ✅ M12 + P0-CON-5（S9 核实） |
| 第三档 | share contract / 工作树分层 / LegalReacceptanceGate i18n | ✅ S0/S7/S9 |

---

## 3. 验证记录（M15，2026-06-13 post-v7）

```bash
cd avrag-rs
RUSTFLAGS="-D warnings" cargo check --workspace
# Finished `dev` profile — 零 warning

RUSTFLAGS="-D warnings" cargo build -p avrag-worker -p app --features product-e2e --tests
# Finished — 零 warning

./scripts/run-product-smoke-e2e.sh --check-modules
# OK: smoke module coverage guard passed (11 modules match cargo --list)

cargo test -p transport-http --lib auth_legal
# 5 passed

cargo test -p avrag-share --tests
# 14 passed (6 behavior + 4 contract + 3 access_level + 1 module_surface)

cd ../desktop/src-tauri && cargo test --quiet
# 13 passed

cd ../../frontend_next && pnpm vitest run
# 70 files / 293 tests passed

cd .. && bash scripts/verify-legal-p0.sh
# 40/40 passed
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
| `crates/share/tests/share_behavior.rs` | 6 行为测：public notebook / public chat context / invite 授权 |
| `desktop/src-tauri/src/` | 13 `#[test]`；CI `cargo test --manifest-path desktop/src-tauri/Cargo.toml` |
| `scripts/verify-legal-p0.sh` | 40 项含 P0-CON-5 三源版本比对 |
| `frontend_next/lib/i18n/messages/legal.ts` | LegalReacceptanceGate 全量 i18n |
| `crates/app/tests/product_e2e/mock_rag_state.rs` | S6：8 个 OnceLock 收敛为 `MockRagState` |

---

## Summary

v7 计划 S0–S9 执行完毕后，技术债从初稿 86 升至 **99/100**。S9 核实确认：desktop 与 share 两条 Warning 是技术债报告未同步测试 Round 7 的**过时结论**；legal 版本双源已有 P0-CON-5 CI 守护。剩余 1 分留给 legal 常量长期 typeshare 化（可选 P3）。无 Critical、无 Warning、无 Dependency Disorder。

---

*生成工具：Brooks-Lint Tech Debt Assessment · 2026-06-13 v7 post-S9/M15*
