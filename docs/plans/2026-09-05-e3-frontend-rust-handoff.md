# Rust 前端与桌面宿主库交接文档 (E0–E4.4 交付与下一棒指引)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 状态 | **Active（现行有效交接文档）** |
| 当前 HEAD | E4.4 之后（见 git log；切片提交 E3.5 / E4.1 / E4.2 / E4.3 / E4.4） |
| 权威计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) |
| 全量路由矩阵 | [`docs/design/ROUTE_MIGRATION_MATRIX.md`](../design/ROUTE_MIGRATION_MATRIX.md) (71 个真实端点) |
| 达成门禁 | **G0 / G1 / G2 / G3 / G4 全部达成**；**D0.1** 完成；**71/71 端点全量挂载** |
| 下一棒任务 | **W5 Web 切流准备** 与 **D0.2–D1 桌面里程碑（GD1：Windows 真机）** |

---

## 1. 进展与提交记录链

本次连续推进了 6 个大阶段、累计完成 9 次规范原子提交，各阶段对应任务文档与交付成果如下：

| 阶段 | 提交 SHA | 核心工作内容 | 对应记录文档 |
|---|---|---|---|
| **E0** | `cd821a74` | 会话文件托盘 (`SessionFileTray`)、能力标签 (`ScopeBar`)、多端口 `resolve_upload_url`、**G0 达成** | [`2026-09-05-e0-w2-files-scope-task.md`](2026-09-05-e0-w2-files-scope-task.md) |
| **E1** | `40614225` | 盘点全量 71 路由迁移矩阵、样式守卫扩展覆盖 `chat-poc.css`、静态产物 MIME/br/immutable 验证、Tiptap 方案明确、**G1 达成** | [`2026-09-05-e1-w1-acceptance-route-matrix-task.md`](2026-09-05-e1-w1-acceptance-route-matrix-task.md) |
| **W2.6** | `774c3b3a` | 模型角色标牌 (`ModelRoleBadge`)、`quick_chat` 个人默认角色与自备密钥 BYOK 识别 | [`2026-09-05-w2-6-model-role-byok-task.md`](2026-09-05-w2-6-model-role-byok-task.md) |
| **W2.7** | `b2ba620d` | 回答复制操作、Feedback 点赞/点踩真实持久化与错误警示、已删除来源卡标记与链接禁用 | [`2026-09-05-w2-7-feedback-actions-citations-task.md`](2026-09-05-w2-7-feedback-actions-citations-task.md) |
| **W2.8** | `93533806` | `/dashboard/:id?session=:sid` 路由、复用 ChatCanvas、工作区横幅、全局会话归属徽标、切库防串流 | [`2026-09-05-w2-8-workspace-chatcanvas-shell-task.md`](2026-09-05-w2-8-workspace-chatcanvas-shell-task.md) |
| **W2.9** | `5bd52c59` | 十二条 Chat-first 不变量逐一验收闭环、受限上下文隔离、跨轮次历史事实不可变、**G2 达成** | [`2026-09-05-w2-9-chat-first-gate2-task.md`](2026-09-05-w2-9-chat-first-gate2-task.md) |
| **D0.1** | `b3ce4ad7` | 抽离最小宿主库 `desktop/core`、复用 `web-sdk::SseDecoder` 根治多字节中文切块乱码、删除废弃许可门 | [`2026-09-05-d0-1-desktop-stream-parsing-task.md`](2026-09-05-d0-1-desktop-stream-parsing-task.md) |
| **E3.1** | `b2e71dfc` | 账号认证五页面 (`/login`, `/register`, `/reset-password/*`)、设置页与四大模型 BYOK 密钥录入与撤销闭环 | [`2026-09-05-e3-1-auth-settings-task.md`](2026-09-05-e3-1-auth-settings-task.md) |
| **E3.2** | `03bb02a6` | `/dashboard` 概览与弹窗建库、`/dashboard/:id` 工作台与右侧轨（持久资料+笔记）、分析与统计两端点 | [`2026-09-05-e3-2-dashboard-workspace-task.md`](2026-09-05-e3-2-dashboard-workspace-task.md) |
| **E3.3** | `65ab035a` | 工作区分享中心三端点、公开只读知识库问答 (`/shared/kb/:token`)、分享者公开主页与工作区邀请加入页面 | [`2026-09-05-e3-3-share-invite-task.md`](2026-09-05-e3-3-share-invite-task.md) |
| **E3.4** | `82076d9b` | 套餐定价对比 (`/pricing`)、钱包充值面板 (`#topup`)、拦截墙 (`/upgrade/paywall`)、成功回跳与桌面购买说明 | [`2026-09-05-e3-4-billing-pricing-task.md`](2026-09-05-e3-4-billing-pricing-task.md) |
| **E3.5** | 本切片提交 | `/admin/*` 管理后台 13 端点（门禁 401→login?next / 403→无权面板、分页、空态、CSV 导出、变更请求复核闭环）与 `/help`、`/help/write` 应用内帮助；**G3 达成** | [`2026-09-05-e3-5-admin-help-task.md`](2026-09-05-e3-5-admin-help-task.md) |
| **E4.1** | 本切片提交 | 公开帮助与集成生态 8 端点（`/help/faq`, `/help/compare`, `/help/api-access*`, `/integrations/*`），内容对齐 Next 单一数据源 + canonical/hreflang SEO 头 | [`2026-09-05-e4-1-public-help-integrations-task.md`](2026-09-05-e4-1-public-help-integrations-task.md) |
| **E4.2** | 本切片提交 | 桌面产品与法务公开族 9 端点（`/desktop`, `/activate`, `/setup`, `/legal*`，法务 MDX 内嵌 + TOC）+ 第三方声明下载路由；`/desktop` 规范链接恢复 | [`2026-09-05-e4-2-desktop-legal-task.md`](2026-09-05-e4-2-desktop-legal-task.md) |
| **E4.3** | 本切片提交 | 首页产品根 `/`（SSR 价值主张 + 会话分流 + Organization/WebSite/SoftwareApplication JSON-LD，FAQPage 同源标注）、`/llms.txt`、百度站长验证 | [`2026-09-05-e4-3-home-crawl-protocol-task.md`](2026-09-05-e4-3-home-crawl-protocol-task.md) |
| **E4.4** | 本切片提交 | `/en/*` 英文公共站 13 端点（首页 / 定价 / 桌面 / 帮助 4 / 法务 6），公开组件 locale 参数化；**71/71 全量挂载，G4 达成** | [`2026-09-05-e4-4-en-public-site-task.md`](2026-09-05-e4-4-en-public-site-task.md) |

