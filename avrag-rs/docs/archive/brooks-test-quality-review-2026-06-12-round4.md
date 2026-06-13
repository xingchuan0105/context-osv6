# Brooks-Lint Review

**Mode:** Test Quality Review  
**Scope:** avrag-rs + frontend_next + contracts + desktop（全仓库深度审计，第四轮；合并 [`e2e-gates.md`](./e2e-gates.md) 遗留问题逐项实测）  
**Health Score:** 69/100  
**Trend:** 82 → 69 (−13) over last 4 runs

> **口径说明（重要）：** 分数下降**不代表测试内容退化**——Round 3 的全部 6 项发现已解决或大幅缓解，且本轮实测全量集成套件首次全绿（59 passed / 0 failed，447s）。下降来自本轮新探到的**门禁执行层**问题：5 层质量门禁中 4 层的 workflow 文件位于 GitHub 永不读取的嵌套目录，托管 CI 上从不触发。若仅按 Round 3 的"测试内容"口径，本轮约为 95 分。

测试套件内容已是健康状态（命名、断言取向、fixture 设计、行为契约配对全部达标）；当前唯一的结构性风险是「写好的门禁没有在跑」+ 新桌面传输层零测试。

---

## Test Suite Map

```
Unit tests:        ~1,146 cases
  Rust inline (src):   880 #[test]/#[tokio::test]
  Frontend Vitest:     254 tests / 61 files（本地 15.8s 全绿*）
  contracts (root):    12 tests
  Python SDK:          2 files

Integration tests: ~224 cases（crates/*/tests）
  Product E2E:         72 attrs；E2E_MODE=integration 实测 59 pass / 0 fail / 10 ignored，447s
  行为契约:            8 个 surface crate 全部配对 behavioral contract
                       （admin_api / search_response / access_level / subscription_transition /
                         state_machine / rag_execute / notebook_search_query / chat_stream 等）
  retrieval-data-plane: behavioral_contract.rs 8 tests（Round 3 时为 3）

E2E tests:         ~60 cases
  Playwright:          24 spec files（journey / billing / skills / visual）
  llm_real:            #[ignore] nightly（真实 LLM，9 处）

Ratio:             Unit ~83% : Integration ~13% : E2E ~4%
                   （单元层偏厚的金字塔，可接受）

* Vitest 全绿前提：先 pnpm install——本轮发现 package.json 新增 @tauri-apps/api
  未同步 lockfile，曾导致 12/61 文件载入失败（详见 Warning 1）
```

---

## Findings

### 🔴 Critical

**Coverage Illusion — 5 层质量门禁中 4 层的 workflow 位于 GitHub 永不读取的目录**

Symptom: [`e2e-gates.md`](./e2e-gates.md) 触发表声明 5 层门禁（PR smoke / Integration main / llm_real nightly / Playwright skills / judge）。实测仓库根 `.github/workflows/` 只有 3 个文件：`frontend-unit.yml`（PR，在线）、`nightly-playwright-judge.yml`（cron，在线但 score<6 仅警告）、`playwright-extended-e2e.yml`（仅手动）。而 `smoke-e2e.yml`、`integration-e2e.yml`、`nightly-llm-real.yml`（另有 nightly-quality、weekly-regression）全部位于 `avrag-rs/.github/workflows/`，`frontend-skills.yml` 位于 `frontend_next/.github/workflows/`——这两个目录不是子模块（无嵌套 `.git`、无 `.gitmodules`，唯一 remote 是 `context-osv6`），GitHub Actions 只读仓库根的 `.github/workflows/`，嵌套位置的 workflow **永不触发**。文件内容也按"仓库根 = avrag-rs"撰写（`paths: crates/**`、cargo 在 checkout 根直接执行），即使原样搬到根目录也不会工作。git 历史佐证漂移过程：先 "chore: remove GitHub Actions CI, add local E2E runner"，后 "ci: add E2E gate workflows" 把门禁加回了不会执行的位置。  
Source: Google — *How Google Tests Software*, Ch. 11（change coverage：门禁只有在执行时才构成保护）; Feathers — *Working Effectively with Legacy Code*（无测试保护的变更路径）  
Consequence: PR 与 main 上 **Rust 零强制**——编译、单元、契约、Product E2E 全部依赖开发者自觉本地执行；五层门禁中唯一自动运行的恰是"不会失败"的 judge 层。文档描述的 hard gate 体系给出虚假安全感（gates 文档自己也记录过"上次全量 run 被中断"却无 CI 兜底发现）。本轮三份姊妹报告（架构/PR/技术债 v4）均未覆盖此问题。  
Remedy: 二选一并验证：(a) 将 4 个 workflow 迁至根 `.github/workflows/`，补 `defaults.run.working-directory: avrag-rs` 与路径前缀（`paths: 'avrag-rs/crates/**'` 等），用一个空 PR 实测触发；(b) 若刻意本地化执行，在 `e2e-gates.md` 触发表加"执行方式"列标明"本地脚本，未托管"，并提供 pre-push hook 调用 `run-product-smoke-e2e.sh`，消除文档与现实的落差。

