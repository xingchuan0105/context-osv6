# 产品级E2E测试重构设计文档

**日期**: 2026-06-03  
**主题**: 整合12项修复，将分散的E2E测试重构为以真实用户旅程为核心的垂直切片体系  
**范围**: `frontend_next/e2e/`、`avrag-rs/e2e/`、`avrag-rs/crates/app/tests/e2e_*.rs`、`.github/workflows/e2e-staging.yml`

---

## 1. 背景与目标

### 1.1 当前问题

现有E2E测试存在以下结构性缺陷：

- **前端E2E极度匮乏**：`frontend_next/e2e/` 仅2个测试，大量核心用户旅程（workspace chat、文档上传、share设置）完全缺失。
- **测试路径不等价于人工测试**：`seedBrowserAuth` 直接注入localStorage绕过登录流程；文件上传直接 `PUT /dev-upload/{id}` 绕过前端组件；notebook创建全部走API。
- **断言过于宽松**：文档上传测试接受 `completed|failed|queued|processing` 任意状态即通过；RAG citation测试默认被跳过（`E2E_STRICT_CITATIONS` 未设置）。
- **测试架构分裂**：两套Playwright配置（frontend_next:3000 与 avrag-rs:8080），浏览器旅程分散在两个仓库位置。
- **CI覆盖不完整**：不运行 `frontend_next` 的Playwright测试；Rust E2E排除 `e2e_format_output` 和 `e2e_ingestion_answer`。
- **debug-ui.spec.ts 不是测试**：零断言，仅为调试脚本。

### 1.2 目标

**核心原则**：E2E测试的通过必须等价于"从前端操作的人工测试也不会出问题"。

具体目标（映射12项修复）：

| # | 修复项 | 目标状态 |
|---|--------|---------|
| 1 | T05文档上传断言过松 | 要求ingestion状态必须为 `"completed"` |
| 2 | debug-ui.spec.ts不是测试 | **删除** |
| 3 | CI不跑前端E2E | CI新增 `frontend_next` Playwright步骤 |
| 4 | login-hydration硬编码URL | 使用 `baseURL` 配置 |
| 5 | 前端缺少核心旅程 | 补充注册→登录→workspace→chat→websearch完整流 |
| 6 | seedBrowserAuth绕登录 | `globalSetup` 走真实登录，`storageState` 共享 |
| 7 | RAG citation默认跳过 | 默认启用citation验证，不依赖环境变量开关 |
| 8 | format_output/ingestion_answer不在CI | CI新增对应cargo test步骤 |
| 9 | 两套Playwright分裂 | 前端UI统一在 `frontend_next/e2e/`，Rust只保留纯API/策略测试 |
| 10 | 缺少POM | 引入 `pom/` 目录封装页面 |
| 11 | 缺少负面测试 | 补充错误密码、网络降级等场景 |
| 12 | 缺少cross-browser | 新增firefox/webkit projects（默认只跑chromium） |

---

## 2. 总体架构

### 2.1 核心决策

| 维度 | 决策 |
|------|------|
| 前端E2E入口 | 统一在 `frontend_next/e2e/`，baseURL = `:3000` |
| 后端API/策略E2E | 保留在 `avrag-rs/crates/app/tests/e2e_*.rs`，**移除所有浏览器测试** |
| 认证模式 | `globalSetup` **仅**负责环境准备（reset数据→真实登录→保存storageState）；`auth-flow.spec.ts` 作为独立业务spec验证注册/登录功能，两者**不互相依赖** |
| 账号策略 | 预置账号 + **run-scoped数据命名空间**：每个test run生成唯一前缀，notebook/session名称均带前缀，实现软隔离；spec级`beforeAll`可选reset |
| 并发 | 保留 `workers: 1`，run-scoped命名空间已为后续按worker隔离并行化预留接口 |
| API shortcut边界 | **仅** `setup`/`teardown` 允许API调用；用户可见的操作必须走UI |

### 2.2 目标目录结构

```
frontend_next/
├── e2e/
│   ├── global-setup.ts              # 预登录，保存 storageState
│   ├── fixtures/
│   │   └── test-user.ts             # 预置账号配置 + API重置工具
│   ├── pom/
│   │   ├── login-page.ts            # 登录/注册页POM
│   │   ├── dashboard-page.ts        # Dashboard页POM
│   │   └── workspace-page.ts        # Workspace页POM（chat、sources、upload等）
│   ├── specs/
│   │   ├── auth-flow.spec.ts        # 注册→登录→登出（真实流，不继承storageState）
│   │   ├── workspace-chat.spec.ts   # MVP：dashboard→创建workspace→chat→websearch
│   │   ├── workspace-upload-rag.spec.ts   # 上传→ingestion→RAG→citation
│   │   ├── workspace-share.spec.ts        # share设置→链接验证→权限
│   │   ├── auth-failure.spec.ts           # 负面：错误密码、空字段
│   │   ├── network-degradation.spec.ts    # 负面：slow 3G、offline
│   │   └── visual/
│   │       └── *.spec.ts            # 视觉回归快照（从avrag-rs迁移）
│   └── utils/
│       └── api-helpers.ts           # 仅用于setup/teardown的API调用
├── playwright.config.ts             # 统一配置
└── ...existing code...

avrag-rs/
├── e2e/
│   ├── README.md                    # 冻结声明：只读，不再修改
│   └── (原有文件冻结，Slice 7完成后30天内删除)
├── crates/app/tests/
│   ├── e2e_chat.rs                  # 保留：纯策略状态机测试
│   ├── e2e_rag.rs                   # 保留：纯策略状态机测试
│   ├── e2e_search.rs                # 保留：纯策略状态机测试
│   ├── e2e_format_output.rs         # CI中加入
│   ├── e2e_ingestion_answer.rs      # CI中加入
│   └── e2e/                         # 共享辅助模块
└── tests/api-contract/              # 从avrag-rs/e2e/迁移的纯API测试
    └── (原rust-frontend-e2e.spec.ts中的API测试)
```

