# 三处改动 GAP 修复计划

## 1. 概述

### 目标
修复 Review 中发现的 P0–P2 级别 GAP，使 `code_gen_query` 在产品中对 LLM 可见、`run-e2e.sh` 可用、前端 E2E 覆盖 RAG 核心链路、以及代码健壮性缺陷。

### 范围
- P0：`code_gen_query` 注册到 `rag_tool_catalog()`；`run-e2e.sh` 服务检查修复
- P1：`run-e2e.sh` 与 playwright webServer 启动策略统一；前端 RAG mode 切换 E2E
- P2：SharePage selector 加固；`.env.example` 补齐；Milvus collection 显式清理

### 非目标
- 不新增 CI pipeline（已有 Playwright 配置）
- 不做像素级视觉回归（保持现状）
- 不改动 RAG/Search 策略状态机（无相关 GAP）

---

## 2. 依赖关系

```
P0-1 (code_gen_query 注册)
  └─> 依赖：common::CodeGenQueryArgs 结构已存在（✅）
P0-2 (run-e2e.sh 服务检查)
  └─> 依赖：无
P1-1 (启动策略统一)
  └─> 依赖：P0-2（先修复检查逻辑）
P1-2 (RAG mode 切换 E2E)
  └─> 依赖：前端 data-testid 已存在（✅）
P2-1 (SharePage selector)
  └─> 依赖：前端组件添加 data-testid
P2-2 (.env.example 补齐)
  └─> 依赖：无
P2-3 (Milvus 显式清理)
  └─> 依赖：无
```

**执行顺序**：P0-2 → P1-1 → P0-1 → P1-2 → (P2 并行)

---

## 3. P0 — 阻塞级修复

### P0-1: code_gen_query 注册到 rag_tool_catalog

**问题**：`code_gen_query` 在 runtime (`rag-core`) 中可执行，但 `rag_tool_catalog()` 只有 7 个工具，LLM planner 永远看不到它。

**修改文件**：
- `avrag-rs/crates/app/src/agents/progressive/tool_catalog.rs`
  - 在 `rag_tool_catalog()` vec 末尾追加第 8 个 `Tool::new(common::ToolSpec { ... })`
  - 字段：
    - `name`: `"code_gen_query"`
    - `version`: `"1.0"`
    - `description`: 引用 SKILL.md 中的 tool-selection rules（多步检索编排、cross-source correlation、adaptive iteration）
    - `input_schema`: 对应 `common::CodeGenQueryArgs` — `code` (string, required), `context` (object, optional), `session_id` (string, optional)
    - `output_schema`: 对应 `Vec<Chunk>` — `chunk_id`, `doc_id`, `content`, `score`, `source`, `page`, `chunk_type`
  - `gotchas`: 引用 SKILL.md 中的 Don't 列表
- `avrag-rs/crates/app/src/agents/progressive/tool_catalog.rs` 测试
  - `rag_tool_catalog_cached_has_seven_tools` → 重命名为 `has_eight_tools`，断言 `tools.len() == 8`
- `avrag-rs/crates/app/src/agents/capability/registry.rs` 测试
  - `list_tools_returns_all` 中 `"expected at least 11 tools"` → `"expected at least 12 tools"`

**验证**：
```bash
cd avrag-rs && cargo test -p app --lib progressive::tests::rag_tool_catalog_cached_has_eight_tools
cd avrag-rs && cargo test -p app --lib capability::registry::tests::list_tools_returns_all
cd avrag-rs && cargo test -p app --lib capability::registry::tests::can_lookup_rag_tools  # 新增断言 registry.tool("code_gen_query").is_some()
```

### P0-2: run-e2e.sh 服务检查逻辑修复

**问题**：`curl` 检查 PostgreSQL (TCP:5432) 和 Redis (TCP:6379) 会协议错误，几乎总是误判为未运行。

**修改文件**：
- `scripts/run-e2e.sh`
  - PostgreSQL: `pg_isready -h 127.0.0.1 -p 5432 >/dev/null 2>&1`
  - Redis: `redis-cli -h 127.0.0.1 -p 6379 ping 2>/dev/null | grep -q PONG`
  - 保留 Milvus/MinIO/avrag-api 的 HTTP check（它们本来就是 HTTP）
  - 如果 `pg_isready` / `redis-cli` 不存在，fallback 到 `nc -z`：
    ```bash
    check_tcp() { nc -z "$1" "$2" 2>/dev/null; }
    ```

