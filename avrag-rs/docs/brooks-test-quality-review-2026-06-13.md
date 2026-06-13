# Brooks-Lint Review

**Mode:** Test Quality Review  
**Scope:** avrag-rs + frontend_next + contracts + desktop（全仓库深度审计，第五轮；Round 4 遗留项逐项复测 + 工作区编译/单测实测）  
**Health Score:** 68/100  
**Trend:** 69 → 68 (−1) over last 5 runs

Round 4 的三项 Warning 已在工作区落地（CI 迁移、Tauri 传输层单测、E2E seam 处置、脚本 EXIT trap）；但 **Product E2E 测试二进制当前无法编译**，托管 smoke/integration 门禁会在「Compile tests」步直接失败——这比「门禁不在线」更直接。

---

## Test Suite Map

```
Unit tests:        ~1,384 cases
  Rust inline (src):   1,077 #[test]/#[tokio::test]（202 文件）
  Frontend Vitest:     263 tests / 63 files（本地 15.6s 全绿）
  contracts (root):    44 tests

Integration tests: ~311 cases
  crates/*/tests:      211 #[test]/#[tokio::test]
  Product E2E:         ~100 attrs（product_e2e.rs 树；**当前编译失败，无法 --list 枚举**）
  行为契约:            admin_api / search_response / access_level / subscription_transition /
                       state_machine / rag_execute / chat_stream / storage_port / bootstrap 等
  retrieval-data-plane: behavioral_contract.rs

E2E tests:         ~60+ cases
  Playwright:          24 spec files（journey / billing / skills / visual）
  llm_real:            10 #[ignore]（含新增并发独立性变体 1 个）

Ratio:             Unit ~82% : Integration ~17% : E2E ~1%（Playwright 计 E2E 层）
                   金字塔形状健康；Product E2E 层当前不可执行

CI 门禁（根 `.github/workflows/`，Round 4 Critical 已闭环）:
  smoke-e2e.yml          PR → run-product-smoke-e2e.sh（working-directory: avrag-rs）
  integration-e2e.yml    main → 全量 product_e2e（E2E_MODE=integration）
  nightly-llm-real.yml   cron → llm_real --ignored
  frontend-unit.yml      PR → vitest + contract codegen drift check
  frontend-skills.yml    schedule → Playwright skills
  desktop-check          smoke-e2e 内 cargo check desktop（无 Rust 单测）
```

---

## Findings

### 🔴 Critical

**Coverage Illusion — Product E2E 测试二进制编译失败，smoke/integration 门禁无法执行**

Symptom: `cargo test --no-run -p app --test product_e2e --features product-e2e` 在当前工作区 **5 个编译错误**：（1）`mock_servers.rs:917` 使用 `headers`，但 handler 形参已改名为 `_headers`（删除 `x-mock-rag-query` 管道时的半完成重构）；（2）`concurrent_query.rs:106` 调用 `save_llm_artifact` 仅传 3 个参数（签名需 4 个：`test_name, resp, extra, capture`），并对同步函数误加 `.await`；（3）`setup.rs:546` `release_shared_milvus` 中 `milvus` 被借用同时又 move 进 `stop_milvus_and_clear_slot`（E0505）。`transport-http` 与 `contracts` 仍可编译；Vitest 263/263 全绿——问题集中在 Rust 最重门禁层。  
Source: Feathers — *Working Effectively with Legacy Code*, Ch. 1（无测试保护的变更路径）; Google — *How Google Tests Software*, Ch. 11（change coverage：门禁只有在可执行时才构成保护）  
Consequence: 根目录已迁移的 `smoke-e2e.yml` / `integration-e2e.yml` 在「Compile tests」步即红；Round 4 实测 59 pass / 0 fail 的集成基线**无法在当前工作区复现**；任何依赖 Product E2E 的 PR/main 保护形同虚设，直到编译修复。  
Remedy: 三处机械修复即可 unblock：（1）`mock_llm_handler` 将 `_headers` 改回 `headers`（`x-mock-route` 路由仍依赖该参数）；（2）`concurrent_query` 的 `save_llm_artifact` 对齐 `llm_real/*.rs` 四参数调用（`capture: None` 或传入 `result1.reasoning`），去掉 `.await`；（3）`release_shared_milvus` 先 `let name = milvus.container_name.clone()` 再 move `milvus`。修复后跑 `cargo test --no-run -p app --test product_e2e --features product-e2e` 与 `./scripts/run-product-smoke-e2e.sh` 验收。

---

### 🟡 Warning

**Coverage Illusion — share 模块大改后仅有枚举契约，handler/Store 行为无单测**

