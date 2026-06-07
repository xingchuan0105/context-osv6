# E2E 测试组件重构设计（适配 v6 架构）

> 状态：评审中  
> 评审日期：2026-06-07  
> 决策：方案 B（合并前端 E2E，按技术栈二分）+ 新增 Tool & Skill Availability 层

---

## 1. 背景与目标

### 1.1 现状问题

当前项目中存在 **三套 E2E 测试体系**，职责边界模糊、内容重叠：

| 测试套件 | 位置 | 当前状态 |
|---------|------|---------|
| 前端 Playwright E2E | `frontend_next/e2e/` | 5 个 spec，覆盖基础 UI 流程，但未覆盖 v6 新增功能 |
| 全栈前端 E2E | `avrag-rs/tests/frontend_e2e/` | 9 个 spec，使用真实 LLM + golden set，但目录位置与前端项目分离 |
| API 层 E2E | `avrag-rs/crates/app/tests/product_e2e/` | 14 个 Rust 测试，已完工且稳定 |

### 1.2 v6 架构新增功能（当前 E2E 未覆盖）

- `/dashboard/[workspace_id]/analyze` — 数据分析与洞察
- `/dashboard/[workspace_id]/api-access` — API Key 管理
- `/dashboard/[workspace_id]/share/access-logs` — 分享访问日志
- `/dashboard/[workspace_id]/share/analytics` — 分享数据分析
- `/admin/*` — 后台管理（用户、组织、计费、审计日志等）
- `/invite/[workspace_id]/[member_id]` — 协作邀请

### 1.3 核心目标

1. **合并前端 E2E**：将 `avrag-rs/tests/frontend_e2e/` 迁移到 `frontend_next/e2e/`，删除原目录，统一维护。
2. **重新划分职责**：明确前端 E2E 与 API E2E 的边界，消除重复。
3. **补齐 v6 缺口**：新增 admin、analyze、api-access、invite 等场景的 E2E 覆盖。
4. **新增 Tool & Skill Availability 测试**：使用真实 LLM + query 走前端 UI 流程，验证产品内所有工具/skill 的可用性。

---

## 2. 总体架构

合并后只保留 **两套 E2E**：

| 测试套件 | 位置 | 职责 | 运行时机 |
|---------|------|------|---------|
| **Frontend Full-Stack E2E** | `frontend_next/e2e/` | 三大子集：<br>① **Smoke**：UI 渲染、导航、基础交互（Mock/轻量后端）<br>② **Journey**：端到端业务闭环（真实基础设施）<br>③ **Skills Availability**：真实 LLM + query 走 UI 验证所有工具/skill 可被触发 | ① PR 级<br>② 主干合并后<br>③ **Nightly** |
| **API Contract & Resilience** | `avrag-rs/crates/app/tests/product_e2e/` | HTTP 契约、降级路径、并发、租户隔离、失败场景 | PR + 主干（已完工，不动） |

**删除目标**：`avrag-rs/tests/frontend_e2e/` 整体迁移后删除。

### 2.1 与既有测试的边界

#### 与 `product_e2e/llm_real/` 的边界

`avrag-rs/crates/app/tests/product_e2e/llm_real/` 已存在，职责是**后端到 LLM 的直接契约测试**：验证后端直接调用 LLM 时，响应结构、token 消耗、错误码符合预期。**不涉及前端 UI 和 HTTP 路由**。

新的 `skills/` 层职责是**前端 UI 到工具调用的完整链路**：验证用户从浏览器发送 query → 后端路由 → 工具触发 → 前端渲染结果的全过程。**两者的测试入口和断言目标完全不同，不重叠**。

#### 与 `2026-05-23-e2e-state-machine-prompt-validation-design.md` 的边界

该设计使用 `RecordingLlmProvider` 捕获后端 Agent 的 prompt，验证**状态机转换和策略路由的正确性**（例如：什么 query 会触发什么 strategy、prompt 中是否包含正确上下文）。

本次 `skills/` 层从**用户视角**验证工具可用性（发送 query → 看到 citation / format output / analyze chart），**不验证内部状态机或 prompt 内容**。夜间 CI 中两者可并行运行，互不阻塞。

---

## 3. 合并后的目录结构