---

## 3. 前端E2E详细设计

### 3.1 Playwright统一配置

```typescript
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  timeout: 90_000,
  fullyParallel: false,
  workers: 1,
  // globalSetup 对外为单入口（兼容 Playwright API：string 而非 array[]），
  // 内部在 global-setup.ts 中串行调用 setupEnv() + setupAuth()。
  // 拆分原因：环境准备失败与认证失败的责任清晰分离；运维能单独重跑某一步。
  globalSetup: "./e2e/global-setup.ts",
  reporter: "list",

  webServer: [
    {
      // 注意：--bin 后的名称需替换为实际的Rust server binary名称
      command: "cd ../avrag-rs && cargo run --bin <avrag-server-binary-name>",
      url: "http://127.0.0.1:8080/health",
      timeout: 120_000, // 初始建议值，需根据CI观测校准
      reuseExistingServer: !process.env.CI,
    },
    {
      // 统一由 Playwright webServer 启动前端；CI用 build+start，本地用 dev
      command: process.env.CI ? "pnpm build && pnpm start" : "pnpm dev",
      url: "http://127.0.0.1:3000",
      timeout: 60_000, // 初始建议值，需根据CI观测校准
      reuseExistingServer: !process.env.CI,
    },
  ],

  projects: [
    {
      name: "functional",
      // 只匹配 specs/ 根目录下的 .spec.ts（不进入子目录），排除 auth-flow
      testMatch: [/specs\/[^/]*\.spec\.ts/],
      testIgnore: [/auth-flow\.spec\.ts/],
      use: {
        baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
        locale: "zh-CN",
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
        storageState: "playwright/.auth/user.json",
      },
    },
    {
      name: "auth",
      testMatch: [/auth-flow\.spec\.ts/],
      use: {
        baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
        locale: "zh-CN",
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
        storageState: { cookies: [], origins: [] },
      },
    },
    {
      name: "visual-desktop",
      testMatch: [/visual\/.*\.spec\.ts/],
      use: {
        browserName: "chromium",
        viewport: { width: 1440, height: 900 },
        storageState: "playwright/.auth/user.json",
        trace: "off",
        screenshot: "off",
        video: "off",
      },
    },
    {
      name: "visual-mobile",
      testMatch: [/visual\/.*\.spec\.ts/],
      use: {
        ...devices["Pixel 5"],
        storageState: "playwright/.auth/user.json",
        trace: "off",
        screenshot: "off",
        video: "off",
      },
    },
    {
      name: "cross-browser-firefox",
      testMatch: [/specs\/[^/]*\.spec\.ts/],
      testIgnore: [/auth-flow\.spec\.ts/],
      use: {
        browserName: "firefox",
        storageState: "playwright/.auth/user.json",
      },
    },
    {
      name: "cross-browser-webkit",
      testMatch: [/specs\/[^/]*\.spec\.ts/],
      testIgnore: [/auth-flow\.spec\.ts/],
      use: {
        browserName: "webkit",
        storageState: "playwright/.auth/user.json",
      },
    },
  ],
});
```

> **Cross-browser策略**：`cross-browser-firefox` 和 `cross-browser-webkit` 项目默认**不**在CI中运行（节省时间和成本）。通过 `npx playwright test --project=cross-browser-firefox` 手动触发，或在weekly regression workflow中启用。

### 3.2 global-setup.ts（对外单入口）

```typescript
import setupEnv from "./setup-env";
import setupAuth from "./setup-auth";

/**
 * globalSetup 对外单入口（兼容 Playwright 的 string 类型 API）。
 * 内部按顺序调用 setupEnv → setupAuth，职责分离但对外保持单一入口。
 */
export default async function globalSetup() {
  await setupEnv();
  await setupAuth();
}
```

### 3.3 setup-env.ts（环境准备，无浏览器）

