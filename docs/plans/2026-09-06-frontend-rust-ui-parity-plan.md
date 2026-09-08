# frontend_rust 呈现层对齐（E5）：开发计划

日期：2026-09-06。状态：**E5.1 / G5.1、E5.2 / G5.2、E5.3 / G5.3、E5.4 / G5.4、E5.5 / G5.5 达成**。G5 总门未开始。证据：`docs/engineering/_reports/2026-09-07-e5-1/`、`docs/engineering/_reports/2026-09-07-e5-2/`、`docs/engineering/_reports/2026-09-08-e5-3/`、`docs/engineering/_reports/2026-09-08-e5-4/`、`docs/engineering/_reports/2026-09-08-e5-5/`。本计划是 [2026-09-05 执行计划](2026-09-05-development-execution-plan.md) 的**续章补丁**：E1–E4 已完成 71/71 路由挂载与数据管道（G2/G3/G4 达成），但当日代码级审阅确认呈现层（壳 / 视觉深度 / 交互件 / 文案）距 `frontend_next` 成品效果差距大。本计划新增 **E5 轨道（呈现层对齐）**；W5 切流评审前置条件由 G3/G4 扩展为 **G3/G4/G5**。

对齐基线（冻结）：`frontend_next` 以 **`5f9dedda`** 为对比基线。基线之后 Next 的演进不在本计划范围；如基线移动需修订本文。

## 0. 现状审阅摘要（2026-09-06，代码级证据）

骨架层（路由、鉴权、数据流、SSE）约 70–80%；呈现层约 20–30%。关键差距：

| 轴 | Next 基线 | Rust 现状 | 关键证据 |
|---|---|---|---|
| CSS 体量 | ~13,000 行 / 30 模块 / 1,139 选择器 | 1 文件 2,711 行 / 323 选择器（自称 "PoC 布局样式"） | `assets/style/chat-poc.css` |
| 交互状态 | hover 135 · focus 42 · active 22 | hover 14 · **focus 0 · active 0** | 同上 |
| 动效/阴影/骨架屏/断点 | transition 54 · keyframes 33 · shadow 58 · @media 42 | 2 · **0** · **0** · 1 | 同上 |
| 图标系统 | 16 个 SVG 组件文件 | **0，全文字按钮** | `components/workspace/chat-icons.tsx` 等 |
| 应用壳 | AppTopBar + 账户菜单 + 通知 + 命令面板 + 页脚；PRODUCT_IA §5 全合规 | **无共享壳**；每页裸返回链；`/chat` 导航链接数 0 | `web-ui/src/app.rs` 直挂页面 |
| 硬编码占位页 | 0 | **4 处**：settings/usage、workspace_analytics、settings profile/preferences 文案、pricing 视图（已 fetch 真实 plans 却渲染硬编码卡片） | `usage_page.rs:20-27`、`workspace_analytics.rs:42-84`、`pricing_page.rs:91-165` |
| 交互件 | ~10 弹窗组件 · toast · 2 图表 · TipTap | **1 弹窗 · 0 toast · 0 图表 · textarea** | 全 web-ui grep |
| i18n | 全量 zh/en 切换 | 全硬编码中文；`/en/pricing` 文案仍中文 | — |
| 另发现 bug | — | chat-poc.css 引用 **9 个不存在的 token 变量**（静默失效）；无 `body{}` 基础层（字体栈/背景/box-sizing 均未设） | `chat-poc.css` vs `tokens.css` |

注意口径：E1–E4 的「业务闭环」验收针对功能与数据，不含呈现对齐；本文的 gap 不与 G3/G4 结论冲突，是其未覆盖层的补充。

## 1. 目标 / 终态

**终态定义**：同一账号、同一数据下，用户从 `frontend_next` 切换到 `frontend_rust`，完成 §5 的 8 条核心旅程，**不因 UI 缺失而中断，不产生「这不是同一个产品」的感知**。可检验的终态条件：

1. **壳合规**：PRODUCT_IA §5 全部 Shell 规则在 Rust 端逐条落实（App top bar / Chat shell / Dashboard main / Workspace chrome / Settings rail / 深层工具页统一顶栏）；任一页出发 ≤2 次点击到达 §4 全部 canonical 目的地；深层页无「裸返回链作唯一出口」。
2. **视觉深度对齐**：CSS 深度信号（hover / focus / transition / keyframes / 断点）达到 Next 基线的 **≥80%**（按同口径 grep 计数）；0 悬空 token 变量；存在完整 base 层（body 字体栈、背景、box-sizing、全局 `:focus-visible`）。
3. **零硬编码占位**：usage / workspace_analytics / pricing 视图 / settings 面板全部绑定真实 API 数据；占位文案与写死数字清零。
4. **Chat 呈现对齐**：§4 E5.3 gap 表中的「缺失」项清零或逐项记录豁免；十二条 Chat-first 不变量保持全绿。
5. **交互件齐备**：通用 dialog / toast / 空-加载-错误三态成为共享组件并被各页使用；workbench 具备资料上传与会话管理。
6. **双语与响应式**：产品页 zh/en 可切换；断点档位对齐 Next；移动端关键旅程可用。