```
frontend_next/e2e/
├── playwright.config.ts          # 统一配置，projects 区分运行时机
├── global-setup.ts               # 整合 auth + 环境准备
├── setup-env.ts
├── setup-auth.ts
├── fixtures/
│   ├── test-user.ts
│   ├── run-context.ts
│   ├── sample-document.txt
│   ├── antifragile.txt           # ← 从 avrag-rs/tests/frontend_e2e/fixtures/ 迁移
│   └── golden-set.json           # ← 从 avrag-rs/tests/frontend_e2e/fixtures/ 迁移
├── pom/                          # Page Object Model
│   ├── login-page.ts             # ← 合并 avrag-rs 版本的方法
│   ├── dashboard-page.ts
│   ├── workspace-page.ts         # ← 导航/上传/切换 tab 等容器级方法
│   ├── chat-panel-page.ts        # ← 从 ChatPage.ts 迁移：setMode / ask / waitForAnswer / lastAnswerHtml / citationCount
│   ├── share-page.ts
│   ├── notebook-page.ts          # ← 从 avrag-rs 迁移并适配
│   ├── admin-page.ts             # 新增
│   ├── analyze-page.ts           # 新增
│   └── api-access-page.ts        # 新增
├── utils/
│   ├── api-helpers.ts
│   ├── backend-url.ts            # ← 从 avrag-rs 迁移
│   └── judge.ts                  # ← 从 avrag-rs/tests/frontend_e2e/src/quality/ 迁移
└── specs/
    ├── smoke/                    # PR 级：Mock/轻量后端
    │   ├── auth-flow.spec.ts
    │   ├── auth-failure.spec.ts
    │   ├── workspace-create.spec.ts
    │   └── admin-navigation.spec.ts    # 新增
    ├── journey/                  # 主干合并后：真实依赖，业务闭环
    │   ├── workspace-chat.spec.ts      # 保留在 journey/，仅验证消息流（不验证具体 tool 触发）
    │   ├── workspace-upload-rag.spec.ts
    │   ├── workspace-share.spec.ts
    │   ├── chat-session.spec.ts        # ← 从 avrag-rs 迁移
    │   ├── notebook-crud.spec.ts       # ← 从 avrag-rs 分阶段改造（Phase 2 保留 API 方式迁移，Phase 3 改为 UI 流程）
    │   ├── session-history.spec.ts     # ← 从 avrag-rs 迁移
    │   ├── analyze-workflow.spec.ts    # 新增
    │   └── invite-collaboration.spec.ts # 新增
    ├── skills/                   # Nightly：真实 LLM，工具可用性矩阵
    │   ├── rag-available.spec.ts       # ← 从 avrag-rs 改造
    │   ├── search-available.spec.ts    # ← 从 avrag-rs 改造
    │   ├── format-output.spec.ts       # ← 从 avrag-rs 改造
    │   ├── analyze-skill.spec.ts       # 新增
    │   └── notebook-skill.spec.ts      # 新增
    └── visual/                   # 视觉回归（保留）
        └── workspace-visual.spec.ts
```

**avrag-rs 侧保留结构（不动）**：

```
avrag-rs/crates/app/tests/product_e2e/
├── mod.rs
├── setup.rs
├── assertions.rs
├── fixtures/
├── smoke/
├── integration/
├── failure/
└── tenants/
```

**删除结构**：

```
avrag-rs/tests/frontend_e2e/          # 整体删除
├── specs/00-smoke.spec.ts            # 重复，不保留
├── src/pages/ChatPage.ts             # 合并到 workspace-page.ts 后删除
├── src/pages/LoginPage.ts            # 合并到 login-page.ts 后删除
├── src/setup/auth.ts                 # 合并到 setup-auth.ts 后删除
├── src/setup/backendUrl.ts           # 迁移后删除
├── package.json                      # 删除
├── playwright.config.ts              # 删除
├── global-setup.ts                   # 删除
├── global-teardown.ts                # 删除
├── tsconfig.json                     # 删除
└── pnpm-lock.yaml                    # 删除
```

---

## 4. 迁移与清理清单

### 4.1 从 `avrag-rs/tests/frontend_e2e/` 迁移

