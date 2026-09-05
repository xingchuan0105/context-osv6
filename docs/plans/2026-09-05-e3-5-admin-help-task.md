# E3.5 任务记录：管理后台运维与应用内帮助 (`/admin/*`, `/help`, `/help/write`)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 负责人 | Agent / Solo Trunk |
| 关联计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) §6 E3.5 |
| 门禁目标 | **E3.5 切片完成，冲刺 Gate 3 (G3)** |

---

## 1. 任务目标与交付范围

按全量路由矩阵 §7/§8 挂载剩余 15 个端点，达成 G3：

1. **管理后台路由族 (13 端点)**：`/admin`（概览入口网格）、`/admin/accounts`（列表 + 客户端分页）、`/admin/accounts/:owner_user_id`（详情 + 封禁/解封）、`/admin/users`（按 owner 查询 + 两步确认删除）、`/admin/usage`（owner + period 表单统计）、`/admin/billing`、`/admin/health`、`/admin/rag-health`、`/admin/system/workers`、`/admin/system/degradation`（只读状态卡）、`/admin/broadcast`（公告表单 + 送达数回显）、`/admin/audit-logs`（过滤 + 服务端分页 + 空态 + CSV 导出）、`/admin/feature-flags`（开关表 + 变更请求提交/复核执行）。
2. **应用内帮助路由族 (2 端点)**：`/help`（帮助中心入口卡，仅链已挂载规范路由）、`/help/write`（长文与提示词编写建议静态长文）。
3. **不变量**：
   - 非 admin 拦截：未登录 → `/login?next=<pathname>`；403 `admin_access_denied` → 友好无权访问面板（`admin-forbidden`）。权限真源仍为后端 DB role（super_admin/ops_admin/finance_admin），前端不新增角色字段、不解析 JWT。
   - `contracts::admin` 与后端已分叉（OrgRow/UserRow/Usage/Worker/Health 字段不一致），本切片**不修 contracts**；web-sdk 按 `billing_api` 惯例本地定义 DTO 并解 `ApiResponse` 信封。
   - admin 不进 ROUTE_FAMILIES / nav-config（双向 parity 测试 + PRODUCT_IA：admin 不属导航权威）；`AppRoute::Admin{AdminSection}` 仅作路由解析登记。
   - 样式全部走 `var(--cos-*)` token，零 hex、零字重 ≥500（守卫测试把关）。

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E3.5.1 | web-sdk `admin_api.rs`：信封解析（`AdminEnvelope` 手动 Deserialize 规避 serde 泛型 default 陷阱）、13 组 DTO、URL builder/parse fn、`BrowserRestClient` 双 impl 18 方法、lib.rs 导出 | 已完成 | `web-sdk/src/admin_api.rs`、`browser_rest.rs`；`admin_api_tests.rs` 5/5 passed（信封展开/裸数组/audit 分页/403 错误体/native unavailable） |
| E3.5.2 | web-ui `components/admin/`：`AdminShell`（门禁 + 子导航闭环 + LoginRequired/Forbidden/Loading 集中渲染）+ 13 页面组件 | 已完成 | 页面全部带 `data-testid`、`role="alert"` 错误段、空态与分页 |
| E3.5.3 | web-ui `components/help/`：`HelpPage`、`HelpWritePage` | 已完成 | 仅链已挂载路由，不制造孤儿目的地 |
| E3.5.4 | 路由：`app.rs` 15 条 `<Route>`；`routes.rs` `AdminSection` 13 值枚举 + `AppRoute::Admin/Help/HelpWrite` + 解析测试（含 `/admin/nope` → NotFound） | 已完成 | nav_parity 双向校验保持绿 |
| E3.5.5 | 样式：`chat-poc.css` 追加 `admin-*`/`help-*` 段（token-only） | 已完成 | style_baseline_guard 1/1 passed |
| E3.5.6 | fixture：`fixture-sse-server.mjs` 新增 `/api/v1/admin/*` 全套端点（信封响应、12 账户分页数据、audit 过滤/翻页/CSV、403 case 路径、reset 复位） | 已完成 | `admin-journey.spec.ts` 8/8 passed |
| E3.5.7 | 浏览器测试：`admin-journey.spec.ts`（未登录跳转 / 403 面板 / 未知路由兜底 / 概览入口 / 账户分页→详情封禁→用户删除闭环 / audit 筛选翻页空态 CSV / broadcast / feature-flags 请求与复核）+ `help-journey.spec.ts`（渲染互链） | 已完成 | 全量 Playwright 48/48 passed |
| E3.5.8 | 修复记录：SSR panic（页面先于 AdminShell `expect_context`）→ 页面自建 `AdminPageState` 并 provide；view! 宏属性表达式含 `>=`/`<=` 被当标签解析 → 提升 Signal；`paged` 信号 `get_untracked` 破坏翻页响应性；flags/users/usage 门禁卡 Loading | 已完成 | 见 `_reports/2026-09-05-e3-5/` |
| E3.5.9 | 验证收敛、矩阵更新（15 行 已挂载）、图谱更新与本地提交 | 已完成 | 图谱增量 20 files/153 nodes；提交号见 §3 |

## 3. 验证证据

命令与退出码存 `docs/engineering/_reports/2026-09-05-e3-5/`：

- `cargo test -p web-sdk -p web-ui`：全套件 0 failed（含 style_baseline_guard、nav_parity、routes、admin_api）
- `cargo check -p web-server --features ssr`：exit 0
- `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate`：exit 0
- `cargo leptos build`（wasm-bindgen-cli 0.2.127）：成功
- `pnpm exec playwright test`：**48 passed / 0 failed**（38 既有 + 10 新增）
- 测试环境：WSL2，`CARGO_BUILD_JOBS=2`

## 4. 剩余问题

- `contracts::admin` 与后端 `admin_domain.rs` 的 DTO 分叉（OrgRow/UserRow/AdminUsageResponse/WorkerStatusResponse/RagHealthStatus/HealthResponse）待另立切片修复（涉及 TS bindings 再生）。
- 用户列表删除无服务端 owner 归属校验提示依赖后端；前端已做两步确认。
- G3 达成后 E4（W4 公共 SSR / SEO / 双语）可开启。

## 5. 图谱状态

`code-review-graph update` 已在根目录执行：Incremental 20 files updated, 153 nodes, 1286 edges。`.code-review-graph/` 不入库。
