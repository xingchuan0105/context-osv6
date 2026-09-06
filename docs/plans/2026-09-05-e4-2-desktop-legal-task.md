# E4.2 任务记录：桌面产品与法务公开族 (`/desktop`, `/activate`, `/setup`, `/legal*`)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 负责人 | Agent / Solo Trunk |
| 关联计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) §6 E4 |
| 门禁目标 | **E4.2 切片完成**（E4 剩余 16 端点） |

---

## 1. 任务目标与交付范围

按矩阵 §9/§10 挂载桌面生态与法务公开 9 端点 + 1 条静态下载路由：

1. **桌面族 (3)**：`/desktop`（产品页：卖点六条 / 下载按钮 + 发布清单 `/releases/desktop/latest.json` 拉取，未发布呈 Unavailable 态 / 安装步骤 / SmartScreen 提示 / SHA256 详情）、`/activate`（客户端免费无需激活 → 重定向 `/desktop`，ADR-0010）、`/setup`（已退役 → 重定向 `/settings?tab=providers`，PRODUCT_IA §2）。页面组件带 `locale` 参数，`/en/desktop`（E4.4）直接复用。
2. **法务族 (6)**：`/legal` 法律中心（三卡 + 版本日期 + 法务邮箱）、`/legal/terms`、`/legal/privacy`（MDX 正文编译期内嵌 + pulldown_cmark 渲染 + h2/h3 slug 注入 + 目录导航 + 版本/最后更新）、`/legal/licenses` 开源摘要（组件表 6 行 + 弱 copyleft 说明）、`/legal/licenses/project`（MIT 全文）、`/legal/licenses/third-party`（完整声明渲染 + 组件总数统计 + 目录）。
3. **静态路由 (web-server 层)**：`/legal/third-party-notices.md`（licenses 页下载入口）。
4. **不变量与偏差记录**：
   - 法务文本逐字取自 `content/legal/*`（结构、数字、日期、版本不变；不借迁移改写条款）。
   - **E4.1 偏差恢复**：FAQ / compare / integrations 的「免费客户端」链接由 `/desktop/buy` 恢复为规范 `/desktop`（row 47 已挂载）。
   - 文档级 `#` H1 在渲染前剥离，LegalShell 单 H1（Next 为双 H1，本迁移更严格）。
   - dev-only 404：`/releases/desktop/latest.json` 未发布时下载态为「安装包暂未发布」（与 Next 行为一致）。
   - chrome 差异：Next 法务/桌面页用 MarketingShell（营销导航壳），Rust 侧统一 `pub-shell` 轻壳；文本内容对等。

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E4.2.1 | 资产拷贝：`assets/legal/{zh-CN,en}/{terms,privacy}.md`、`third-party-notices.md`、`LICENSE` | 已完成 | 法务文本逐字一致 |
| E4.2.2 | `legal_markdown.rs`：标题 slug 注入 + TOC 提取（含 CJK slug 与重名去重，2 个单测） | 已完成 | web-ui 单测通过 |
| E4.2.3 | `components/legal/`：LegalShell / LegalFooterLinks / 六个法务页面 | 已完成 | canonical/hreflang 齐全 |
| E4.2.4 | `components/marketing/desktop_page.rs`：DesktopProductPage(zh/en) + Activate + Setup；`web-sdk` BrowserRestClient 补 `get_bytes`（wasm fetch / native unavailable 对齐） | 已完成 | 未发布清单 → Unavailable 态 |
| E4.2.5 | web-server：`/legal/third-party-notices.md` 静态路由 | 已完成 | Playwright 下载断言通过 |
| E4.2.6 | 路由 9 条 + AppRoute 9 变体 + 解析测试；`/desktop` 链接恢复 | 已完成 | nav_parity 绿 |
| E4.2.7 | `desktop-legal-journey.spec.ts` 7 例 | 已完成 | 全量 Playwright 61/61 |
| E4.2.8 | 验证收敛、矩阵 9 行 + 汇总行更新、图谱更新、本地提交 | 已完成 | 提交号见 git log |

## 3. 验证证据

证据目录 `docs/engineering/_reports/2026-09-05-e4-2/`：

- `cargo test -p web-sdk -p web-ui`：24 套件全部 0 failed（legal_markdown 单测含在内）
- `cargo check -p web-server --features ssr` / wasm hydrate check：exit 0
- `cargo leptos build`：exit 0（注意：此前一次构建失败被管道掩盖，已改用显式 exit code 检查）
- `pnpm exec playwright test`：**61 passed / 0 failed**（54 + 7）

## 4. 剩余问题

- `/en/*` 双语 13 端点（E4.4）；`/` 首页产品根与 JSON-LD、`/llms.txt`、百度验证文件（E4.3）。
- MarketingShell 营销导航壳未移植（公开页统一轻壳，文本内容对等）。

## 5. 图谱状态

`code-review-graph update` 已执行（42 files, 182 nodes, 1150 edges）。