```typescript
import { resetTestUserData } from "./utils/api-helpers";

/**
 * setup-env 职责：纯环境准备，无浏览器参与。
 *   1. 生成 runId 并持久化到磁盘（供所有 spec 和后续 setup-auth 复用）
 *   2. 重置预置账号数据（清空上一 run 的残留）
 *
 * 设计理由：这一步完全不需要浏览器。如果 runId 生成或 reset 失败，
 * 失败原因明确归为"环境/后端"问题，而不是"登录流程"问题。
 */
export default async function setupEnv() {
  const runId = `r${Date.now()}`;

  const fs = await import("fs");
  fs.mkdirSync("playwright/.auth", { recursive: true });
  fs.writeFileSync("playwright/.auth/run-id.txt", runId);

  // 使用 Playwright 的 request 对象（无需浏览器）调用 reset API
  const { request } = await import("@playwright/test");
  const reqCtx = await request.newContext({
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
  });
  await resetTestUserData(reqCtx);
  await reqCtx.dispose();
}
```

### 3.4 setup-auth.ts（认证准备，有浏览器）

```typescript
import { chromium } from "@playwright/test";
import { TEST_USER } from "./fixtures/test-user";

/**
 * setup-auth 职责：仅负责浏览器登录，生成 storageState。
 *
 * 注意：setup-auth 不验证任何业务功能（如注册表单是否正常）。
 * 业务功能验证由独立的 auth-flow.spec.ts 负责。
 * 两者完全解耦，避免"登录失败"与"业务测试失败"相互混淆。
 */
export default async function setupAuth() {
  const browser = await chromium.launch();
  const page = await browser.newPage({
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
  });

  await page.goto("/login");
  await page.locator("#login-email").fill(TEST_USER.email);
  await page.locator("#login-password").fill(TEST_USER.password);
  await page.getByRole("button", { name: /继续登录/ }).click();
  await page.waitForURL(/\/dashboard$/);

  await page.context().storageState({ path: "playwright/.auth/user.json" });
  await browser.close();
}
```

### 3.5 fixtures/test-user.ts

```typescript
export const TEST_USER = {
  email: process.env.E2E_TEST_USER_EMAIL || "e2e-test@example.com",
  password: process.env.E2E_TEST_USER_PASSWORD || "E2eTest123!",
  fullName: "E2E Test User",
};
```

### 3.6 fixtures/run-context.ts

```typescript
import { test as base } from "@playwright/test";
import { readRunId } from "../utils/api-helpers";

/**
 * 扩展 Playwright test，注入 runId fixture。
 *
 * 用法：
 *   import { test, expect } from "../fixtures/run-context";
 *   test("...", async ({ page, runId }) => { ... });
 *
 * 设计理由：runId 是测试运行的上下文信息，不是业务数据。
 * 通过 fixture 注入比每个 spec 手动调用 readRunId() 更清晰，
 * 也便于未来扩展（如 worker-scoped runId、并行账号分配等）。
 */
export const test = base.extend<{
  runId: string;
}>({
  runId: async ({}, use) => {
    await use(readRunId());
  },
});

export { expect } from "@playwright/test";
```

### 3.7 utils/api-helpers.ts

```typescript
import { type APIRequestContext } from "@playwright/test";
import { TEST_USER } from "../fixtures/test-user";

/**
 * 重置预置测试账号的所有数据。
 * 仅用于 setup/teardown，不用于测试断言。
 */
export async function resetTestUserData(request: APIRequestContext) {
  const resp = await request.post("/api/e2e/reset-user-data", {
    headers: { "X-E2E-Secret": process.env.E2E_RESET_SECRET! },
    data: { email: TEST_USER.email },
  });
  if (!resp.ok()) {
    throw new Error(`reset-user-data failed: ${resp.status()} ${await resp.text()}`);
  }
}

/**
 * 读取 setup-env.ts 生成的 runId（run-scoped 隔离标识）。
 *
 * 注意：spec 中优先通过 run-context fixture 注入 runId，而非直接调用本函数。
 * 本函数保留作为底层工具，供 fixture 和 setup 文件使用。
 */
export function readRunId(): string {
  const fs = require("fs");
  return fs.readFileSync("playwright/.auth/run-id.txt", "utf-8").trim();
}

/**
 * 为 notebook/session 等命名，确保 run-scoped 隔离。
 */
export function runScopedName(label: string, runId: string): string {
  return `${label} ${runId}`;
}

/**
 * 通过API删除指定notebook。
 * 仅用于 afterAll 清理，不用于测试断言。
 *
 * 依赖：Playwright的 `request` fixture会自动携带storageState中的认证cookie。
 * 如果后端使用JWT Bearer Token而非session cookie，需在setup-env.ts中将token
 * 写入文件（如 `playwright/.auth/token.txt`），本函数读取后附加到header。
 */
export async function deleteWorkspaceViaAPI(
  request: APIRequestContext,
  notebookId: string,
) {
  if (!notebookId) return;
  await request.delete(`/api/v1/notebooks/${notebookId}`);
}
```

### 3.8 POM设计

#### LoginPage

