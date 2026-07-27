# E2E 重构实施计划（按 Slice 拆分）

**对应设计文档**: `docs/superpowers/specs/2026-06-03-product-e2e-redesign-design.md`
**生成日期**: 2026-06-03

---

## 总体策略

- **每个 Slice = 一个独立 PR**，可独立 review、独立合并
- **Slice 0 必须先合并**，后续 Slice 依赖它
- **Slice 7（Rust CI 补全）可与 Slice 1-4 并行开发**
- **所有 PR 共用同一分支基线**：`main`，通过 rebase 保持线性

---

## PR 拆分总览

```
PR #1  Slice 0: 基础设施（已就绪，可直接提 PR）
  |
  +--> PR #2  Slice 1: Auth Flow
  |       |
  |       +--> PR #3  Slice 2: Workspace Chat + WebSearch
  |               |
  |               +--> PR #4  Slice 3: Upload + RAG
  |                       |
  |                       +--> PR #5  Slice 4: Share
  |
  +--> PR #6  Slice 5: 负面测试（依赖 Slice 1+2）
  |
  +--> PR #7  Slice 6: Cross-browser + Visual（依赖 Slice 2）
  |
  +--> PR #8  Slice 7: Rust CI 补全（与 Slice 1-4 并行）
```

---

## PR #1 — Slice 0: 基础设施

### 目标
搭建 `frontend_next/e2e/` 完整骨架，使 `npx playwright test --project=functional` 能成功运行。

### 包含文件（已生成，可直接提交）

**新增**:
```
frontend_next/e2e/global-setup.ts
frontend_next/e2e/setup-env.ts
frontend_next/e2e/setup-auth.ts
frontend_next/e2e/fixtures/test-user.ts
frontend_next/e2e/fixtures/run-context.ts
frontend_next/e2e/utils/api-helpers.ts
frontend_next/e2e/pom/login-page.ts
frontend_next/e2e/pom/dashboard-page.ts
frontend_next/e2e/pom/workspace-page.ts
frontend_next/e2e/specs/auth-flow.spec.ts
frontend_next/e2e/specs/workspace-chat.spec.ts
.github/workflows/e2e-cross-browser.yml
```

**修改**:
```
frontend_next/playwright.config.ts          # 重写
.github/workflows/e2e-staging.yml           # 新增 frontend 步骤
```

**临时保留**（Slice 1 删除）:
```
frontend_next/e2e/login-hydration.spec.ts   # 旧文件，逻辑已迁入 auth-flow.spec.ts
```

### 依赖
- 后端需实现 `/api/e2e/reset-user-data` 端点（6 层安全 gate）
- 如后端未就绪，可临时注释掉 `setup-env.ts` 中的 `resetTestUserData` 调用，用 `deleteWorkspaceViaAPI` 逐个清理作为降级方案

### 验收标准
```bash
cd frontend_next
npx playwright test --project=auth
# 期望：auth-flow.spec.ts 通过（注册+登录+hydration）

npx playwright test --project=functional
# 期望：workspace-chat.spec.ts 通过（需 reset API 已就绪）
```

### 风险与降级
| 风险 | 降级方案 |
|------|---------|
| reset API 未实现 | 临时注释 reset，改为 spec afterAll 逐个 deleteWorkspace |
| avrag-api 启动超时 | webServer timeout 从 120s 调至 180s |
| login selector 不匹配 | 根据实际 DOM 调整 `#login-email` / `#login-password` |

---

## PR #2 — Slice 1: Auth Flow

### 目标
验证注册/登录/登出的业务功能，删除旧文件。

### 包含文件

**新增**: 无（auth-flow.spec.ts 已在 Slice 0 中创建）

**修改**: 无

**删除**:
```
frontend_next/e2e/login-hydration.spec.ts   # 逻辑已完整迁入 auth-flow.spec.ts
```

### 依赖
- PR #1（Slice 0）已合并

