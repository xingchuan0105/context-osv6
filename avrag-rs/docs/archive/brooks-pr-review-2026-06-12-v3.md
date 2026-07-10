# Brooks-Lint Review

**Mode:** PR Review  
**Scope:** 工作区未提交变更 3 文件（`builder.rs`、`config.rs`、`infra.rs`，+18/−2）；并关联已落地的 `concurrent_query.rs` / `assertions.rs` / `e2e-gates.md` 做深入探测  
**Health Score:** 93/100  
**Trend:** 83 → 93（+10）— 自 v2 起调试输出已清除，P2-13 并发测试已补强；本次 diff 修复 E2E 租户身份错位

**一句话结论：** 本次变更是正确且必要的 E2E 基础设施修复（API/worker 与 HTTP 头身份对齐 + RAG 超时放宽），方向健康；需关注 `assert_independent_citation_chunks` 在单文档场景下的潜在 flaky 风险。

---

## 变更概览

| 文件 | 变更 | 目的 |
|------|------|------|
| `test_context/config.rs` | `E2eBootstrapConfig` 增加 `owner_user_id`/`user_id`；写入 `AppConfig` 与 worker env | 使 bootstrap 身份与 `x-owner-user-id`/`x-user-id` 一致 |
| `test_context/builder.rs` | 传入 identity；RAG 场景 HTTP timeout 60→120s | 修复错位；避免并发 RAG 请求超时 |
| `routes/infra.rs` | 注释说明 `/dev-upload` 在 `router_core` 注册 | 文档性，防路由重复误解 |

### 关联上下文（已提交，非本 diff 行）

| 项 | 状态 |
|----|------|
| `concurrent_query.rs` | 已用 `tokio::join!`、bridge 断言、关键词与 `assert_independent_citation_chunks` |
| v2 报告的 `eprintln!` 调试块 | 已移除 |
| `e2e-gates.md` | 已记录 P2-13 断言清单 |

### 修复前的身份错位（根因）

```
HTTP 客户端 ──► x-owner-user-id / x-user-id = unique_test_identity()  (每测试 UUID)
API bootstrap ──► AppConfig::default()  owner_user_id/user_id = 固定默认值   ← 修复前
Worker env    ──► 未显式设置 NEXT_PUBLIC_DEV_OWNER_USER_ID               ← 修复前
```

后果：ingestion/检索/worker 写入与 HTTP 请求租户不一致，并发或多租户 E2E 可能出现空 citation、403 或「假独立」结果。

---

## Findings

### 🟡 Warning

**Test Brittleness — 单文档场景要求 citation chunk 完全不相交**

Symptom: [`assert_independent_citation_chunks`](../crates/app/tests/product_e2e/assertions.rs) 要求两路响应的 `chunk_id` 集合交集为空；`concurrent_query` 对**同一** `antifragile.txt` 发起两路检索，主题虽不同（antifragility vs Lindy Effect），向量检索仍可能返回相同 top-k chunk。

Source: Meszaros — *xUnit Test Patterns*, Erratic Test; Hunt & Thomas — *The Pragmatic Programmer*, Orthogonality

Consequence: 身份对齐修复后测试更常走真实检索路径，重叠 chunk 导致间歇性失败；CI 上表现为「并发独立」用例不稳定，掩盖真实回归。

Remedy: 放宽为「非完全相同集合」或「至少一个 query 含独占 chunk」；或改用两文档 scope（参考 `multi_doc.rs`）再要求 disjoint；失败消息中打印 `chunks_a`/`chunks_b` 便于 triage。

---

### 🟢 Suggestion

**Cognitive Overload — HTTP 超时阶梯缺少命名常量**

Symptom: `builder.rs` 中 `http_timeout_secs` 为 `180 / 120 / 60` 三档字面量，120 新增无注释说明与 worker/RAG 耗时的关系。

Source: McConnell — *Code Complete*, Ch. 12 Fundamental Data Types