---

## 2. 架构现状与已挂载路由清单

当前 `frontend_rust` 已挂载并实现 **71 个产品端点**（100% 覆盖 Next 全量路由矩阵）：zh 全量 + `/en/*` 英文公共站 + 管理后台 + 抓取协议。路由级清单以 `ROUTE_MIGRATION_MATRIX.md` 为权威（71 行全部「已挂载」，归属阶段 E0–E4.4）；另含两条辅助静态路由：`/legal/third-party-notices.md`（下载）与 `/healthz`。主要家族概览：

- 对话与工作区：`/chat/:session_id?`、`/dashboard/*`（8）、`/shared/*`、`/invite/*`
- 认证与设置：`/login`、`/register`、`/reset-password/*`、`/settings/*`
- 交易与桌面：`/pricing`、`/upgrade/*`、`/desktop`、`/desktop/buy`、`/activate`、`/setup`
- 管理后台：`/admin/*`（13）
- 公开站 zh：`/`、`/help/*`（5）、`/integrations/*`（4）、`/legal/*`（6）
- 公开站 en：`/en`、`/en/pricing`、`/en/desktop`、`/en/help/*`（4）、`/en/legal/*`（6）
- 抓取协议：`/llms.txt`、`/baidu_verify_codeva-THd6TRYMwv.html`