| 源文件 | 迁移目标 | 处理方式 |
|--------|---------|---------|
| `fixtures/golden_set.json` | `frontend_next/e2e/fixtures/golden_set.json` | 直接复制，扩展新增 skill 条目 |
| `fixtures/documents/antifragile.txt` | `frontend_next/e2e/fixtures/antifragile.txt` | 直接复制 |
| `src/quality/judge.ts` | `frontend_next/e2e/utils/judge.ts` | 直接复制，`.env` 加载路径改为 `path.resolve(__dirname, "../../../../")`（项目根目录） |
| `src/pages/ChatPage.ts` | `frontend_next/e2e/pom/chat-panel-page.ts` | **迁移为独立 `ChatPanelPage`**：`setMode`、`ask`、`waitForAnswer`、`lastAnswerHtml`、`citationCount` 等方法整体迁移；`workspace-page.ts` 仅保留容器级方法（导航/上传/切换 tab） |
| `src/pages/NotebookPage.ts` | `frontend_next/e2e/pom/notebook-page.ts` | 迁移并适配到前端项目 |
| `src/setup/auth.ts` | 与 `setup-auth.ts` 合并 | 保留现有的 `storageState` 方案，吸收 `registerTestUser`、`injectAuth` 作为备用 |
| `src/setup/backendUrl.ts` | `frontend_next/e2e/utils/backend-url.ts` | 直接迁移，统一后端地址获取 |

### 4.2 从 `avrag-rs/tests/frontend_e2e/specs/` 迁移改造