```typescript
import { type Page, type Locator, expect } from "@playwright/test";

export class LoginPage {
  readonly emailInput: Locator;
  readonly passwordInput: Locator;
  readonly submitButton: Locator;
  readonly errorMessage: Locator;

  constructor(private page: Page) {
    this.emailInput = page.locator("#login-email");
    this.passwordInput = page.locator("#login-password");
    this.submitButton = page.getByRole("button", { name: /继续登录/ });
    this.errorMessage = page.locator("[data-testid='login-error']");
  }

  async goto() {
    await this.page.goto("/login");
    await expect(this.page.locator("form").first()).toBeVisible();
  }

  async login(email: string, password: string) {
    await this.goto();
    await this.emailInput.fill(email);
    await this.passwordInput.fill(password);
    await expect(this.submitButton).toBeEnabled();
    await this.submitButton.click();
    await this.page.waitForURL(/\/dashboard$/);
  }
}
```

#### DashboardPage

```typescript
import { type Page, expect } from "@playwright/test";

export class DashboardPage {
  constructor(private page: Page) {}

  async createWorkspace(name: string) {
    await this.page.getByRole("button", { name: /新建/ }).click();
    await this.page.getByPlaceholder(/名称/).fill(name);
    await this.page.getByRole("button", { name: /确认/ }).click();
    await expect(this.page.locator("text=" + name)).toBeVisible();
  }

  async openWorkspace(name: string) {
    await this.page.locator("text=" + name).first().click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);
  }

  getWorkspaceList() {
    return this.page.locator("[data-testid='notebook-list']");
  }
}
```

#### WorkspacePage

```typescript
import { type Page, expect } from "@playwright/test";

export class WorkspacePage {
  constructor(private page: Page) {}

  async sendMessage(text: string) {
    const input = this.page.getByPlaceholder(/围绕当前资料继续研究|Ask a question/);
    await input.fill(text);
    await this.page.getByRole("button", { name: /发送|Send/ }).click();
  }

  async waitForResponse(timeout = 30_000) {
    await this.page.waitForSelector("[data-testid='message-done']", { timeout });
  }

  getMessages() {
    return this.page.locator("[data-testid='chat-message']").all();
  }

  getLastMessage() {
    return this.page.locator("[data-testid='chat-message']").last();
  }

  async switchToWebSearchMode() {
    await this.page.getByRole("button", { name: /联网搜索|Web Search/ }).click();
    await expect(this.page.locator("[data-testid='mode-indicator']")).toContainText(/search|联网/i);
  }

  async switchToHistoryTab() {
    await this.page.getByRole("button", { name: /历史|History/ }).click();
  }

  async uploadFile(filePath: string) {
    const input = this.page.locator("input[type='file']");
    await input.setInputFiles(filePath);
    await expect(this.page.locator("[data-testid='upload-done']")).toBeVisible({ timeout: 10_000 }); // 初始建议值，需根据CI观测校准
  }

  async waitForIngestionComplete(timeout = 60_000) { // 初始建议值，需根据CI观测校准
    await expect(this.page.locator("[data-testid='ingestion-status']")).toHaveText(/completed|已完成/, { timeout });
  }

  getCitationButton() {
    return this.page.locator("[data-testid='citation-button']").first();
  }
}
```

### 3.9 Specs设计

#### auth-flow.spec.ts

```typescript
import { test, expect } from "@playwright/test";
import { LoginPage } from "../pom/login-page";
import { TEST_USER } from "../fixtures/test-user";

/**
 * auth-flow.spec.ts 职责：验证注册/登录/登出的业务功能。
 *
 * 注意：setup-env + setup-auth 已独立完成环境准备（reset + login + storageState）。
 * 本spec不依赖 setup 的执行结果，也不被任何 project 依赖。
 * 使用空 storageState 确保每次测试都是"未登录"的干净状态。
 */
test.use({ storageState: { cookies: [], origins: [] } });

test.describe("Auth Flow", () => {
  test("user can register and login via UI", async ({ page }) => {
    const email = `e2e-${Date.now()}@test.local`;

    // Register
    await page.goto("/register");
    await page.locator("#register-email").fill(email);
    await page.locator("#register-password").fill("E2eTest123!");
    await page.locator("#register-name").fill("E2E User");
    await page.getByRole("button", { name: /注册/ }).click();
    await page.waitForURL(/\/login$/);

    // Login
    const login = new LoginPage(page);
    await login.login(email, "E2eTest123!");

    // Verify
    await expect(page).toHaveURL(/\/dashboard$/);
  });

  test("login page does not submit before hydration", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false, locale: "zh-CN" });
    const page = await context.newPage();

    try {
      await page.goto("/login");
      await page.locator("#login-email").fill("prehydrate@example.com");
      await page.locator("#login-password").fill("E2eTest123!");
      await page.locator("#login-password").press("Enter");
      await expect(page).toHaveURL(/\/login$/);
    } finally {
      await context.close();
    }
  });
});
```

#### workspace-chat.spec.ts（MVP垂直切片）

