# E4.1 任务记录：公开帮助与集成生态路由族 (`/help/faq`, `/help/compare`, `/help/api-access*`, `/integrations/*`)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 负责人 | Agent / Solo Trunk |
| 关联计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) §6 E4 |
| 门禁目标 | **E4.1 切片完成**（公开站第一波，E4 剩余 25 端点） |

---

## 1. 任务目标与交付范围

按路由矩阵 §8 挂载公开帮助与集成生态 8 端点（zh-CN 内容），内容逐字对齐 `frontend_next` 单一数据源：

1. **帮助族 (4)**：`/help/faq`（12 条问答 + 证据区）、`/help/compare`（中立对照表 6 维度 + 下一步互链）、`/help/api-access`（人类接入说明 + 自动化步骤）、`/help/api-access/agents`（Agent 可读文档，`assets/docs/api-access-for-agents.md` 编译期内嵌 + `render_assistant_markdown` 渲染，剥文档级 `#` 防双 H1）。
2. **集成族 (4)**：`/integrations` 索引（三张承接卡 + 证据互链）、`/integrations/mcp`、`/integrations/cursor`、`/integrations/claude-desktop`（承接文档页：段落 / 表格 / 代码块 / 边界 bullets；事实源锚定 Agent 文档）。
3. **SEO 头（对齐 Next metadata）**：逐页 `leptos_meta` Title + description + `rel=canonical` + `hreflang=zh-CN/en/x-default`（集成族无 en，与 Next 一致）。
4. **不变量**：
   - 内容单一数据源移植进 `public_content.rs` 静态结构，不在组件里发明新声明；更新日期 / 署名行与 Next 一致。
   - 公开页无鉴权壳（open light chrome）；互链仅指向已挂载路由，无死路径。
   - **已知偏差（待 E4 后续切片恢复）**：Next 指向 `/desktop`（矩阵 row 47，公开桌面下载页，未挂载）的按钮/证据链接，E4.1 暂指已挂载的 `/desktop/buy`；hub 作者外链（`getHubOrigin()/#studio`）暂渲染纯文本。两者均在挂载 row 47 后恢复。
   - en 内容不并入（`/en/*` 属 E4.4），内容结构已预留。
   - `/help` 应用内帮助中心新增「API 接入」卡（api-access 为 nav-config 目的地，help 族互链即其规范入口）。

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E4.1.1 | 盘点 Next 源（4 page.tsx + faq/compare/integrations 内容 + api-access i18n + agent md + shared 组件） | 已完成 | 本切片内容全部取自下列单一源 |
| E4.1.2 | `assets/docs/api-access-for-agents.md` 拷贝 + `public_content.rs`（FAQ/Compare/ApiAccess/Integrations 静态数据 + `include_str!` 文档） | 已完成 | 内容与 Next zh 文案逐字一致 |
| E4.1.3 | `public_common.rs`（PublicSeoHead / PublicPageHeader）+ 4 个 help 公开页组件 | 已完成 | data-testid + SEO 头齐全 |
| E4.1.4 | `components/integrations/`（索引 + 承接文档页 + 3 slug 包装） | 已完成 | 未知 slug 走 Routes 404 兜底 |
| E4.1.5 | 路由：app.rs 8 条 Route + routes.rs 8 个 AppRoute 变体 + 解析测试（含 `/help/nope`、`/integrations/nope` → NotFound） | 已完成 | nav_parity 保持绿 |
| E4.1.6 | `chat-poc.css` 追加 `pub-*` 段（含 `.pub-doc-body` markdown 渲染样式，token-only） | 已完成 | style_baseline_guard 通过 |
| E4.1.7 | `public-help-journey.spec.ts` 6 例（渲染 / 表格 / 代码块 / canonical / 互链闭环） | 已完成 | 全量 Playwright 54/54 |
| E4.1.8 | 验证收敛、矩阵 8 行 + 汇总行更新、图谱更新与本地提交 | 已完成 | 见 §3 |

## 3. 验证证据

命令与退出码存 `docs/engineering/_reports/2026-09-05-e4-1/`：

- `cargo test -p web-sdk -p web-ui`：24 个套件全部 0 failed
- `cargo check -p web-server --features ssr`：exit 0
- `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate`：exit 0
- `cargo leptos build`（wasm-bindgen-cli 0.2.127）：成功
- `pnpm exec playwright test`：**54 passed / 0 failed**（48 既有 + 6 新增）
- 测试环境：WSL2，`CARGO_BUILD_JOBS=2`

## 4. 剩余问题

- `/desktop`（row 47）公开桌面下载页未挂载：FAQ/compare/integrations 的「免费客户端」链接暂指 `/desktop/buy`，挂载后恢复 `/desktop`。
- hub 作者外链（品牌站 origin）待营销站路由族处理。
- `/en/*` 双语路由与 en 内容、结构化数据（JSON-LD）、robots/sitemap/manifest/llms.txt、OG/验证文件、状态码与重定向属 E4 后续切片。

## 5. 图谱状态

`code-review-graph update` 已在根目录执行。`.code-review-graph/` 不入库。