**非目标**（防 scope creep）：
- 不做像素级复刻；token 已共享，组件视觉允许小幅差异，以「状态齐备 + 结构对齐」为准。
- 不新增 Next 没有的功能（对齐不是超越）。
- 不改 `frontend_next`（它是冻结参照物）；不动后端契约。
- 不追求 Gate 0 性能目标（仍为观察项）。

## 2. 关键约束

| # | 约束 | 来源 |
|---|---|---|
| C1 | Token 单一来源 `packages/cos-tokens/tokens.css`（两端逐字节同步）；Rust style guard 从 2 条扩到 Next `design-baseline.test.ts` 全 5 条（字重 <500、CSS 无裸 hex、组件无裸 hex、阴影仅白名单浮层、无内联样式逃逸），并新增「悬空 CSS 变量 = 0」机械检查 | AGENTS.md 验证默认；Next style baseline |
| C2 | 导航目的地唯一权威 = `PRODUCT_IA.md` §4/§5 + `frontend_next/lib/navigation/nav-config.ts`；Rust 壳所有入口 href 必须落在 §4 表内，并有与 nav-config 对齐的 parity 测试；**改导航先改 PRODUCT_IA**（本计划范围内不预期需要改） | PRODUCT_IA §9 |
| C3 | 禁止第三完成页：checkout 只经 `/pricing`，BYOK 只在 `/settings?tab=providers`，不发明第二路径 | PRODUCT_IA §2/§7 |
| C4 | `web-server` **永不代理 `/api`**（Nginx 直连 avrag-api 不变量）；本地开发走 `LEPTOS_SITE_ADDR=127.0.0.1:18080` + 后端 18081（CORS 白名单）；为「好跑」加代理属红线行为 | 迁移设计不变量 |
| C5 | 分层增长：E5.1→E5.5 顺序推进，每个切片独立 gate、独立本地提交；不得破坏 G2–G4 已绿证据（回归套件每 gate 必跑）；fixture 测试不替代 live smoke | AGENTS.md 设计原则 / 执行计划门禁 |
| C6 | 无 backward-compat 税：被替换的 PoC 样式/页面直接删除，不留兼容层；`rebuild-playwright.log\r` 残留本轨道内清理 | AGENTS.md 设计原则 |
| C7 | 十二条 Chat-first 不变量继续有效：尤其「一套 ChatCanvas / 一套管线」「无隐式建 Workspace」「模型与上下文解耦」——E5.3 不得为凑对齐复制第二套画布 | 执行计划 §2 / Chat-first 设计 |
| C8 | UI 文案遵循 PRODUCT_IA §6 Taxonomy（对话/工作区/本会话文件/充值/官方模型…）；i18n 字典方案从简（复用 Next locale 文案结构，zh 默认），不引重型框架 | PRODUCT_IA §6 |
| C9 | 构建纪律：wasm-bindgen CLI 0.2.127 PATH 作用域、`CARGO_BUILD_JOBS=2`；编译/测试/长任务执行前报时；结构性改动后同会话 `code-review-graph update` | AGENTS.md / rust-resources |
| C10 | 硬编码判定纪律：视图必须渲染 API 数据；营销 fallback 仅在 API 失败时显式降级且带 telemetry（对齐 Next pricing 的 live+fallback 模式），禁止「fetch 了却渲染写死内容」 | 本次审阅发现的反模式 |

## 3. 执行顺序与里程碑

```text
E5.1 视觉地基 → E5.2 全局壳 → E5.3 Chat 呈现 → E5.4 业务页真数据化与交互件 → E5.5 i18n 与响应式 → G5 总门 → 具备 W5 评审资格
```

前置：G3/G4 已达成（满足）。每个切片出口未过则停在该切片，不把失败标成完成。

### E5.1 视觉地基（G5.1）

范围：`assets/style/chat-poc.css` 拆分重构为 `base.css`（reset/body/排版/焦点环/工具类）+ 按域样式文件；修 9 个悬空变量（映射到真实 token 或补入 tokens 同步源）；按钮/输入/链接/卡片/表格五类基础件的 hover/focus/active/disabled 状态矩阵与 transition；style guard 扩到 C1 全 5 条 + 悬空 var 检查脚本。