### 核心分工体系
1. **`contracts`**：跨端统一协议源头（DTO 契约），保证前端、后端与桌面端零协议分叉。
2. **`web-sdk`**：平台中立客户端库（无 DOM、无 Leptos、无 Tauri），涵盖增量 `SseDecoder`、状态机 `reduce_chat_event`、以及完整的 `browser_auth`、`browser_rest`、`workspace_api`、`share_api`、`billing_api`。在 wasm32 下对接 Fetch API，在 native 下返回类型安全的 `Unavailable`。
3. **`web-ui`**：Leptos 0.8 SSR + WASM 水合界面层，统一提供 `ChatCanvasModel` 信号上下文，多页面共享同一个聊天画布组件，彻底消除双轨维护负担。
4. **`desktop/core`**：平台通用的桌面宿主核心库，供 Tauri 和未来 GPUI 桌面端共用流式传输与后端桥接。

---

## 3. 测试与验证门禁证据

所有交付均附带全量自动化验证证据，当前本地代码库处于 100% 绿态：

| 验证维度 | 命令与覆盖 | 结果 |
|---|---|:---:|
| **Rust 模型与集成测试** | `CARGO_BUILD_JOBS=2 cargo test -p web-sdk -p web-ui` | **24 个测试套件，122 passed / 0 failed** |
| **desktop-core 单元测试** | `CARGO_BUILD_JOBS=2 cargo test --manifest-path desktop/core/Cargo.toml` | **4 passed / 0 failed** (中文跨 chunk 切片测试通过) |
| **SSR 检查** | `cargo check -p web-server --features ssr` | **exit code 0** |
| **WASM Hydrate 检查** | `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate` | **exit code 0** |
| **Tauri 宿主检查** | `cargo +1.96.1 check --manifest-path desktop/src-tauri/Cargo.toml` | **exit code 0** |
| **设计系统样式守卫** | `cargo test -p web-ui --test style_baseline_guard` | **1 passed** (无字重 ≥500、无裸十六进制、无未授权阴影) |
| **Playwright 端到端旅程** | `pnpm exec playwright test (11 个 spec 文件)` | **全量 74 项测试 100% passed** |
| **Live Backend Smoke** | `LIVE_BACKEND=1 LIVE_API_BASE=http://127.0.0.1:18081 pnpm exec playwright test --config playwright.live.config.ts` | **2 passed / 0 failed** (真实 API 对接通过) |

---

## 4. 下一棒执行指南：W5 Web 切流准备 与 D0.2–D1 桌面里程碑

E3 / E4 已全部完成，**71/71 端点挂载，G3 / G4 达成**（切片记录见 §1 与 `plans/2026-09-05-e4-*` 任务文档）。下一棒两条线：

1. **W5 Web 切流**（G3/G4 已过，沿用现行排期）：所有在线路由完成；切流前按基线冻结观测窗口、样本量、错误率/成功率/延迟阈值；版本化产物可回滚。权威条件见权威计划 §8。
2. **D0.2–D1 桌面里程碑**（依赖 G4、D0.1）：GPUI 本地登录、真实聊天、取消，冲刺 GD1（Windows 真机）。

遗留偏差备忘：`/en/pricing` UI 文案为 zh（en 文案后续补齐）；`contracts::admin` 与后端 DTO 分叉待另立切片；MarketingShell 营销导航壳未移植（公开页统一轻壳）。

### 4.1 常用命令与操作提示
- **构建工作目录**：`frontend_rust`
- **WASM bindgen CLI 锁定版本**：构建前必须确保使用 0.2.127：
  ```bash
  export PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$PATH"
  ```
- **构建命令**：`CARGO_BUILD_JOBS=2 cargo leptos build`
- **测试命令**：
  - Rust 测试：`CARGO_BUILD_JOBS=2 cargo test -p web-sdk -p web-ui`
  - Playwright：`cd tests/browser && pnpm exec playwright test`
  - 样式守卫：`cargo test -p web-ui --test style_baseline_guard`
- **图谱维护**：结构性代码修改后，务必在根目录运行：
  ```bash
  code-review-graph update
  ```
- **Time-cost consent 准则**：在启动耗时编译或测试脚本前，先向用户呈报时间预算并取得授权。
- **遗留问题**：`contracts::admin` 与后端 `admin_domain.rs` 的 DTO 分叉（OrgRow/UserRow/AdminUsageResponse/WorkerStatusResponse/RagHealthStatus/HealthResponse）待另立切片修复；web-sdk 管理端 DTO 目前本地定义于 `web-sdk/src/admin_api.rs`。
