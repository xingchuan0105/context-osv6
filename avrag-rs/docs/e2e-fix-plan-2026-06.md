# Product E2E 测试修复计划

> 基于 product_e2e 代码 review（2026-06-02）

---

## P0 — 阻塞项（测试目前会失败或测不到东西）

### P0-1: `cross_org_rag_does_not_leak_documents` 是假测试

**问题**: User B 和 User A 是同一个 org，doc_scope 明确包含 A 的文档，断言永远为 true。

**修复**:
1. 在 `TestContext` 增加 `with_org_id(owner_user_id: &str)` 工厂方法，生成不同 auth header 的上下文
2. 重写测试：
   - Org A 上传 antifragile.txt
   - Org B 上传 empty.txt
   - Org B 查询 "What is antifragility?"（不传 doc_scope 或只传 B 的 doc）
   - 验证返回的 citations 中**没有** doc_id == A 的文档

**验收**: 测试在正确实现下通过，在故意移除 org 过滤时代码下失败。

---

### P0-2: `search_smoke` 断言引用不存在的 `layer` 字段

**问题**: `assert_answer_has_web_citation` 检查 `c.layer == "search"`，但 `Citation` struct 没有 `layer` 字段。

**修复**:
1. 确认 production `contracts/src/chat.rs::Citation` 字段
2. 如果无 `layer`，改为通过 `doc_id` 格式判断（URL 格式 = web citation）
3. 如果未来要加 `layer`，先改 production schema，再同步测试

**验收**: search_smoke 在真实运行时不因 layer 字段缺失而失败。

---

### P0-3: `unsafe { std::env::set_var }` 并发竞争

**问题**: `build_smoke` 在 unsafe block 中 set env var，并发测试会互相覆盖。

**修复**:
1. 短期：在 `TestContext` 中用 `std::sync::Mutex` 保护 set_var 调用（所有 `build_smoke` 串行化）
2. 长期：改 `AppConfig` 支持从 `struct` 实例构建（而非仅从 env），`TestContext` 传显式 config

**验收**: `concurrent_query` 测试和任意两个并行的 `TestContext::new_smoke` 不再出现 mock 地址错位。

---

## P1 — 断言质量（测得到但测不准）

### P1-4: `format_output` 字符串断言 → HTML 解析

**问题**: `contains("slide")` 和 `contains("<html")` 是脆弱断言。

**修复**:
1. 引入 `scraper` crate 到 dev-dependencies
2. `assertions.rs` 新增 `assert_html_has_structure(html: &str, selectors: &[&str])`
3. `format_output.rs` 改用 DOM 断言：
   - `presentation-html`: `html.select(".slide").count() >= 2`
   - `html-renderer`: `html.select("body").count() == 1`

**验收**: 测试在 mock LLM 返回有效 HTML 时通过，返回纯文本时失败。

---

### P1-5: `assert_format_output_type` 空实现

**问题**: 函数完全忽略 `expected_type`，只检查 answer 非空。

**修复**:
1. 在 `ChatResponse` 加 `format_output: Option<FormatOutput>` 字段（或先用 answer 内容推断）
2. 或短期：根据 answer 中的 marker 推断类型，断言匹配 `expected_type`

**验收**: 调用 `assert_format_output_type(resp, "presentation-html")` 时，如果 answer 不含 slide marker 则失败。

---

### P1-6: `concurrent_query` 验证深度不足

**问题**: 只验证"两个请求都成功"，不验证并发安全。

**修复**:
1. 验证 `chat1.state_history` 和 `chat2.state_history` 独立（如果有）
2. 验证 `chat1.citations` 和 `chat2.citations` 的 `chunk_id` 不交错（如果 mock LLM 返回不同内容）
3. 或至少验证两个 response 的 `message_id` 不同

**验收**: 并发测试能发现共享状态污染。

---

## P2 — 健壮性（测试稳定但代码质量差）

### P2-7: `find_worker_binary` 路径硬编码

**修复**: 用 `std::env::current_exe()` 推导 workspace root，再拼接 `target/debug/avrag-worker`。

---

### P2-8: `Drop` 中 `Runtime::new()` 健壮性

**修复**: 把 `std::thread::spawn` 中的 `Runtime::new()` 改为 `tokio::runtime::Builder::new_current_thread()`，避免和外部 runtime 冲突。

---

### P2-9: `set_ingestion_max_attempts` 每次新建 PgPool

**修复**: 在 `TestContext` 中缓存 `PgPool`（`Option<sqlx::PgPool>`），`set_ingestion_max_attempts` 复用。

---

### P2-10: `timeout.rs` 依赖文档大小

**修复**: 明确使用一个足够大的 fixture（如 50KB 的 lorem ipsum），确保任何机器上 1s 都处理不完。

---

## 执行顺序

| 步骤 | 内容 | 依赖 | 预估时间 |
|:----:|------|------|:--------:|
| 1 | P0-2: 确认 Citation 字段 + 改 search_smoke 断言 | 无 | 10 min |
| 2 | P0-3: Mutex 保护 set_var | 无 | 20 min |
| 3 | P0-1: 重写 cross_org_rag | 步骤 2 | 30 min |
| 4 | P1-4: HTML 解析断言 | 无 | 30 min |
| 5 | P1-5: assert_format_output_type 实现 | 步骤 4 | 15 min |
| 6 | P1-6: concurrent_query 深度验证 | 无 | 20 min |
| 7 | P2-7~P2-10: 健壮性小修复 | 无 | 20 min |
| 8 | 全量 product_e2e 回归 | 全部 | 10 min |

**合计: ~2.5 小时**