**验证**：
```bash
# 场景1：服务未启动
cd /home/chuan/context-osv6 && ./scripts/run-e2e.sh
# 期望：仅 PostgreSQL/Redis 显示 ✗，Milvus/MinIO/api 按实际状态显示，脚本继续（见 P1-1）

# 场景2：服务已启动
# 期望：全部显示 ✓，进入 playwright 测试
```

---

## 4. P1 — 重要修复

### P1-1: 统一 run-e2e.sh 与 playwright webServer 启动策略

**问题**：当前脚本"服务缺失 → exit 1"，但 playwright.config.ts 的 webServer 会自动启动后端+前端，两者矛盾。

**修改文件**：
- `scripts/run-e2e.sh`
  - 删除 `missing -eq 1` 时的 `exit 1`，改为只打印 warning 并继续
  - 增加 `--skip-backend-check` 参数，用于 CI 中 playwright 自己管理 webServer
  - 最终逻辑：
    ```bash
    if [[ $missing -eq 1 ]]; then
      echo -e "${YELLOW}⚠ 部分服务未启动，Playwright webServer 将尝试自动启动${NC}"
    fi
    ```
  - 保留启动提示（docker compose、cargo run 等）作为参考，但不强制退出

- `frontend_next/playwright.config.ts`
  - 在 `webServer` 的 cargo run 命令中，添加 `E2E_ENABLED=true` 环境变量注入，确保 E2E Reset API 可用

**验证**：
```bash
# 场景：只启动 PostgreSQL/Redis/Milvus/MinIO，不启动 avrag-api 和前端
cd /home/chuan/context-osv6 && ./scripts/run-e2e.sh
# 期望：avrag-api 显示 ✗，但脚本继续，playwright webServer 自动启动后端和前端
```

### P1-2: 添加 RAG 模式切换的前端 E2E

**问题**：`workspace-upload-rag.spec.ts` 上传文档后直接发消息，没有显式切换到 RAG mode，也未验证 citation 来自上传文档。

**修改文件**：
- `frontend_next/e2e/pom/workspace-page.ts`
  - 新增方法：
    ```ts
    async switchToRagMode() {
      await this.page.getByRole("button", { name: /对话模式|Chat mode/i }).click();
      await this.page.getByRole("button", { name: /文档问答|RAG|document/i }).click();
    }
    ```

- `frontend_next/e2e/specs/workspace-upload-rag.spec.ts`
  - 在 `uploadFile` 和 `waitForIngestionComplete` 之后，**显式调用 `workspace.switchToRagMode()`**
  - 发送 RAG 问题后，增加断言：
    ```ts
    // 验证 mode-indicator 显示 rag
    await expect(page.locator("[data-testid='mode-indicator']")).toContainText(/rag|文档/i);
    // citation 按钮应可见（因为文档中有相关内容）
    await expect(workspace.getCitationButton()).toBeVisible();
    ```

- `frontend_next/components/workspace/workspace-chat-pane.tsx`
  - 确认 RAG mode 按钮的 text 包含 "文档问答" 或 "RAG"（当前已有 mode 切换 UI，只需确认 POM selector 能匹配）

**验证**：
```bash
cd frontend_next && npx playwright test specs/workspace-upload-rag.spec.ts --project=functional
```

---

## 5. P2 — 健壮性修复

### P2-1: SharePage selector 加固

**问题**：`SharePage.copyShareLink()` 使用 `[style*="font-family: ui-monospace"]`，样式一变即坏。

**修改文件**：
- `frontend_next/components/workspace/workspace-share-pane.tsx`（或对应的 share 页面组件）
  - 在 share URL 显示元素上添加 `data-testid="share-link"`

- `frontend_next/e2e/pom/share-page.ts`
  - `copyShareLink()` 改为：
    ```ts
    const urlLocator = this.page.locator('[data-testid="share-link"]');
    ```
  - `enableShare()` 中的 `waitForFunction` 也改为检测 `data-testid="share-link"` 的 textContent

