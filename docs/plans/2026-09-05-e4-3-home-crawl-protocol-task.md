# E4.3 任务记录：首页产品根、JSON-LD 与抓取协议 (`/`, `/llms.txt`, 百度站长验证)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 负责人 | Agent / Solo Trunk |
| 关联计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) §6 E4 |
| 门禁目标 | **E4.3 切片完成**（E4 剩余 13 端点 = `/en/*`） |

---

## 1. 任务目标与交付范围

1. **首页产品根 `/`**：替换 E0 的 PoC 重定向。SSR 输出可被抓取的产品价值主张（H1/副标题/署名/更新日/四段能力说明/五 CTA/法务页脚，文案对齐 `home.seo*` zh 值）；浏览器端按会话 cookie（`avrag.auth.session=1`）分流 `/chat` 或 `/login`（与 Next HomeClient 行为一致；Tauri 分流属桌面宿主，不在 web 路径）。组件带 `locale` 参数，`/en`（E4.4）直接复用。
2. **JSON-LD 结构化数据**：`OrganizationJsonLd`（Organization + WebSite）、`SoftwareApplicationJsonLd`（zh/en 文案）、`FaqPageJsonLd`（与 FAQ 页可见问答同源）。挂载：`PublicSeoHead`（全部公开页 = Next (open)/(marketing) layout 语义）+ FAQ 页 + 首页。script 正文 `inner_html` 注入原始 JSON，避免转义破坏 JSON-LD。
3. **抓取协议静态路由（web-server 层）**：`/llms.txt`（agent-readable 站点索引，`assets/site/llms.txt`）、`/baidu_verify_codeva-THd6TRYMwv.html`（站长验证位串）。
4. **不变量**：
   - 根页面冷启动跳转语义保持（登录 → /chat；未登录 → /login），Playwright 各验一例。
   - JSON-LD 内容与页面可见文本同源，不发明新声明。
   - AppRoute::parse("/") 由 NotFound 改为 Home（测试同步更新）。

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E4.3.1 | `json_ld.rs`：三个 JSON-LD 组件（serde_json 构造 + inner_html script） | 已完成 | SSR HTML 中 script 原文可见（Playwright textContent 断言） |
| E4.3.2 | `PublicSeoHead` 挂 org+software（全部公开页生效）；FAQ 页挂 FAQPageJsonLd | 已完成 | FAQ 页 script 含 FAQPage 与问题原文 |
| E4.3.3 | `marketing/home_page.rs`：ProductHomePage(zh/en) + HomePage；app.rs `/` 换绑；routes.rs `Home` 变体 + parse 测试 | 已完成 | 未登录 → /login、已登录 → /chat 两例通过 |
| E4.3.4 | web-server：`/llms.txt`、`/baidu_verify_*.html` 静态文本路由（E4.2 已随第三方法务下载路由先行合入） | 已完成 | 内容与 Content-Type 断言通过 |
| E4.3.5 | `home-journey.spec.ts` 6 例（无 JS SSR / 双向分流 / llms.txt / 百度验证 / FAQ JSON-LD） | 已完成 | 全量 Playwright 67/67 |
| E4.3.6 | 矩阵 rows 69-71 + 汇总行更新、图谱更新、本地提交 | 已完成 | 提交号见 git log |

## 3. 验证证据

证据目录 `docs/engineering/_reports/2026-09-05-e4-3/`：

- `cargo test -p web-sdk -p web-ui`：24 套件全部 0 failed
- `cargo check` ssr / wasm hydrate：exit 0、零警告
- `cargo leptos build`：exit 0
- `pnpm exec playwright test`：**67 passed / 0 failed**（61 + 6）

## 4. 剩余问题

- `/en/*` 双语 13 端点（E4.4，最后一个 E4 切片）。
- robots.txt / sitemap.xml 在 Next 站点中不存在对应源文件（矩阵亦无行），未创建；llms.txt 中「见 robots.txt」的表述与 Next 产物一致地保留。
- MarketingShell 营销导航壳未移植（公开页统一轻壳）。

## 5. 图谱状态

`code-review-graph update` 已执行。