Consequence: 后续调整并发测试或 worker 超时时，HTTP 客户端超时易与 `worker_timeout_secs` 不同步。

Remedy: 提取 `const HTTP_TIMEOUT_RAG_SECS: u64 = 120`（及 REAL_LLM/DEFAULT），并在 `e2e-gates.md` 或 builder 顶部一行注释与 `worker_timeout_secs` 的关系。

---

**Dependency Disorder — Worker 身份 env 沿用 `NEXT_PUBLIC_*` 命名**

Symptom: `apply_worker_env` 通过 `NEXT_PUBLIC_DEV_OWNER_USER_ID` / `NEXT_PUBLIC_DEV_USER_ID` 注入 worker，与前端 public env 命名耦合（[`app-core/config.rs`](../crates/app-core/src/config.rs) L301–302 已有此约定）。

Source: Brooks — *The Mythical Man-Month*, Conceptual Integrity; Ousterhout — Information Leakage

Consequence: 新贡献者可能误以为仅 Next.js 消费这些变量；E2E 专用路径与生产配置边界不够直观。

Remedy: 在 `E2eBootstrapConfig.apply_worker_env` 旁注释「与 `AppConfig::from_env` 共用键，worker 无独立 AVRAG_OWNER_USER_ID」；长期可考虑 `AVRAG_DEV_OWNER_USER_ID` 别名。

---

## Quick Test Check（Step 7）

| 信号 | 结果 |
|------|------|
| 生产逻辑 + 缺测试 | 生产侧仅 `infra.rs` 注释，无行为变更 → 跳过 |
| Mock Abuse | 无 mock 测试改动 → 跳过 |
| Test Obscurity | `concurrent_query` 名称与 `e2e-gates.md` 一致；断言带消息 → 通过 |

**正向信号：** 身份对齐属于 Feathers 式「可安全改测试基础设施」的 seam 修复；`require_integration_suite()` 门禁已存在。

---

## 正向观察

- **深度模块修复：** 身份从 `unique_test_identity()` 单点流入 `E2eBootstrapConfig`，再 fan-out 到 API config 与 worker env，接口小、隐藏了 env 键细节。
- **P2-13 闭环：** v2 指出的并发/独立性问题在 `concurrent_query.rs` 已按 [`e2e-gates.md`](./e2e-gates.md) 落地。
- **路由注释：** `infra.rs` 一行注释避免 `/dev-upload` 重复注册误读，符合 Hyrum's Law 下的文档化。

---

## 推荐修复顺序

1. **合入** 当前 3 文件 diff（身份 + 超时 + 注释）— 阻塞性 bugfix
2. 观察 integration CI 上 `concurrent_query` 稳定性；若 flaky，按 Warning 放宽 chunk 独立性断言
3. 提取 HTTP 超时命名常量（可选，合入后小 PR）

### 验证命令

```bash
cd avrag-rs
cargo check -p transport-http
E2E_MODE=integration cargo test -p app --test product_e2e integration::concurrent_query -- --test-threads=1 --nocapture
```

---

## Summary

相较 v2（单文件 `eprintln!` 调试块），当前工作区已进入可合并区间：核心修复是 E2E 租户身份一致化，直接支撑 P2-13 并发测试可信度。剩余风险主要在 `assert_independent_citation_chunks` 对单文档 overlap 的零容忍，建议在 integration CI 跑一轮后决定是否放宽。无需重复 196 文件级 brooks 全量扫描。

---

## 历史与归档

| 文档 | 说明 |
|------|------|
| [`archive/brooks-pr-review-2026-06-12-v1.md`](./archive/brooks-pr-review-2026-06-12-v1.md) | 196 文件巨型 diff |
| [`archive/brooks-pr-review-2026-06-12-v2.md`](./archive/brooks-pr-review-2026-06-12-v2.md) | concurrent_query eprintln 审查 |
| 本文 v3 | 身份对齐 + 关联 P2-13 深入探测 |

记录来源：[`.brooks-lint-history.json`](../../.brooks-lint-history.json)