---

### 🟡 Warning

**Coverage Illusion — 桌面端（Tauri）传输层零测试，本轮改动已踩中一个本可被单测拦截的 bug**

Symptom: 生产链路本轮重布线：`use-chat-stream` → `streamChat`、`api-access/client` → `restRequest`，二者经新增的 [`lib/runtime/transport.ts`](../../frontend_next/lib/runtime/transport.ts) 按环境分流到 [`tauri-ipc.ts`](../../frontend_next/lib/runtime/tauri-ipc.ts) 或 Web 实现；另有整个 `desktop/src-tauri` 新应用。全仓库**没有任何测试**引用 `runtime/transport`、`tauri-ipc` 或 `useChatStream`。佐证一：工作区 diff 中 `tauri-ipc.ts` 恰是手工修复 `JSON.parse(init.body as string)` 在 body 非字符串时的崩溃——典型单测可拦截缺陷。佐证二：`package.json` 新增 `@tauri-apps/api` 未跑 `pnpm install`，导致 Vitest 12/61 文件载入失败（本轮已执行安装并同步 lockfile，254 测全绿；若 lockfile 不随提交，`frontend-unit.yml` 的 `--frozen-lockfile` 会让唯一在线的 PR 门禁直接红）。佐证三：重构后 `api-access/client.ts` 的 `decodeError` 成为零引用孤儿函数。  
Source: Feathers — "legacy code is code without tests"（活跃修改区无测试）; Osherove — *The Art of Unit Testing*, 测试完整性原则  
Consequence: Web 路径有 `stream.test.ts` / `client.test.ts` 兜底（重构后原样通过，证明其断言绑定行为而非实现——这部分是健康信号）；但 Tauri 分支（`streamChatViaIPC` / `requestViaIPC` / `initLocalBackend`）的任何回归只能靠桌面端手测发现。  
Remedy: 为 `tauri-ipc.ts` 写纯单测（`vi.mock("@tauri-apps/api/core")` 模拟 `invoke`/`Channel`），覆盖 body 序列化（字符串/对象/空）、流事件转发顺序；`transport.ts` 分流逻辑按 `isTauri` 两分支各一测；删除孤儿 `decodeError`；提交本轮已同步的 `pnpm-lock.yaml`。

**Mock Abuse — `x-mock-rag-query` 半接线 seam + 引文独立性断言成孤儿**

Symptom: 本轮改动让 `TestContext` 在 `/api/v1/chat` 请求上附加 `x-mock-rag-query` header（[`test_context/http.rs`](../crates/app/tests/product_e2e/test_context/http.rs) 285/475 行），mock LLM handler 读取同名 header（[`mock_servers.rs`](../crates/app/tests/product_e2e/mock_servers.rs) 846 行）。但生产代码**不转发** inbound header 到 LLM 调用（全仓库搜索无生产触点；gates 文档自述 "not forwarded today"）——header 在两端都是死管道，实际查询解析仍依赖 messages 解析 + 有竞态的全局 `set_mock_rag_codegen_query`（http.rs 仍在 3 处设置）。同时 `concurrent_query` 删除强断言后，`assert_independent_citation_chunks`（[`assertions.rs`](../crates/app/tests/product_e2e/assertions.rs):59）成为零引用孤儿——"并发查询引文独立"这一产品意图目前**没有任何 gate 表达**（mock 路径已删、llm_real 变体未建）。  
Source: Meszaros — *xUnit Test Patterns*, Behavior Verification (p.544); Osherove — mock completeness; Feathers — Ch. 4 Seam Model  
Consequence: 未来测试作者看到 header 会以为存在 per-request 隔离，在并发场景再次踩中全局 cell 竞态——正是迫使本轮断言弱化的根因；引文独立性回归将静默通过。  
Remedy: 三选一落实：(a) `E2E_ENABLED` 下让 LLM client 转发该 header，恢复强断言；(b) 删除两端死代码，在 builder 注释指明唯一可靠路径是 messages 解析；(c) 落实 gates 文档提议的 `#[ignore]` llm_real 独立性变体，把孤儿断言挂进去。（弱化本身合理且已在测试内注释说明——本条针对的是半接线管道与丢失的意图，不是弱化决定。）

