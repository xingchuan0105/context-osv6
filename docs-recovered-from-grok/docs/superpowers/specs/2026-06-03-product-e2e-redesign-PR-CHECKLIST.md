# E2E 重构实施 PR 检查清单

> 对应设计文档：`2026-06-03-product-e2e-redesign-design.md`

---

## P0 — 必须在部署/实施前修复

### P0-1: global-setup.ts 单入口实现

**文件**: `frontend_next/e2e/global-setup.ts`
**状态**: ✅ 已生成
**关键代码**:
```typescript
export default async function globalSetup() {
  try { await setupEnv(); } catch (e) { console.error("[global-setup] setup-env failed:", e); throw e; }
  try { await setupAuth(); } catch (e) { console.error("[global-setup] setup-auth failed:", e); throw e; }
  console.log("[global-setup] env + auth ready");
}
```
**验证**: `npx playwright test --project=functional` 应成功运行，
`playwright/.auth/run-id.txt` 和 `playwright/.auth/user.json` 必须存在。

---

### P0-2: CI workflow project 名称与 config 完全一致

**文件**: `.github/workflows/e2e-staging.yml`
**修改**:
```yaml
# BEFORE
run: npx playwright test --project=e2e --project=auth

# AFTER
run: npx playwright test --project=functional --project=auth
```

**文件**: `.github/workflows/e2e-cross-browser.yml` (新增)
**修改**:
```yaml
run: npx playwright test --project=cross-browser-firefox --project=cross-browser-webkit
```

**文件**: `frontend_next/playwright.config.ts`
**确认**: projects 数组中的 `name` 字段必须与 CI 命令完全一致：
- `functional`
- `auth`
- `visual-desktop`
- `visual-mobile`
- `cross-browser-firefox`
- `cross-browser-webkit`

---

### P0-3: reset API 安全规则与后端实现契约

**文件**: 后端实现（不在本 PR 范围，需与后端同学确认）
**文档要求**: 在 `docs/superpowers/specs/2026-06-03-product-e2e-redesign-design.md` 4.3 节已定义 6 层安全 gate。
**必须确认项**:
- [ ] 环境变量名：`NODE_ENV`、`E2E_ENABLED`（具体值是什么？）
- [ ] Secret 长度和存放位置：CI secrets / 本地 `.env`
- [ ] 允许的 account pattern：`e2e-*` 前缀 或 `@test.local` 后缀
- [ ] 反向代理是否已有 IP 白名单
- [ ] 审计日志写入位置

**验证 curl**:
```bash
curl -X POST http://localhost:8080/api/e2e/reset-user-data \
  -H "X-E2E-Secret: ${E2E_RESET_SECRET}" \
  -H "Content-Type: application/json" \
  -d '{"email":"e2e-test@example.com"}'
# 期望：200 OK（staging），404/403（production）
```

---

## P1 — 高优先，应在实现前或早期实现时修正

### P1-1: runId 持久化可观测与过期策略

**文件**: `frontend_next/e2e/setup-env.ts`
**建议修改**:
```typescript
export default async function setupEnv() {
  const runId = `r${Date.now()}`;
  const authDir = "playwright/.auth";
  const runIdPath = `${authDir}/run-id.txt`;

  const fs = await import("fs");
  fs.mkdirSync(authDir, { recursive: true });

  // P1 增强：检测已有 runId，若存在且非 stale（< 10 分钟）则覆盖并警告
  if (fs.existsSync(runIdPath)) {
    const existing = fs.readFileSync(runIdPath, "utf-8").trim();
    const existingTs = parseInt(existing.slice(1), 10);
    const ageMin = (Date.now() - existingTs) / 60000;
    if (ageMin < 10) {
      console.warn(`[setup-env] existing runId ${existing} is only ${ageMin.toFixed(1)}min old, overwriting`);
    }
  }

  fs.writeFileSync(runIdPath, runId);
  // ...
}
```

---

### P1-2: fixtures/run-context 注入与 spec import 统一