| 源文件 | 目标位置 | 改造说明 |
|--------|---------|---------|
| `00-smoke.spec.ts` | 不单独保留 | 内容（登录+页面加载）已覆盖在 `auth-flow.spec.ts` 和 `workspace-create.spec.ts` 中 |
| `01-upload-ingestion.spec.ts` | `specs/journey/workspace-upload-rag.spec.ts` | 与现有测试合并，升级为完整 RAG 闭环 |
| `02-rag-qa.spec.ts` | `specs/skills/rag-available.spec.ts` | 改造为 **工具可用性矩阵** 格式：验证 `rag` 模式可被触发且返回 citations |
| `03-search-qa.spec.ts` | `specs/skills/search-available.spec.ts` | 同上，验证 `search` 模式可被触发且返回 web citations |
| `04-chat-session.spec.ts` | `specs/journey/chat-session.spec.ts` | 保留为多轮对话场景 |
| `05-notebook-crud.spec.ts` | `specs/journey/notebook-crud.spec.ts` | **分阶段改造**：Phase 2 先以 API 方式迁移（不阻塞整体进度），Phase 3 再改为 UI 流程（用 `page` 对象操作：① 点击"新建 Notebook" → 填写名称 → 提交；② 点击重命名 → 修改名称 → 确认；③ 点击删除 → 确认对话框） |
| `06-format-output.spec.ts` | `specs/skills/format-output.spec.ts` | 改造为可用性测试：验证 HTML/PPT 格式可被触发且返回结构化输出 |
| `07-session-history.spec.ts` | `specs/journey/session-history.spec.ts` | 保留，验证历史记录持久化 |
| `08-tenant-isolation.spec.ts` | **不迁移到 journey/** | 租户隔离核心在数据层，UI 验证成本高、断言面窄，**仅在 `product_e2e/tenants/` 保留** |

### 4.3 前端现有测试调整

| 现有文件 | 调整动作 | 原因 |
|---------|---------|------|
| `specs/workspace-chat.spec.ts` | 保留在 `specs/journey/`，**仅验证消息流**（消息能发、能收、历史有记录） | `journey/` 不验证具体 tool 是否触发，此职责完全由 `skills/` 层承担 |
| `specs/workspace-share.spec.ts` | 保留在 `specs/journey/` | 业务闭环，无需大改 |
| `specs/auth-flow.spec.ts` | 保留在 `specs/smoke/` | 纯 UI 流程，定位不变 |
| `specs/auth-failure.spec.ts` | 保留在 `specs/smoke/` | 纯 UI 流程，定位不变 |
| `pom/workspace-page.ts` | 瘦身，移除 Chat 专属方法 | Chat 方法已迁移到 `chat-panel-page.ts`；`workspace-page.ts` 仅保留容器级职责 |

### 4.4 新增内容

| 新增文件 | 所属分类 | 说明 |
|---------|---------|------|
| `pom/admin-page.ts` | 基础设施 | Admin 后台导航、用户管理、组织管理 |
| `pom/analyze-page.ts` | 基础设施 | Analyze 数据/图表页面交互 |
| `pom/api-access-page.ts` | 基础设施 | API Key 创建/查看/删除 |
| `specs/smoke/admin-navigation.spec.ts` | Smoke | 验证 admin 页面可加载、导航正常 |
| `specs/journey/analyze-workflow.spec.ts` | Journey | 上传文档 → 进入 analyze → 验证图表/洞察生成 |
| `specs/journey/invite-collaboration.spec.ts` | Journey | 邀请用户 → 被邀请者接受 → 协作访问 workspace |
| `specs/skills/analyze-skill.spec.ts` | Skills | 真实 LLM query 触发 analyze 工具，验证返回结构化分析 |
| `specs/skills/notebook-skill.spec.ts` | Skills | 真实 LLM query 触发 notebook 相关 skill |

---

## 5. 运行模式与 CI/CD 调整

### 5.1 Playwright Projects 设计

```ts
// frontend_next/playwright.config.ts
projects: [
  // ── PR 级：Smoke（Mock/轻量，秒级-分钟级）
  {
    name: "smoke",
    testMatch: [/specs\/smoke\/.*\.spec\.ts/],
    use: {
      storageState: "playwright/.auth/user.json",
      trace: "retain-on-failure",
    },
  },

  // ── 主干合并后：Journey（真实基础设施，业务闭环）
  {
    name: "journey",
    testMatch: [/specs\/journey\/.*\.spec\.ts/],
    use: {
      storageState: "playwright/.auth/user.json",
      trace: "retain-on-failure",
    },
  },

  // ── Nightly：Skills Availability（真实 LLM，工具矩阵）
  {
    name: "skills",
    testMatch: [/specs\/skills\/.*\.spec\.ts/],
    retries: 1,                    // 真实 LLM 可能抖动
    use: {
      storageState: "playwright/.auth/user.json",
      trace: "on-first-retry",
      video: "on-first-retry",
    },
  },

  // ── 视觉回归（独立 tier，不属于 smoke/journey/skills）
  {
    name: "visual-desktop",
    testMatch: [/visual\/.*\.spec\.ts/],
    use: { viewport: { width: 1440, height: 900 } },
  },
  {
    name: "visual-mobile",
    testMatch: [/visual\/.*\.spec\.ts/],
    use: { ...devices["Pixel 5"] },
  },

  // ── Auth（独立，无登录态）
  {
    name: "auth",
    testMatch: [/smoke\/auth.*\.spec\.ts/],
    use: { storageState: { cookies: [], origins: [] } },
  },
],
```

### 5.2 webServer 调整

合并后 `frontend_next` 的 `playwright.config.ts` 中的 `webServer` 需要能启动 Rust 后端（因为全栈测试和 skill 测试需要真实后端）：

```ts
webServer: [
  // Rust 后端（Skip 只在纯前端 smoke 时可用）
  ...(process.env.SKIP_BACKEND
    ? []
    : [{
        command: "cd ../avrag-rs && cargo run --bin avrag-api",
        url: "http://127.0.0.1:8080/health",
        timeout: 120_000,
        reuseExistingServer: !process.env.CI,
      }]),
  // Next.js 前端
  {
    command: process.env.CI ? "pnpm build && pnpm start" : "pnpm dev",
    url: "http://127.0.0.1:3000",
    timeout: 60_000,
    reuseExistingServer: !process.env.CI,
  },
],
```

### 5.3 CI Workflow 调整

| Workflow | 触发时机 | 运行命令 | 说明 |
|---------|---------|---------|------|
| `frontend-smoke.yml` | PR | `pnpm exec playwright test --project=smoke --project=auth` | 5min 内完成 |
| `frontend-journey.yml` | 主干合并 | `pnpm exec playwright test --project=journey` | 真实后端 |
| `frontend-skills.yml` | Nightly (cron) | `pnpm exec playwright test --project=skills` | 真实 LLM，retries=1 |
| `frontend-visual.yml` | PR/主干 | `pnpm exec playwright test --project=visual-*` | 截图对比 |
| `product-e2e.yml` | PR + 主干 | `cargo test -p app --test product_e2e` | 已存在，不动 |

### 5.4 环境变量需求

| 变量 | 用途 | 必填场景 |
|------|------|---------|
| `DASHSCOPE_API_KEY` | LLM Judge | `skills` project |
| `E2E_LLM_BASE_URL` | 真实 LLM endpoint | `skills` project |
| `E2E_LLM_API_KEY` | 真实 LLM API Key | `skills` project |
| `E2E_BRAVE_API_KEY` | Search provider | `skills` project (search-available) |
| `SKIP_BACKEND` | 跳过 Rust 后端启动 | **仅本地纯前端开发调试用**；CI 中所有 project 都启动后端 |
| `E2E_INGESTION_TIMEOUT` | Ingestion 等待超时 | `journey` project |

---

## 6. Tool & Skill Availability 矩阵设计

这是本次新增的 **Nightly 专用测试层**。每个 spec 验证一个工具/skill 能否被正确触发并返回结构化输出。断言聚焦在**可用性标识**（不依赖 LLM 措辞，避免抖动）。

### 6.1 统一测试模板

```ts
// specs/skills/_skill-template.ts（非执行，仅规范）
// 1. Setup: 登录 → 创建 workspace → 上传文档（如需要）
// 2. Trigger: 发送特定 query 或切换 mode
// 3. Wait: 等待响应完成（progress card detached）
// 4. Availability Assert: 验证工具触发的结构性标识
// 5. Quality Assert (optional): 调用 judge.ts 评估输出质量（仅 nightly 报告）
```

### 6.2 Skill 矩阵

| Skill | Spec 文件 | 触发方式 | 可用性断言 | 质量断言（可选） |
|-------|----------|---------|-----------|----------------|
| **RAG** | `rag-available.spec.ts` | `setMode("rag")` + query | `citationCount > 0` + `citations` 含 `doc_id` | judge: 回答是否引用文档内容 |
| **Search** | `search-available.spec.ts` | `setMode("search")` + 开放 query | `citationCount > 0` + `citations` 含 `source_type == "web"` | judge: 回答是否基于搜索结果 |
| **Format Output (HTML)** | `format-output.spec.ts` | query 含 "生成 HTML/PPT" 或 mode | 回答区域包含 `<html` 标签或 `data-format="html"` | judge: HTML 结构有效性 |
| **Analyze** | `analyze-skill.spec.ts` | 进入 `/analyze` + 发送分析 query | 页面出现 `data-testid="analyze-chart"` 或 `data-testid="analyze-insight"` | judge: 洞察是否基于文档数据 |
| **Notebook** | `notebook-skill.spec.ts` | query 触发 notebook 操作 | 页面出现 notebook 引用或 `data-testid="notebook-citation"` | judge: notebook 内容相关性 |

**预估运行时长**：5 个 skills × (LLM 调用 10-30s + 可能的 ingestion 30-60s) ≈ **15–25 分钟**。若 RAG 类型 spec 复用已上传文档（见 6.5），可压缩至 **10–15 分钟**。

### 6.3 断言分层（关键设计）

**可用性断言（硬门槛，必须 pass）：**

- 不依赖 LLM 输出内容，只验证**工具是否被调用 + 返回结构是否正确**。
- 示例：`expect(citationCount).toBeGreaterThan(0)`、`expect(page.locator('[data-testid="analyze-chart"]')).toBeVisible()`。

**质量断言（软门槛，仅报告）：**

- 调用 `judge.ts` 返回 0-1 分数。
- 分数低于阈值**不 fail 测试**，只写入报告用于趋势分析。
- 避免 LLM 抖动阻塞 nightly CI。

```ts
// 质量断言示例（非阻塞）
test("quality score meets baseline", async ({ page }) => {
  test.skip(!process.env.RUN_QUALITY_JUDGE, "Quality judge disabled");
  const answer = await workspace.getLastAnswerText();
  const entry = goldenSet.entries.find(e => e.id === "rag-01");
  const result = await judgeAnswer(answer, entry!);
  // 写入报告，不 assert
  test.info().attach("judge-result", { body: JSON.stringify(result) });
});
```

### 6.4 Golden Set 扩展

迁移后的 `golden_set.json` 需扩展新增 skill 的判定条目：

```json
{
  "entries": [
    { "id": "rag-01", "query": "What is antifragility?", "judge_prompt": "..." },
    { "id": "search-01", "query": "Latest Rust release", "judge_prompt": "..." },
    { "id": "format-html-01", "query": "Generate an HTML summary", "judge_prompt": "..." },
    { "id": "analyze-01", "query": "Analyze the key themes", "judge_prompt": "..." },
    { "id": "notebook-01", "query": "Save this to notebook", "judge_prompt": "..." }
  ]
}
```

### 6.5 Fixture Document 复用策略

RAG 和 Analyze 类型的 skills 需要上传文档并等待 ingestion。为避免每个 spec 独立上传导致时长膨胀：

- `journey/` 的 `workspace-upload-rag.spec.ts` 在 `test.beforeAll` 中上传 `antifragile.txt` 并等待 ingestion 完成。
- `skills/` 的 `rag-available.spec.ts` 和 `analyze-skill.spec.ts` **复用同一 workspace 和文档**，不再重复上传。
- 若 `skills` project 独立运行（不依赖 `journey` 前置），则在 `globalSetup` 或 `test.beforeAll` 中完成一次性的 fixture 上传。

---

## 7. 关键风险与应对

| 风险 | 影响 | 应对策略 |
|------|------|---------|
| **POM 合并破坏现有测试** | `workspace-page.ts` 吸收 `ChatPage.ts` 时可能改变接口，导致现有 spec 失败 | **Phase 1 先做 POM 重构**：重构完成后跑通全部现有测试，再进入迁移 |
| **Notebook CRUD 从 API 改 UI 工作量大** | 需要重新实现页面交互逻辑，可能阻塞整体进度 | **分阶段**：Phase 2 先保留 API 方式迁移到 `journey/`，Phase 3 再改为纯 UI 流程 |
| **真实 LLM 抖动导致 nightly 不稳定** | `skills` project 可能频繁失败，降低信任度 | retries=1 + 质量断言不阻塞 + 失败自动收集 screenshot/video/trace |
| **前端 E2E 与 API E2E 重复未完全消除** | `workspace-upload-rag` 在前端和 API 层都测上传+ingestion | 明确分层：前端测**用户流程完整性**，API 测**契约边界和降级路径**，重复是预期内 |
| **avrag-rs frontend_e2e 用 3001 端口** | 合并后需统一为 3000，可能硬编码在某些脚本中 | 全局搜索替换 `3001` → `3000`，CI 脚本同步更新 |
| **两套 auth 方案冲突** | `frontend_next` 用 Playwright `storageState`，`avrag-rs frontend_e2e` 用 `injectAuth`（直接操作 localStorage） | 统一以 `storageState` 为主；`injectAuth` 仅用于需要动态注册并立即登录的测试场景（如 auth-flow 中创建临时用户） |

---

## 8. 实施阶段建议

| Phase | 内容 | 时长估算 |
|-------|------|---------|
| Phase 1 | POM 重构：`ChatPage.ts` → `chat-panel-page.ts`，`workspace-page.ts` 瘦身，跑通现有测试 | 0.5 天 |
| Phase 2 | 迁移 `avrag-rs/tests/frontend_e2e/` 内容到 `frontend_next/e2e/`，建立 `journey/` 和 `skills/` 目录 | 1 天 |
| Phase 3 | 补齐 v6 缺口 + notebook-crud 改 UI：admin、analyze、api-access、invite POM + spec；notebook-crud 从 API 方式改为 UI 流程 | 1 天 |
| Phase 4 | 删除 `avrag-rs/tests/frontend_e2e/`，调整 CI workflow，端到端验证 | 0.5 天 |

**总计：约 3 天。**

---

## 9. 成功标准

实施完成需同时满足以下全部条件：

1. **Phase 1–4 全部完成**：POM 重构、迁移、补齐、清理四阶段代码均已提交。
2. **现有测试全绿**：`frontend_next/e2e/` 中原有 spec（auth-flow、auth-failure、workspace-chat、workspace-share、workspace-upload-rag、workspace-visual）在重构后全部通过。
3. **新增 journey 测试可运行**：`notebook-crud`、`session-history`、`chat-session`、`analyze-workflow`、`invite-collaboration` 至少 5 个新增/迁移的 journey spec 在主干合并流程中通过。
4. **skills  nightly 跑通**：`rag-available`、`search-available`、`format-output`、`analyze-skill`、`notebook-skill` 5 个 skills spec 在真实 LLM 环境下通过（允许 retries=1 后的通过）。
5. **`avrag-rs/tests/frontend_e2e/` 完整删除**：原目录及所有文件已从仓库移除，无残留引用。
6. **API E2E 不受影响**：`avrag-rs/crates/app/tests/product_e2e/` 的 14 个测试继续全绿，运行方式和结果无变化。
7. **CI workflow 生效**：`frontend-smoke.yml`、`frontend-journey.yml`、`frontend-skills.yml`（或等效配置）在对应触发时机正常执行并产生报告。