| 验收 | 标准 |
|---|---|
| 机械检查 | guard 5 条 + 悬空 var = 0 全绿；base 层存在（body 字体栈/背景/box-sizing/`:focus-visible` 全局规则） |
| 计数底线 | `:focus*` ≥ 20、`transition` ≥ 20、`@keyframes` ≥ 3（流式光标等基础动效），向 Next 口径收敛的趋势在 E5.2–E5.4 持续 |
| 回归 | `cargo test -p web-sdk -p web-ui` 全绿；fixture Playwright 全绿；页面视觉无大面积塌陷（人工走查 5 页截图存档） |
| 提交 | 单切片提交；code-review-graph 更新 |

### E5.2 全局壳 AppShell（G5.2）

范围：以 `admin_shell.rs` 模式为模板新建共享壳组件——`AppTopBar`（品牌→`/chat`、分享组菜单[访问/API/升级]、通知*、账户菜单[用户卡/设置/帮助/管理台探测/登出]）；Chat 会话栏补 Workspaces 区与「全部工作区」入口；`ProductChromeFooter`；Marketing chrome（定价·客户端·法律·语言·进入应用，含 active 态）；深层工具页（analytics / share 中心 / usage / help/*）统一接入 AppTopBar + 对象级 breadcrumb。通知：若后端无通知 API，记录豁免并渲染空态入口，不伪造数据（C10）。

| 验收 | 标准 |
|---|---|
| 可达性矩阵（Playwright） | 从 `/chat`、`/dashboard`、`/dashboard/:id`、`/settings`、`/pricing`、`/help`、任一深层页出发，≤2 次点击到达 §4 全部 canonical 目的地；`/chat` 导航链接数 0 → ≥壳内完整入口 |
| IA 合规 | PRODUCT_IA §5 逐行 checklist 打勾存档；壳入口 href 100% 落在 §4 表（机械测试，对齐 nav-config） |
| 反模式 | 深层页「裸返回链唯一出口」= 0；孤立页 = 0 |
| 回归 | G2/G3 相关 fixture journey 全绿 |

### E5.3 Chat 呈现层（G5.3）

范围（对照审阅 gap 表逐项关闭或豁免）：hero 空态（品牌标题+模式提示+居中 composer）；composer 自动伸缩 + 拖拽手柄（ARIA slider 语义）；代码块语言标签 + 复制按钮；图片 figure 卡；`tool_result` 工具卡渲染（当前 web-ui 零匹配）；web 来源计数按钮 + 弹窗；流式光标 + 打字机平滑；进度耗时计时；自动滚动 + 「回到底部」；用户消息编辑进 composer；degrade/guard 通知条；会话栏 loading / error+retry / empty 三态与流式中导航锁；移动端会话抽屉。

| 验收 | 标准 |
|---|---|
| gap 表 | 「缺失」项清零或逐项豁免记录（豁免需理由 + 负责人决策） |
| Playwright | chat journey 新增断言 ≥ 10（hero、代码块复制、工具卡、web 来源弹窗、自动滚动、会话栏三态、移动抽屉） |
| 不变量 | 十二条 Chat-first 不变量逐条回归证据；仍一套 ChatCanvas |
| live | live smoke 全绿（真实后端流式肉眼走查一次存档） |

### E5.4 业务页真数据化与交互件（G5.4）

范围：
- **pricing**：视图绑定已 fetch 的 plans/packs 信号（API 失败才走营销 fallback）；支付对齐 Next 页内二维码弹窗（Alipay/Creem 选择 + 推荐徽标 + 月/年切换 + FAQ + 同意勾选）。
- **settings/usage**：真数据 + 用量趋势 SVG 图 + 额度表（对齐 Next `usage-dashboard-client`）。
- **`/dashboard/:id/analyze`**：对齐 Next 现行行为（重定向至分享中心），**删除硬编码分析页**；dashboard 总览补 tabs（全部/我的/收藏）、排序、卡片/列表切换、搜索弹窗、收藏/重命名/删除项菜单与确认弹窗、空态/骨架屏。
- **workbench**：资料上传弹窗（上传/链接/粘贴 tabs）、资料列表多选与状态、会话管理（pin/重命名/删除 + URL 同步）、空态三态；笔记编辑器按 E1 Tiptap 接入边界分析落地（最大单点，必要时拆 E5.4b 子切片）。
- **settings**：补 billing / security 两个 tab；profile 面板（头像/资料表单）；preferences 面板（主题/语言，接 E5.5 字典）。
- **共享件**：通用 `Dialog` / `Toast` / 空-加载-错误三态组件，各页接入；admin 补指标卡、状态徽章、搜索/排序筛选层（图表豁免项见 §6）。

| 验收 | 标准 |
|---|---|
| 占位清零 | 硬编码清单 grep = 0：`"98.5"`、`"42"` chunks、`"128"` views、`"0"` usage 卡、pricing 写死档位文案等逐项列出并消除 |
| 三态 | 每个数据页存在空 / 加载 / 错误三态且有 journey 断言 |
| 交互件 | `role="dialog"` ≥ 6 处、toast 组件存在并被 ≥3 页使用；workbench 上传 journey 通（fixture + 一次真实文件 live） |
| 回归 | billing / workspace / auth-settings / admin fixture journey 全绿 |

### E5.5 i18n 与响应式（G5.5）

范围：Leptos 字典方案（复用 Next locale 结构，zh 默认）；账户菜单语言切换；产品页（壳/chat/dashboard/settings/billing/auth）zh/en 全量；`/en/pricing` 中文文案修复；断点档位对齐 Next（桌面三档 + 移动）；移动视口 chat / dashboard 旅程。

| 验收 | 标准 |
|---|---|
| i18n | 语言切换 journey 通；壳与 chat 无硬编码中文字符串残留（抽查 + 基线清单）；`/en/pricing` 全英文 |
| 响应式 | `@media` 断点档位与 Next 对齐（≥3 档 + 移动抽屉）；移动视口 chat / dashboard fixture journey 通 |
| 偏好 | `prefers-reduced-motion` 下动效关闭（token 已有块，组件遵守） |

## 4. G5 总门（终态验收）

1. **旅程验收**：以下 8 条在 Rust 端 fixture + live 双轨完成（J 编号对应 PRODUCT_IA §1）：J0 无库直聊（含会话文件）、J1 建库持续问答、J2 BYOK 闭环、J3 充值/升级、J4 分享开链 + 访客问答、J5 分享效果查看、J8 设置与安全、管理台巡检。每条旅程两端（Next 基线 vs Rust）各走一遍，步骤数与完成态一致。
2. **感知走查**：同一账号同数据，8 个关键页面两端截图对照存档；结构/状态/密度对齐（非像素）由评审人签字。
3. **机械门**：C1 全部检查绿；§1 终态条件 2/3 的计数阈值达标；E5.1–E5.5 各 gate 证据归档。
4. **回归门**：`cargo test -p web-sdk -p web-ui`、fixture 全套、live smoke 全绿；G2–G4 证据未失效。
5. 通过后：更新执行计划状态头（W5 前置 = G3/G4/G5），本计划标记达成。

## 5. 风险与开放问题（2026-09-06 已全部裁决）

| # | 问题 | 裁决 |
|---|---|---|
| R1 | 通知系统：Next 有 NotificationBell，Rust 需要后端通知 API——是否存在？ | **存在,E5.2 实现。** 已核实:后端 `GET /api/v1/notifications`（需用户会话,`list_notifications_handler` → `NotificationsResponse { notifications: [NotificationRow] }`）与 `POST /api/v1/notifications/{id}/read`（标记已读）均已上线;contracts `NotificationRow`/`NotificationsResponse` 已有 typeshare 契约。E5.2 通知铃铛 = 列表 + 标记已读 + 空态,不豁免 |
| R2 | i18n 体量：Next 全量 locale 是 LOC 差大头 | **全量。** E5.5 按原案执行,不做范围收缩 |
| R3 | TipTap 编辑器（workbench 笔记）是 E5.4 最大单点 | **直接做。** 不拆 E5.4b;E5.4 gate 以含 TipTap 的完整 workbench 验收 |
| R4 | 图表（UsageTrendChart / ShareViewsBarChart 手卷 SVG）移植成本 | **移植。** E5.4 移植两个 SVG 图表;G5 走查按「对齐」口径,无简化项 |
| R5 | Next 是活基线：已冻结 `5f9dedda`；若基线必须移动 | 修订本文并重记受影响切片的验收对照 |

## 6. 工作量粗估（非工期承诺）

| 切片 | 预估有效开发时间 | 主要变量 |
|---|---|---|
| E5.1 | 1–2 个工作日 | 样式域拆分范围；guard 扩展误报处理 |
| E5.2 | 2–3 个工作日 | 通知 API 有无（R1）；账户菜单/分享组菜单交互深度 |
| E5.3 | 3–5 个工作日 | 工具卡/web 来源的事件契约细节；打字机与自动滚动调优 |
| E5.4 | 5–8 个工作日 | TipTap（R3）；workbench 上传链路真实后端联调；settings 四面板 |
| E5.5 | 2–4 个工作日 | R2 裁决范围 |
| **合计** | **13–22 个工作日** | 不含 R1 后端任务 |

口径与 E0 一致：这是预估有效开发时间，主要变量已列出，不是实测工期承诺。Gate 未过停在当前切片，只推进不依赖失败项的准备工作。