**文件**: 所有 `frontend_next/e2e/specs/*.spec.ts`
**规则**:
```typescript
// ✅ 正确
import { test, expect } from "../fixtures/run-context";

// ❌ 错误（禁止）
import { test, expect } from "@playwright/test";
import { readRunId } from "../utils/api-helpers";
const runId = readRunId();
```

**例外**: `auth-flow.spec.ts` 使用空 storageState，必须从 `@playwright/test` 导入（已正确处理）。

---

### P1-3: functional 项目 testMatch 规则与新增 spec 规范

**文件**: `frontend_next/playwright.config.ts`
**当前规则**:
```typescript
testMatch: [/specs\/[^/]*\.spec\.ts/],  // 仅匹配 specs/ 根目录，不进入子目录
```

**PR 模板/文档应补充**:
```markdown
## 新增 spec 放置规则
- 功能测试 → `e2e/specs/` 根目录（会被 `functional` 和 `cross-browser-*` 捕获）
- 视觉回归 → `e2e/specs/visual/`（会被 `visual-desktop` 和 `visual-mobile` 捕获）
- 认证流程 → `e2e/specs/auth-flow.spec.ts`（会被 `auth` 捕获）
- 命名规范：`*.{scope}.spec.ts`，如 `workspace-chat.spec.ts`
```

---

## P2 — 可选但建议实施

### P2-1: 可观测日志与失败上下文

**文件**: `frontend_next/e2e/global-setup.ts`、`setup-env.ts`、`setup-auth.ts`
**已部分实现**: console.log/error 已添加。
**建议增强**:
- CI 失败时上传 `playwright/.auth/` 目录作为 artifact（含 run-id.txt）
- setup-auth 失败时截图保存到 `playwright-report/setup-auth-failure.png`

### P2-2: 断言策略（结构性优先）

**已采纳**: workspace-chat.spec.ts 中已使用 `message-done`、`mode-indicator`、`citation-button` 等结构性断言。
**建议**: 在 PR 描述中注明"本 PR 不验证 LLM 生成内容的语义正确性，只验证 UI 结构和交互完整性"。

### P2-3: ingestion timeout 配置化

**文件**: `frontend_next/e2e/pom/workspace-page.ts`
**建议**:
```typescript
// 从环境变量读取，CI 中可覆盖
const INGESTION_TIMEOUT = parseInt(process.env.E2E_INGESTION_TIMEOUT || "60000", 10);
```

### P2-4: visual snapshot baseline 策略

**建议**: 新增 `docs/e2e-visual-baseline.md`，规定：
- baseline 更新必须在独立分支进行
- 通过 `npx playwright test --project=visual-desktop --update-snapshots` 生成
- PR 中须附 before/after 截图对比

---

## 文件变更总览

### 新增文件
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

### 修改文件
```
frontend_next/playwright.config.ts          # 重写
.github/workflows/e2e-staging.yml           # 新增 frontend 步骤、修正 project 名
```

### 待删除文件（Slice 1 完成时）
```
frontend_next/e2e/login-hydration.spec.ts   # 逻辑已迁入 auth-flow.spec.ts
```

### 待废弃文件（Slice 7 完成时）
```
avrag-rs/e2e/debug-ui.spec.ts              # 零断言，直接删除
avrag-rs/e2e/rust-frontend-e2e.spec.ts     # Group 5 浏览器旅程已迁移
avrag-rs/e2e/visual-ui.spec.ts             # 已迁移到 frontend_next
avrag-rs/playwright.config.ts              # 标记 deprecated
```

---

## 复查通过判定（验收检查清单）

- [ ] `global-setup.ts` 存在且能在本地/CI 顺序执行成功（runId 文件与 user.json 存在）
- [ ] workflow 中引用的 project 名称与 config 一致（functional/auth/…）
- [ ] reset API 实现与文档安全约束一致且 ops 已同意
- [ ] 一个 sample spec（workspace-chat）在本地能跑通并产生 trace + storageState artifact
- [ ] PR 模板/README 已包含"新增 spec 放置规则"与"如何更新视觉 baseline"说明