**Erratic Test（资源泄漏型）— fixture 故意泄漏 + 清理跳过活跃容器，容器堆积已实测发生**

Symptom: [`fixtures/ready_rag.rs`](../crates/app/tests/product_e2e/fixtures/ready_rag.rs) 用 5 处 `mem::forget` + `static OnceCell` 维持进程级基础设施（设计意图有注释，合理），代价是测试二进制退出后**无人停止 PG 容器**；`setup` 的孤儿清理只删非活跃/非年轻的 `avrag-test-*`；本地脚本 `run-product-smoke-e2e.sh` 无 trap/结尾清理。实测本机当前滞留 **≥9 个 `avrag-test-pg-*`（22 分钟～2 小时+）+ 1 个 redis**，Milvus 中残留多个 `avrag_e2e_*` collection。keep-alive 字段同时触发 2 条常驻编译警告（`fields never read`、`RagSharedFixture` 可见性低于 `shared_rag_fixture`）。  
Source: Meszaros — *xUnit Test Patterns*, Erratic Test（共享环境状态）; gates 文档开放问题 #1  
Consequence: 本地高频跑套件持续累积容器占用端口/内存；"young/active 跳过"规则让下一轮清理也不回收它们；若未来 CI 启用，runner 同样累积。常驻警告训练开发者忽略编译输出中的真实信号。  
Remedy: 给 `run-product-smoke-e2e.sh` 加 `trap 'docker ps -aq --filter name=avrag-test- | xargs -r docker rm -f' EXIT`（与 CI yml 的清理步对齐）；fixture keep-alive 字段加 `_` 前缀或 `#[allow(dead_code)]` 并注明 keep-alive 意图；`shared_rag_fixture` 可见性对齐为 `pub(crate)`。

---

### 🟢 Suggestion

**Test Obscurity（反馈噪声）— 生产 lib 的 dead_code 警告常驻测试编译输出**

Symptom: 每次 `cargo test` 编译输出 app lib 16 条 + admin 4 条（`audit.rs` 4 个函数零调用）+ billing 2 条（`OrderStatus` / `BillingOrder`）warning，与测试自身的 2 条警告混在一起。  
Source: McConnell — *Code Complete*, 整洁构建纪律; Google — 信噪比  
Consequence: 真正预示问题的警告（如本轮 fixture 的 never-read）被噪声淹没。  
Remedy: 死代码处置归技术债流程（v4 债务报告已覆盖前端 fetch 重复等）；测试侧目标是警告清零后在 CI 启用 `-D warnings`。

---

## Round 3 发现的闭环核对（6/6 解决）

| Round 3 发现 | 本轮实测状态 |
|---|---|
| 🟡 8 crate `module_surface` 假覆盖 | ✅ 8 个 crate 全部配对行为契约测试；retrieval-data-plane 3→8 测试 |
| 🟡 Product E2E 45 分钟 CI 预算 | ✅ 实测全量 447s（7.5 分钟）；但重新定性为 Critical 的"门禁不在线"问题 |
| 🟡 typewriter 测试文件偏大 | ✅ 658 → 270 → 160 行 |
| 🟢 Playwright citation 条件断言 | ✅ `E2E_TIER=nightly\|staging` hard gate 已落地（spec 62 行） |
| 🟢 billing/api.test.ts 命名缺 subject | ✅ 已改为 `getPlans_throwsOnServerError` 等 subject-first 命名 |
| 🟢 Surface 测试重复 `vi.mock` 块 | ✅ `helpers/workspace-surface.{mocks,setup,harness}` 三件套落地 |

---

## e2e-gates.md 遗留问题对照（合并分析）

### 开放问题（5 项）