Symptom: 工作区删除 `share/src/db.rs`，`handlers.rs` / `members.rs` / `sharing.rs` 全面改为 `Arc<dyn ShareStorePort>` 注入；`share/src/` 内 **零** `#[test]`。现有测试仅 `access_level_contract.rs`（3 个枚举映射）与 `module_surface.rs`（lib 装配约束）——不覆盖 token 创建、成员邀请、公开读链路等 handler 行为。Product E2E 有 `smoke::share_boundary`，但属于慢路径集成，无法在 refactor 时提供快速反馈。  
Source: Feathers — legacy code is code without tests; Osherove — *The Art of Unit Testing*, 测试完整性原则  
Consequence: ShareStorePort 适配器或 handler 映射错误需等到 Product E2E 或手测才发现；与 admin crate 删除后迁移到 `app-admin` 测试的模式不一致（后者有 `admin_store_behavior.rs` + `storage_port_contract.rs`）。  
Remedy: 为 `ShareService` + 关键 handler 增加 in-memory fake `ShareStorePort` 单测（create/validate/load_shared_notebook/invite 各 1–2 例）；或扩展现有 `access_level_contract.rs` 为 `share_behavior.rs` 覆盖 Store 交互。

**Test Obscurity — Product E2E mock 层 1,800+ 行 Mystery Guest**

Symptom: `mock_servers.rs`（1,182 行）+ `test_context/builder.rs`（621 行）承载 mock LLM 路由、RAG 合成、embedding/search 桩逻辑；单个 smoke 测试失败时常需跨三文件追溯才能理解「mock 为何返回此内容」。测试名本身清晰（如 `rag_smoke`、`concurrent_rag_queries_are_safe_on_codegen_bridge`），但 **mock 预条件对读者不可见**（Mystery Guest + General Fixture 混合）。  
Source: Meszaros — *xUnit Test Patterns*, Mystery Guest (p.411); General Fixture (p.316)  
Consequence: mock 行为变更（如本轮 header 清理）易引入编译/语义回归且难定位；新贡献者不敢改 mock 层，形成「只改 production、不动 E2E 基础设施」的维护盲区。  
Remedy: 短期：在 `builder.rs` 顶部维护「mock 查询注入唯一路径」注释（已部分存在）并给 `mock_llm_handler` 路由表加 5 行模块级 doc；中期：将 `MockLlmRoute` 路由表与 canned response 抽到 `mock_routing.rs`（≤300 行），handler 只做 dispatch。

**Architecture Mismatch — 桌面 Rust 侧零单测，CI 仅 cargo check**

Symptom: `smoke-e2e.yml` 的 `desktop-check` job 只跑 `cargo check --manifest-path desktop/src-tauri/Cargo.toml`；`desktop/src-tauri/src/` 无 `#[cfg(test)]` 模块。前端已补 `tests/runtime/tauri-ipc.test.ts`（6 测）+ `transport.test.ts`（4 测）覆盖 IPC 适配层，但 **Tauri command handler**（`api_call` / `chat_stream` / `chat_cancel` / body 序列化）在 Rust 侧无契约测试。  
Source: Google — *How Google Tests Software*, 70:20:10 金字塔; Feathers — Ch. 4 Seam Model  
Consequence: 前端 mock 通过的 IPC 契约与 Rust command 实现可能漂移（例如 body 类型、错误码映射）；桌面 bug 只能端到端手测发现。  
Remedy: 为 `desktop/src-tauri/src/` 核心 command 提取纯函数（body 解析、错误映射）并加 `#[test]`；或在 CI 增加 `cargo test -p avrag-desktop`（若 command 逻辑可单测化）。

---

### 🟢 Suggestion

**Test Obscurity（反馈噪声）— 测试编译时生产 crate 警告常驻**

Symptom: `cargo test --no-run -p app --test product_e2e` 编译链输出 `app-core` 1 条 unused import、`billing` 4 条 dead function、`share` unused import 等，与测试自身 warning 混排。  
Source: McConnell — *Code Complete*, 整洁构建纪律  
Consequence: 真正预示 Product E2E 编译失败的 error 淹没在 warning 噪声中（本轮即发生）。  
Remedy: 随 refactor PR 清理 dead code；CI 编译步考虑 `-D warnings`（先清 warning 再启用）。

**Coverage Illusion — storage-local 重构后零 inline 测试**

Symptom: `storage-local` 仅 re-export `LocalContentStore` / `LocalCache`，工作区有改动但 crate 内无 `#[test]`；行为依赖上层 E2E 间接覆盖。  
Source: Osherove — 测试完整性原则  
Consequence: 本地缓存/内容存储边界条件（路径、租户前缀）回归无快速反馈。  
Remedy: 低优先级：为 `LocalCache` get/set/evict 加 2–3 个 tempfile 单测。