```typescript
import { test, expect } from "../fixtures/run-context";
import { DashboardPage } from "../pom/dashboard-page";
import { WorkspacePage } from "../pom/workspace-page";
import { resetTestUserData, runScopedName } from "../utils/api-helpers";

test.describe("Workspace Chat Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("user can create notebook, chat in general mode, and view history", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    await page.goto("/dashboard");

    const notebookName = runScopedName("E2E Chat", runId);
    await dashboard.createWorkspace(notebookName);
    await dashboard.openWorkspace(notebookName);

    // General chat
    await workspace.sendMessage("What is the capital of France?");
    await workspace.waitForResponse();

    // 结构性断言（优先）：消息完成标记存在、消息非空
    const lastMessage = workspace.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    // Verify history persisted — 断言包含当前runId前缀的条目存在
    await workspace.switchToHistoryTab();
    await expect(page.locator(`[data-testid='history-item']:has-text("${runId}")`)).toBeVisible();
  });

  test("user can switch to web search mode and get search-grounded answer", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    await page.goto("/dashboard");

    const notebookName = runScopedName("E2E WebSearch", runId);
    await dashboard.createWorkspace(notebookName);
    await dashboard.openWorkspace(notebookName);

    await workspace.switchToWebSearchMode();

    // 结构性断言（优先）：mode indicator显示正确模式
    await expect(page.locator("[data-testid='mode-indicator']")).toContainText(/search|联网/i);

    await workspace.sendMessage("What is the latest Rust release?");
    await workspace.waitForResponse();

    // 结构性断言（优先）：消息完成、消息非空、citation按钮可见
    const lastMessage = workspace.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    const citationButton = workspace.getCitationButton();
    await expect(citationButton).toBeVisible();
  });
});
```

---

## 4. 后端E2E调整

### 4.1 Rust浏览器测试迁移/废弃

| 原文件 | 决策 | 目的地 |
|--------|------|--------|
| `rust-frontend-e2e.spec.ts` Group 1-4 (API测试) | **保留并迁移** | `avrag-rs/tests/api-contract/` |
| `rust-frontend-e2e.spec.ts` Group 5 (Browser Journeys) | **重写并迁移** | `frontend_next/e2e/specs/` |
| `visual-ui.spec.ts` | **迁移** | `frontend_next/e2e/specs/visual/` |
| `debug-ui.spec.ts` | **删除** | — |

### 4.2 Rust策略E2E断言收紧

- `e2e_rag.rs` 中移除 `test.skip(!STRICT_CITATION_MODE)` 模式。citation是RAG的核心产品承诺，不是可选功能。
- `e2e_ingestion_answer.rs` 中 `uploadDocumentAndWait` 的断言要求状态必须为 `"completed"`。如果staging环境embedding服务不可用，测试应失败并报警，而不是默默通过。

### 4.3 后端新增API端点（高安全约束）

```
POST /api/e2e/reset-user-data
Headers: X-E2E-Secret: <secret>
Body: { "email": "e2e-test@example.com" }
```

**多层安全约束（缺一不可）**：

| 层级 | 约束 | 说明 |
|------|------|------|
| 环境 gates | `NODE_ENV !== "production"` **且** `E2E_ENABLED === "true"` | 双重否定：生产环境**绝对**不注册；非生产环境也需显式开启 |
| 网络 gates | 仅监听 staging 私网 / loopback | 不暴露在公网入口；如有反向代理，需额外IP白名单 |
| Secret gates | `X-E2E-Secret` 必须匹配 `E2E_RESET_SECRET` | 128-bit随机字符串，存储于CI secrets |
| 账号前缀 gates | 只允许操作 `e2e-*` 前缀或 `@test.local` 后缀的账号 | 防止误删真实用户数据 |
| 范围 gates | 仅删除该用户名下数据，不删除账号本身 | 保留账号用于复用登录 |
| 审计 gates | 每次调用记录 audit log（调用时间、来源IP、目标账号） | 用于事后追溯 |

**操作范围**：删除目标用户的 notebooks、documents、chat sessions、share tokens、upload records。不删除用户账号、不删除其他用户数据、不删除系统配置。

---

## 5. CI/CD集成

### 5.1 更新后的 `.github/workflows/e2e-staging.yml`