### 验收标准
```bash
npx playwright test --project=auth
# 期望：2 个 test 全部通过
```

---

## PR #3 — Slice 2: Workspace Chat + WebSearch

### 目标
完成核心用户旅程：创建 notebook → general chat → web search mode → 验证历史/引用。

### 包含文件

**新增**: 无（workspace-chat.spec.ts 已在 Slice 0 中创建）

**需前端配合**（data-testid 补充）:
- `[data-testid='chat-message']`
- `[data-testid='message-done']`
- `[data-testid='mode-indicator']`
- `[data-testid='citation-button']`
- `[data-testid='history-item']`

### 依赖
- PR #1（Slice 0）已合并
- 前端 data-testid 已补充（可与本 PR 同批提交）

### 验收标准
```bash
npx playwright test --project=functional --grep "Workspace Chat"
# 期望：general chat + web search 两个 test 通过
```

---

## PR #4 — Slice 3: Document Upload + RAG

### 目标
扩展 workspace 旅程：上传文件 → ingestion 完成 → RAG 提问 → 验证 citation。

### 包含文件

**新增**:
```
frontend_next/e2e/specs/workspace-upload-rag.spec.ts
frontend_next/e2e/fixtures/sample-document.txt   # 测试用上传文件
```

**修改**:
```
frontend_next/e2e/pom/workspace-page.ts          # 已包含 uploadFile / waitForIngestionComplete
```

**需前端配合**（data-testid 补充）:
- `[data-testid='upload-done']`
- `[data-testid='ingestion-status']`

### 依赖
- PR #3（Slice 2）已合并

### 验收标准
```bash
npx playwright test --project=functional --grep "Upload"
# 期望：上传 → ingestion completed → RAG 回答 → citation 可交互
```

### 特殊注意
- ingestion timeout 默认 60s，CI 中可能更长，建议 `test.slow()`
- 断言要求 ingestion 状态必须为 `"completed"`，不接受 `"processing"` 或 `"queued"`

---

## PR #5 — Slice 4: Share & Collaboration

### 目标
验证 share 设置、访客只读访问。

### 包含文件

**新增**:
```
frontend_next/e2e/pom/share-page.ts
frontend_next/e2e/specs/workspace-share.spec.ts
```

### 依赖
- PR #4（Slice 3）已合并

### 验收标准
```bash
npx playwright test --project=functional --grep "Share"
# 期望：配置 share → 访客打开 → 只读验证通过
```

---

## PR #6 — Slice 5: 负面测试

### 目标
验证错误处理、网络降级场景。

### 包含文件

**新增**:
```
frontend_next/e2e/specs/auth-failure.spec.ts
frontend_next/e2e/specs/network-degradation.spec.ts
```

**修改**:
```
frontend_next/playwright.config.ts
# 新增 page.on('pageerror') 全局收集 console error
```

### 依赖
- PR #2（Slice 1）已合并
- PR #3（Slice 2）已合并

### 验收标准
```bash
npx playwright test --project=functional --grep "Auth Failure|Network"
# 期望：错误密码提示、空字段验证、slow 3G 下 chat 完成
```

---

## PR #7 — Slice 6: Cross-browser + Visual 回归

### 目标
迁移 visual 快照，启用 firefox/webkit cross-browser。

### 包含文件

**新增/迁移**:
```
frontend_next/e2e/specs/visual/*.spec.ts       # 从 avrag-rs/e2e/visual-ui.spec.ts 迁移
frontend_next/e2e/specs/visual/*.spec.ts-snapshots
```

**修改**:
```
.github/workflows/e2e-cross-browser.yml        # 已创建，启用 scheduled run
```

### 依赖
- PR #3（Slice 2）已合并（visual spec 依赖 workspace 稳定结构）

### 验收标准
```bash
npx playwright test --project=visual-desktop --update-snapshots
npx playwright test --project=cross-browser-firefox --project=cross-browser-webkit
# 期望：快照生成成功，cross-browser 无 core journey 失败
```