---

## Round 4 发现闭环核对（4/4 Critical+Warning 已解决，1 项引入编译回归）

| Round 4 发现 | 本轮实测状态 |
|---|---|
| 🔴 4 层 workflow 位于嵌套目录、GitHub 不触发 | ✅ 已迁至根 `.github/workflows/`（11 个文件）；`smoke-e2e.yml` / `integration-e2e.yml` 含 `defaults.run.working-directory: avrag-rs` 与 `avrag-rs/**` paths |
| 🟡 桌面传输层零测试 | ✅ `tauri-ipc.test.ts` 6 测 + `transport.test.ts` 4 测；Vitest **263/263 全绿**（15.6s） |
| 🟡 `x-mock-rag-query` 半接线 + 引文独立性断言孤儿 | ✅ 管道已删除（`rg` 仅 docs 提及）；`real_llm_concurrent_rag_queries_have_independent_citation_chunks` 恢复 `assert_independent_citation_chunks`；⚠️ mock_servers `_headers` 遗漏导致**新编译错误** |
| 🟡 fixture 泄漏 + 脚本无清理 | ✅ `run-product-smoke-e2e.sh` 已加 EXIT trap；`RagSharedFixture._object_store_guard` 已 `#[allow(dead_code)]`；本地 `docker ps --filter avrag-test-` **0 容器** |
| 🟢 生产 lib dead_code 警告 | ⏳ 仍存在（见 Suggestion 1） |

---

## 本轮实测记录（2026-06-13）

```bash
# 1. Frontend Vitest
cd frontend_next && pnpm vitest run
# Test Files  63 passed (63) / Tests  263 passed (263)  # 15.64s

# 2. Product E2E 编译（失败）
cd avrag-rs && cargo test --no-run -p app --test product_e2e --features product-e2e
# error[E0425]: headers not in scope (mock_servers.rs:917)
# error[E0061]: save_llm_artifact 3 args vs 4 (concurrent_query.rs:106)
# error[E0277]: () is not a future (concurrent_query.rs:116)
# error[E0505]: cannot move milvus (setup.rs:546)

# 3. 可编译子集
cargo test --no-run -p transport-http   # OK
cd ../contracts && cargo test --no-run  # OK

# 4. x-mock-rag-query 清理确认
rg 'x-mock-rag-query' avrag-rs/ --glob '*.rs'   # 0 匹配（仅 docs 保留决策记录）

# 5. 容器残留
docker ps --filter name=avrag-test- --format '{{.Names}}'   # 空
```

---

## 验收命令（修复 Critical 后回归）

```bash
# Critical：Product E2E 编译 + smoke
cd avrag-rs
cargo test --no-run -p app --test product_e2e --features product-e2e
./scripts/run-product-smoke-e2e.sh

# Critical：集成全量（修复后，~7–8 分钟）
E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1

# Warning：share 补测后
cargo test -p avrag-share

# Warning：桌面 Rust 补测后
cargo test --manifest-path desktop/src-tauri/Cargo.toml

# CI 触发验证（推送后）
gh run list --workflow=smoke-e2e.yml --limit 3
```

---

## 相关文档

- Round 1–4（已归档）：[`round1`](./archive/brooks-test-quality-review-2026-06-12-round1.md) / [`round2`](./archive/brooks-test-quality-review-2026-06-12-round2.md) / [`round3`](./archive/brooks-test-quality-review-2026-06-12-round3.md) / [`round4`](./archive/brooks-test-quality-review-2026-06-12-round4.md)
- [E2E Quality Gates](./e2e-gates.md)
- 历史分数：[`../../.brooks-lint-history.json`](../../.brooks-lint-history.json)

---

## Summary

Round 4 的结构性问题（CI 门禁不在线、Tauri 前端无测、E2E 死管道、脚本容器泄漏）已在工作区**实质性闭环**；Vitest 263 测全绿，根 workflow 11 文件就位，trap 与 llm_real 并发独立性 gate 均已落地。当前唯一阻塞项是 **Product E2E 五处编译错误**——属于 M10 seam 清理与 Milvus teardown 重构的未完成收尾，修复成本低但影响面大（PR/main Rust 门禁全部失效）。

次优先级：share 大改后的 handler/Store 单测缺口、mock 层 1,800 行可读性、桌面 Rust command 无测。三项可随 port 迁移 PR 分批消化，不阻塞编译修复。