```yaml
name: E2E Staging Tests

on:
  workflow_dispatch:
  schedule:
    - cron: '17 2 * * *'

concurrency:
  group: e2e-staging
  cancel-in-progress: true

jobs:
  e2e-tests:
    name: E2E Full Suite
    runs-on: ubuntu-latest
    timeout-minutes: 45

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            avrag-rs/target/
          key: ${{ runner.os }}-cargo-e2e-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-e2e-

      - name: Install Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Install pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 10.32.1

      - name: Install frontend dependencies
        working-directory: frontend_next
        run: pnpm install

      - name: Install Playwright browsers
        working-directory: frontend_next
        run: npx playwright install chromium

      - name: Run Rust Strategy E2E
        working-directory: avrag-rs
        env:
          E2E_LLM_BASE_URL: ${{ secrets.E2E_LLM_BASE_URL }}
          E2E_LLM_API_KEY: ${{ secrets.E2E_LLM_API_KEY }}
          E2E_LLM_MODEL: gpt-4o-mini
          E2E_BRAVE_API_KEY: ${{ secrets.E2E_BRAVE_API_KEY }}
          E2E_MILVUS_URL: ${{ secrets.E2E_MILVUS_URL }}
          E2E_MILVUS_TOKEN: ${{ secrets.E2E_MILVUS_TOKEN }}
          E2E_EMBEDDING_BASE_URL: ${{ secrets.E2E_EMBEDDING_BASE_URL }}
          E2E_EMBEDDING_API_KEY: ${{ secrets.E2E_EMBEDDING_API_KEY }}
          E2E_EMBEDDING_MODEL: text-embedding-3-small
        run: |
          cargo test --ignored -p app \
            --test e2e_chat --test e2e_rag --test e2e_search \
            --test e2e_format_output --test e2e_ingestion_answer \
            -- --test-threads=1

      - name: Run Frontend E2E
        working-directory: frontend_next
        env:
          PLAYWRIGHT_BASE_URL: http://127.0.0.1:3000
          E2E_TEST_USER_EMAIL: e2e-test@example.com
          E2E_TEST_USER_PASSWORD: ${{ secrets.E2E_TEST_USER_PASSWORD }}
          E2E_RESET_SECRET: ${{ secrets.E2E_RESET_SECRET }}
        run: npx playwright test --project=functional --project=auth

      - name: Upload frontend test results
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: playwright-report
          path: frontend_next/playwright-report/
          retention-days: 7

      - name: Upload Rust test output
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-test-output
          path: avrag-rs/target/debug/deps/e2e_*
          retention-days: 7
```

### 5.2 新增Weekly Cross-Browser Workflow（可选）

```yaml
# .github/workflows/e2e-cross-browser.yml
name: E2E Cross-Browser Weekly
on:
  schedule:
    - cron: '0 3 * * 0'  # 每周日
  workflow_dispatch:

jobs:
  cross-browser:
    runs-on: ubuntu-latest
    steps:
      # ... setup same as above ...
      - name: Run Frontend E2E (all browsers)
        working-directory: frontend_next
        run: npx playwright test --project=cross-browser-firefox --project=cross-browser-webkit
```

---

## 6. 测试运行模型与隔离模型

### 6.1 运行模型（Run Model）

```
┌─────────────────────────────────────────────────────────────┐
│  Test Run（一次 npx playwright test 调用）                    │
│                                                             │
│  1. global-setup.ts（一次性）                                │
│     ├── setup-env.ts（无浏览器）                             │
│     │   ├── generate runId → playwright/.auth/run-id.txt    │
│     │   └── reset-user-data（清空预置账号的所有数据）          │
│     └── setup-auth.ts（有浏览器）                            │
│         └── UI login → save storageState → playwright/.auth/│
│                                                             │
│  2. Specs 串行执行（workers=1）                              │
│     ├── spec A: { runId } fixture → UI journey → afterAll   │
│     ├── spec B: { runId } fixture → UI journey → afterAll   │
│     └── ...                                                 │
│                                                             │
│  3. 所有spec共享同一登录态（storageState）和同一runId        │
│     数据隔离通过 run-scoped 命名空间实现（非账号隔离）        │
└─────────────────────────────────────────────────────────────┘
```

**关键原则**：global-setup.ts 只负责"环境准备"（通过 setup-env 生成 runId、reset 数据；通过 setup-auth 保存 storageState），不验证任何业务功能。业务功能验证由各个 spec 独立完成。

### 6.2 隔离模型（Isolation Model）

即使 `workers: 1`，也采用 **run-scoped 软隔离**，避免spec间数据串扰：

| 隔离维度 | 机制 | 示例 |
|----------|------|------|
| **Run ID** | 每个test run生成唯一 `runId = r${Date.now()}` | `r1717363200000` |
| **Workspace命名** | `runScopedName(label, runId)` | `"E2E Chat r1717363200000"` |
| **Session区分** | notebook名称自带runId，间接隔离session | 不同run的chat session不重叠 |
| **Reset时机** | setup-env.ts在run开始时全局reset一次 | 清空上一run的所有残留数据 |
| **可选spec级reset** | 个别spec的 `beforeAll` 可再次reset | 当spec产生大量垃圾数据时 |

**为什么不每个spec都reset？**  
全局reset在setup-env中执行一次成本最低。如果每个spec都reset，测试时间随spec数线性增长。run-scoped命名空间确保即使不reset，数据也不会冲突。

**未来并行扩展（workers > 1）**：  
在run-scoped基础上，为每个worker分配独立预置账号：

```typescript
const workerIndex = process.env.TEST_PARALLEL_INDEX || "0";
const email = `e2e-test-${workerIndex}@example.com`;
const storageStatePath = `playwright/.auth/user-${workerIndex}.json`;
```

### 6.3 数据清理策略