**验证**：
```bash
cd frontend_next && npx playwright test specs/workspace-share.spec.ts --project=functional
```

### P2-2: .env.example 补齐 E2E 环境变量

**修改文件**：
- `avrag-rs/.env.example`
  - 在 E2E 段落末尾追加：
    ```
    # E2E Reset API secret（前端 Playwright E2E 调用 /api/e2e/reset-user-data 必需）
    E2E_RESET_SECRET=change-me-in-production

    # Playwright base URL（本地测试通常不需覆盖，CI/staging 中可能指向不同端口）
    # PLAYWRIGHT_BASE_URL=http://127.0.0.1:3000
    ```

**验证**：`grep E2E_RESET_SECRET avrag-rs/.env.example` 返回非空

### P2-3: Milvus collection 显式清理

**问题**：`e2e_ingestion_answer.rs` 的 `MilvusTestGuard::drop` 只打印 warning，未真正删除 collection。

**修改文件**：
- `avrag-rs/crates/app/tests/e2e_ingestion_answer.rs`
  - 在 `ingestion_answer_pipeline` 测试末尾、`assert!(result.status == Passed)` 之后，添加显式清理：
    ```rust
    // Explicit cleanup — best effort, errors don't fail the test
    if let Err(e) = data_plane.delete_collections_by_prefix(&collection_prefix).await {
        eprintln!("[WARN] Failed to drop Milvus collections with prefix '{}': {}", collection_prefix, e);
    }
    ```
  - 如果 `RetrievalDataPlane` trait 没有 `delete_collections_by_prefix`，则：
    - 方案 A：将 `data_plane` 从 `Arc<dyn RetrievalDataPlane>` 改为 `Arc<MilvusDataPlane>` 以便调用具体方法
    - 方案 B：通过 Milvus HTTP API 直接删除：`POST /v2/vectordb/collections/drop`
    - **推荐方案 B**（不引入类型耦合）

**验证**：运行测试后，检查 Milvus 中不存在 `e2e_ingestion_*` 前缀的 collection：
```bash
curl -s http://127.0.0.1:19530/v2/vectordb/collections/list | jq '.data[] | select(startswith("e2e_ingestion_"))'
# 期望：无输出
```

---

## 6. 运行与验收

### 本地验收清单

```bash
# 1. P0-1: code_gen_query 注册
cd avrag-rs && cargo test -p app --lib -- progressive::tests::rag_tool_catalog
cd avrag-rs && cargo test -p app --lib -- capability::registry::tests

# 2. P0-2 + P1-1: run-e2e.sh 可用
cd /home/chuan/context-osv6 && ./scripts/run-e2e.sh --project=auth

# 3. P1-2: RAG 模式切换 E2E
cd frontend_next && npx playwright test specs/workspace-upload-rag.spec.ts --project=functional

# 4. P2-1: Share E2E
cd frontend_next && npx playwright test specs/workspace-share.spec.ts --project=functional

# 5. P2-2: env 检查
grep -E "E2E_RESET_SECRET|PLAYWRIGHT_BASE_URL" avrag-rs/.env.example

# 6. P2-3: Milvus 清理（需先运行 ingestion test）
cd avrag-rs && cargo test --ignored -p app --test e2e_ingestion_answer -- --test-threads=1
# 然后检查 Milvus collections
```

### 回归检查

```bash
# 确保修改未破坏现有测试
cd avrag-rs && cargo test -p app --lib  # 所有 lib 测试
cd frontend_next && npx playwright test specs/auth-flow.spec.ts specs/workspace-chat.spec.ts --project=functional
```

---

## 7. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `code_gen_query` schema 与 `CodeGenQueryArgs` 不匹配 | 低 | 中 | 直接引用 `common::CodeGenQueryArgs` 的 serde 结构生成 schema |
| `pg_isready` / `redis-cli` 在部分环境不存在 | 中 | 低 | 已实现 `nc -z` fallback |
| Milvus HTTP API 删除 collection 需要鉴权 | 低 | 低 | 使用与 data_plane 相同的 token；若失败则不影响测试通过（warning only） |
| RAG mode 按钮 text 在不同 i18n 下不匹配 | 中 | 低 | POM 使用 `/rag|文档/i` 正则，覆盖中英文 |