### 特殊注意
- 视觉迁移时统一重新生成 baseline 快照（一次性的 expected change）
- 避免在主 CI 上出现大量 diff 噪声

---

## PR #8 — Slice 7: Rust 后端 E2E 补全

### 目标
CI 补全 format_output/ingestion_answer，标记 avrag-rs/e2e 废弃。

### 包含文件

**修改**:
```
.github/workflows/e2e-staging.yml
# cargo test 步骤新增 --test e2e_format_output --test e2e_ingestion_answer
```

**新增**:
```
avrag-rs/e2e/README.md          # 标记冻结规则
```

**删除/标记**:
```
avrag-rs/e2e/debug-ui.spec.ts              # 零断言，直接删除
avrag-rs/playwright.config.ts              # 标记 deprecated（加注释）
```

### 依赖
- 与 Slice 1-4 并行开发，不阻塞前端

### 验收标准
```bash
cargo test --ignored -p app \
  --test e2e_chat --test e2e_rag --test e2e_search \
  --test e2e_format_output --test e2e_ingestion_answer \
  -- --test-threads=1
# 期望：全部通过
```

---

## 并行开发路线

```
Week 1
  Day 1-2: PR #1 (Slice 0) 提审 → 合并
  Day 3:   PR #2 (Slice 1) 提审 → 合并
  Day 4-5: PR #3 (Slice 2) 提审 → 合并
  并行:    PR #8 (Slice 7) 开发中

Week 2
  Day 1-2: PR #4 (Slice 3) 提审 → 合并
  Day 3:   PR #5 (Slice 4) 提审 → 合并
  Day 4-5: PR #6 (Slice 5) 提审 → 合并
  并行:    PR #8 (Slice 7) 提审 → 合并

Week 3
  Day 1:   PR #7 (Slice 6) 提审 → 合并
  Day 2-3: 整体回归测试 + 文档补全
```

---

## 环境变量清单（CI + 本地）

| 变量 | 用途 | 本地默认值 | CI 来源 |
|------|------|-----------|---------|
| `PLAYWRIGHT_BASE_URL` | 前端基础 URL | `http://127.0.0.1:3000` | `http://127.0.0.1:3000` |
| `E2E_TEST_USER_EMAIL` | 预置账号邮箱 | `e2e-test@example.com` | `e2e-test@example.com` |
| `E2E_TEST_USER_PASSWORD` | 预置账号密码 | `E2eTest123!` | secrets |
| `E2E_RESET_SECRET` | reset API secret | — | secrets |
| `E2E_INGESTION_TIMEOUT` | ingestion 超时（P2 配置化） | `60000` | env |

---

## 已生成代码与设计文档的对应关系

| 设计文档章节 | 已生成文件 |
|-------------|-----------|
| 3.1 Playwright 配置 | `frontend_next/playwright.config.ts` |
| 3.2 global-setup.ts | `frontend_next/e2e/global-setup.ts` |
| 3.3 setup-env.ts | `frontend_next/e2e/setup-env.ts` |
| 3.4 setup-auth.ts | `frontend_next/e2e/setup-auth.ts` |
| 3.5 fixtures/test-user.ts | `frontend_next/e2e/fixtures/test-user.ts` |
| 3.6 fixtures/run-context.ts | `frontend_next/e2e/fixtures/run-context.ts` |
| 3.7 utils/api-helpers.ts | `frontend_next/e2e/utils/api-helpers.ts` |
| 3.8 POM 设计 | `frontend_next/e2e/pom/*.ts` |
| 3.9 Specs 设计 | `frontend_next/e2e/specs/*.ts` |
| 5.1 CI workflow | `.github/workflows/e2e-staging.yml` |
| 5.2 Cross-browser workflow | `.github/workflows/e2e-cross-browser.yml` |
| PR 检查清单 | `docs/superpowers/specs/2026-06-03-product-e2e-redesign-PR-CHECKLIST.md` |