```
global-setup.ts (once per test run)
  ├── setup-env.ts
  │   ├── generate runId → playwright/.auth/run-id.txt
  │   └── POST /api/e2e/reset-user-data  → 清空预置账号的所有数据（全局reset）
  └── setup-auth.ts
      └── UI login on /login             → 保存 storageState → 所有spec复用

spec beforeAll (可选，极少使用)
  └── 再次 resetTestUserData(request)
      （仅当该spec会产生极大量数据且影响后续spec性能时）

spec afterAll (推荐)
  └── deleteWorkspaceViaAPI(request, notebookId)
      （清理当前spec显式创建的notebook，保持数据库轻量）
```

### 6.4 API shortcut边界（重要）

| 操作 | 允许在E2E中使用API？ | 说明 |
|------|---------------------|------|
| 重置/清理测试数据 | ✅ 仅setup/teardown | 用户不可见的后台操作 |
| 用户注册 | ❌ 必须通过UI完成 | auth-flow.spec走注册表单，不允许直接POST /api/auth/register |
| 用户登录 | ❌ 必须通过UI完成 | setup-auth走登录表单保存storageState；其他spec复用storageState，不再登录 |
| 创建notebook | ❌ 必须通过UI完成 | 必须走DashboardPage.createWorkspace |
| 上传文件 | ❌ 必须通过UI完成 | 必须走WorkspacePage.uploadFile |
| 发送chat消息 | ❌ 必须通过UI完成 | 必须走WorkspacePage.sendMessage |
| 删除notebook（清理） | ✅ 仅afterAll | 验证已完成后的清理 |

---

## 7. 实施路线图

### 依赖关系

```
Slice 0 (基础设施)
    │
    ├──→ Slice 1 (Auth Flow)
    │       │
    │       └──→ Slice 2 (Workspace Chat + WebSearch)
    │                       │
    │                       └──→ Slice 3 (Upload + RAG)
    │                                       │
    │                                       └──→ Slice 4 (Share)
    │
    └──→ Slice 7 (Rust CI补全 + 废弃标记)
                │
                └── 与Slice 1-4并行开发

Slice 5 (负面测试) ── 依赖Slice 1+2
Slice 6 (Cross-browser + Visual) ── 依赖Slice 2
```

### Slice 0：基础设施（1-2天）

- 新建 `frontend_next/e2e/` 目录骨架（pom/, fixtures/, utils/, specs/）
- 重写 `playwright.config.ts`（globalSetup、projects、webServer）
- 新建 `global-setup.ts`（对外单入口，串行调用 setup-env + setup-auth）
- 新建 `setup-env.ts`（runId + reset，无浏览器）
- 新建 `setup-auth.ts`（登录 + storageState，有浏览器）
- 新建 `fixtures/test-user.ts`
- 新建 `fixtures/run-context.ts`（runId fixture 注入）
- 新建 `utils/api-helpers.ts`
- **后端**：新增 `/api/e2e/reset-user-data` 端点（staging only，带secret验证）
- 修复 `login-hydration.spec.ts` 硬编码URL（临时修复，后续Slice 1完全重写）

**验收标准**：`npx playwright test --project=functional` 成功运行，setup-env自动完成reset，setup-auth自动完成login并保存storageState。

### Slice 1：Auth Flow（1天）

- 新建 `pom/login-page.ts`
- 新建 `specs/auth-flow.spec.ts`（注册→登录→登出）
- 迁移原 `login-hydration.spec.ts` 逻辑（hydration测试迁入auth-flow.spec.ts）
- 删除旧的 `frontend_next/e2e/login-hydration.spec.ts`

**验收标准**：auth-flow.spec.ts在CI中通过，注册和登录都是真实UI操作。

### Slice 2：Workspace Chat + WebSearch（2天）

- 新建 `pom/dashboard-page.ts`
- 新建 `pom/workspace-page.ts`（含sendMessage、waitForResponse、switchToWebSearchMode）
- 新建 `specs/workspace-chat.spec.ts`
  - Test A：general chat（创建notebook→发送消息→验证回答→验证历史）
  - Test B：web search mode（切换模式→发送消息→验证回答含搜索结果→验证citation按钮可见）
- 前端补充必要的 `data-testid`（chat-message、message-done、mode-indicator、citation-button）

**验收标准**：两个test都在CI中通过，消息发送/渲染完全走UI，websearch模式验证搜索结果引用。

### Slice 3：Document Upload + RAG（2-3天）

- 扩展 `WorkspacePage`（uploadFile、waitForIngestionComplete）
- 新建 `specs/workspace-upload-rag.spec.ts`
  - 上传真实txt文件
  - **断言要求**：ingestion状态必须为 `"completed"`，否则失败
  - RAG提问：验证答案含文档相关内容
  - 验证citation modal可打开并显示来源段落
- 前端补充 `data-testid`（upload-done、ingestion-status）

**验收标准**：上传→ingestion完成→RAG回答正确→citation可交互。

### Slice 4：Share & Collaboration（1-2天）

- 新建 `pom/share-page.ts`
- 新建 `specs/workspace-share.spec.ts`
  - 配置share settings（role、access level、allow download）
  - 复制share token
  - 新browser context模拟访客打开share链接
  - 验证访客只能只读访问，不能编辑