| # | 遗留项 | 本轮核实结果 | 去向 |
|---|---|---|---|
| 1 | `mem::forget(abort_tx)` 无显式关停 | 仍存在；容器堆积已现场证实（≥9 PG + redis + Milvus 残留 collection） | → Warning 3 |
| 2 | `concurrent_query` 语义弱化 | 弱化合理且测试内有注释；**实测 PASS（20.5s）**；但独立性意图无 gate、header seam 半接线 | → Warning 2 |
| 3 | `--features product-e2e` CI 确认 | workflow 文件已正确传递 feature ✔；但 workflow 本身在托管 CI 不触发 | → Critical |
| 4 | parser 重复模块树（mineru/router） | ✅ 已解决：仅存 `mineru/`、`router/` 目录树，无同名 `.rs` 兄弟文件，编译通过 | 可关闭 |
| 5 | docs drift（含陈旧 CI 注释） | ✅ 工作区已更新 gates 文档；`shared_ready_rag` / `Mutex<TestContext>` 陈旧注释全仓已清零 | 可关闭 |

### 未验证项（3 项 → 本轮全部转绿）

| 项 | gates 记录状态 | 本轮实测 |
|---|---|---|
| `integration::concurrent_query` | "pass not confirmed" | ✅ **1 passed，20.51s** |
| `smoke::rag_codegen_multitool_smoke` | "not re-run after fix" | ✅ **1 passed，18.20s** |
| 全量 `E2E_MODE=integration` 套件 | "Unknown"（前基线 49 pass / 6 fail / 10 ignored，~387s） | ✅ **59 passed / 0 failed / 10 ignored，447.47s** |

---

## 本轮实测记录（2026-06-12）

```bash
# 1. Vitest：首跑 12/61 文件载入失败（@tauri-apps/api 未安装）
#    pnpm install 后：
Test Files  61 passed (61) / Tests  254 passed (254)  # 15.8s

# 2. product_e2e 编译门禁
cargo test --no-run -p app --test product_e2e --features product-e2e   # 通过（2 条测试侧警告）

# 3. gates 清单逐项
E2E_MODE=integration cargo test ... integration::concurrent_query        # ok, 20.51s
E2E_MODE=integration cargo test ... smoke::rag_codegen_multitool_smoke   # ok, 18.20s
E2E_MODE=integration cargo test -p app --test product_e2e \
  --features product-e2e -- --test-threads=1                             # 59 pass / 0 fail / 10 ignored, 447.47s
```

---

## 验收命令（修复后回归）

```bash
# Critical：门禁迁移后，用空 PR 验证 4 个 workflow 实际触发
gh run list --workflow=smoke-e2e.yml --limit 3

# Warning 1：Tauri 传输层补测后
cd frontend_next && pnpm vitest run tests/runtime/

# Warning 2：seam 处置后（任一方案）
rg -n 'x-mock-rag-query' avrag-rs/   # 方案 b 应为 0 生产外触点
cargo test -p app --test product_e2e --features product-e2e integration::concurrent_query -- --test-threads=1

# Warning 3：清理兜底后
./avrag-rs/scripts/run-product-smoke-e2e.sh; docker ps --filter name=avrag-test- --format '{{.Names}}'  # 应为空
```

---

## 相关文档

- Round 1–3（已归档）：[`round1`](./archive/brooks-test-quality-review-2026-06-12-round1.md) / [`round2`](./archive/brooks-test-quality-review-2026-06-12-round2.md) / [`round3`](./archive/brooks-test-quality-review-2026-06-12-round3.md)
- [E2E Quality Gates](./e2e-gates.md)（遗留问题状态本轮已同步更新）
- 姊妹报告（同日 v4）：[架构审计](./brooks-architecture-audit-2026-06-13-v4.md) / [PR 审查](./brooks-pr-review-2026-06-12-v4.md) / [技术债](./brooks-tech-debt-assessment-2026-06-12-v4.md)
- 历史分数：[`../../.brooks-lint-history.json`](../../.brooks-lint-history.json)

---

## Summary

测试**内容**已经健康：Round 3 全部 6 项发现闭环，gates 文档 3 个未验证项本轮实测全绿（全量集成套件 59/0/10，447s，历史首次零失败），命名、断言取向与 fixture 设计均无新增内容性问题。当前最高优先级是**把写好的门禁接上电**——4 个 E2E workflow 位于 GitHub 永不读取的嵌套目录，PR/main 上 Rust 零强制，这一项修复（迁移 + working-directory 适配 + 空 PR 验证）即可拿回 15 分并让其余投入真正生效。

次优先级：桌面传输层补单测（本轮已暴露一个手修 bug 和一次 lockfile 失同步）、`x-mock-rag-query` 半接线 seam 三选一处置、本地脚本容器清理兜底。三项都是小改动，可随日常迭代消化。