### Slice 5：负面测试（2天）

- 新建 `specs/auth-failure.spec.ts`
  - 错误密码提示
  - 不存在的邮箱提示
  - 空字段验证
- 新建 `specs/network-degradation.spec.ts`
  - Slow 3G模式下chat仍能完成
  - Offline时显示错误提示（不crash）
- `playwright.config.ts` 全局 `page.on('pageerror')` 收集console error

### Slice 6：Cross-browser + Visual回归（1天）

- 迁移 `avrag-rs/e2e/visual-ui.spec.ts` 到 `frontend_next/e2e/specs/visual/`
- 更新快照路径（从avrag-rs目录复制到frontend_next）
- 配置firefox/webkit projects（默认CI不跑）
- 新增weekly cross-browser workflow

### Slice 7：Rust后端E2E补全（1-2天）

- CI新增 `e2e_format_output` 和 `e2e_ingestion_answer` 步骤
- `avrag-rs/e2e/` 添加 `README.md` 标记冻结，规则如下：
  - **只读**：不再接受任何修改PR
  - **删除触发条件**：Slice 7完成且 `frontend_next/e2e/` 对应覆盖率达标后30天
  - **Owner**：@e2e-maintainer（或指定负责删除的工程师）
- 原 `rust-frontend-e2e.spec.ts` API测试迁移到 `avrag-rs/tests/api-contract/`
- `avrag-rs/playwright.config.ts` 标记deprecated或删除

---

## 8. 12项修复映射表

| # | 修复项 | 落地位置 | 负责Slice |
|---|--------|---------|----------|
| 1 | T05文档上传断言过松 | `workspace-upload-rag.spec.ts` 要求 `"completed"` | Slice 3 |
| 2 | debug-ui.spec.ts不是测试 | **删除** `avrag-rs/e2e/debug-ui.spec.ts` | Slice 7 |
| 3 | CI不跑前端E2E | `.github/workflows/e2e-staging.yml` 新增Frontend步骤 | Slice 0 |
| 4 | login-hydration硬编码URL | `auth-flow.spec.ts` 使用 `page.goto("/login")` | Slice 1 |
| 5 | 前端缺少核心旅程 | `workspace-chat.spec.ts` + `workspace-upload-rag.spec.ts` | Slice 2-3 |
| 6 | seedBrowserAuth绕登录 | `globalSetup` 真实登录 + `storageState` | Slice 0 |
| 7 | RAG citation默认跳过 | `e2e_rag.rs` 移除skip；前端spec验证UI citation | Slice 3 + Slice 7 |
| 8 | format_output/ingestion_answer不在CI | CI新增cargo test步骤 | Slice 7 |
| 9 | 两套Playwright分裂 | 前端UI统一在 `frontend_next/e2e/` | Slice 0-7 |
| 10 | 缺少POM | 新建 `pom/` 目录 | Slice 1-4 |
| 11 | 缺少负面测试 | `auth-failure.spec.ts` + `network-degradation.spec.ts` | Slice 5 |
| 12 | 缺少cross-browser | 新增firefox/webkit projects | Slice 6 |

---

## 9. 风险与降级方案

| 风险 | 影响 | 降级方案 |
|------|------|---------|
| 后端 `/api/e2e/reset-user-data` 端点开发延迟 | Slice 0阻塞 | 临时用 `deleteWorkspaceViaAPI` 逐个清理，setup-env先不做reset |
| ingestion pipeline在CI中不稳定（耗时过长或失败） | Slice 3阻塞 | 引入Playwright `test.step` + 重试；若仍不稳定，将该test标记为 `test.slow()` 并延长timeout |
| LLM API rate limit导致Rust E2E失败 | Slice 7不稳定 | 已有 `--test-threads=1` 缓解；若仍触发，引入exponential backoff retry wrapper |
| 前端缺少 `data-testid` 需要业务代码改动 | Slice 2-4延迟 | 优先使用可访问性选择器（role、placeholder）替代；`data-testid` 作为最后手段补充 |
| Playwright `webServer` 启动双服务超时 | CI失败 | 拆分为独立docker-compose step，health check轮询后再运行测试 |
| 现有visual快照迁移后大量diff | Slice 6噪声 | 迁移时统一重新生成baseline快照，一次性的expected change |

---

## 10. 验收标准（整体）

重构完成后，以下场景必须满足：

1. **一位新加入的开发者**可以在本地通过 `cd frontend_next && npx playwright test` 一键运行全部前端E2E，无需手动启动任何服务。
2. **CI的E2E staging workflow**同时包含 `frontend_next` Playwright测试和 `avrag-rs` Rust策略测试，且全部通过。
3. **任何一个前端E2E spec的失败**都对应一个真实用户在前端操作时会发现的问题（不存在"API shortcut绕过bug"的情况）。
4. **RAG citation验证**默认启用，不依赖环境变量开关。
5. **文档上传测试**在ingestion未完成时失败，而不是默默接受任意状态。
