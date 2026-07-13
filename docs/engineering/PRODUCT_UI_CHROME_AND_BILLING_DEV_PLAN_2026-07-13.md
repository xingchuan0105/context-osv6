# Product UI Chrome + 账单收口 — 开发计划

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-13 |
| 状态 | **Active** — 部分 Done，按波次推进 residual |
| 约束 | Solo 本地 trunk；L1 / 针对性 `pnpm test` / `cargo test -p …`；不扩 CI 剧场 |
| 产品铁律 | `workspace` 唯一真相（禁止用户可见 notebook）；支付仅 **Creem + Alipay**（禁止 Stripe） |
| 权威审计 | [PRODUCT_UI_CHROME_AUDIT_2026-07-13.md](./PRODUCT_UI_CHROME_AUDIT_2026-07-13.md) |
| 支付硬切 | [STRIPE_BILLING_REMOVAL_2026-07-13.md](./STRIPE_BILLING_REMOVAL_2026-07-13.md) |
| 用量单位 / DeepSeek 三桶 | [设计 v2](./DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md) · [开发计划](./DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md)（Ready；与 Chrome Wave **可并行**） |
| 工程纪律 | [SOLO_DISCIPLINE.md](./SOLO_DISCIPLINE.md)、根目录 `AGENTS.md` §7–§8 |

---

## 0. 目标（一句话）

把 **全局顶栏 / 创建语义 / 账户 / 法律与品牌入口 / 账单管理** 收到同一套 Taxonomy 与放置图；支付路径只保留 Creem + 支付宝；用小步可验收波次在本地 trunk 落地，不回退 Product App 架构。

---

## 1. 基线（已完成 — 勿回退）

### 1.1 UI Chrome（部分）

| ID | 项 | 状态 | 关键落地 |
|----|----|------|----------|
| B-UI-1 | 顶栏「新建笔记本」→ 工作区文案 | **Done** | `workspace-top-bar.tsx` + i18n |
| B-UI-2 | Dashboard/Settings 产品页脚（法律/帮助/定价/开源） | **Done** | `product-chrome-footer.tsx` |
| B-UI-3 | Dashboard 品牌官网 + Context-OS 标题链工作台 | **Done** | `dashboard-header.tsx` + `NEXT_PUBLIC_BRAND_HOME_URL` |
| B-BILL-1 | 管理订阅 = 应用内方案 +「更换方案」→ `/pricing` | **Done** | `settings-billing-panel.tsx` |
| B-U-1 | 5h/7d 计量 RLS 修复（写入+读取 set_current_user） | **Done** | `pg_usage_limit_store.rs`、`core_usage.rs` |
| B-U-2 | 去掉账单页「令牌/文档 / 未设置」双轨 | **Done** | `settings-billing-panel.tsx` |
| B-U-3 | 个人用量副标题解释用量单位 | **Done** | `settings.ts` i18n |
| B-U-4 | DeepSeek 三桶 + 分档 M + 限额倒推 + 约 tokens 展示 | **计划 Ready** | [DEV_PLAN Wave 0–5](./DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md)；**不阻塞** Chrome Wave 1 AccountMenu |

### 1.2 Stripe 支付硬切

| ID | 项 | 状态 |
|----|----|------|
| B-PAY-1 | 删除 `StripeClient` / 配置字段 / checkout·portal·webhook 产品路径 | **Done** |
| B-PAY-2 | `/webhooks/stripe` → 410；portal-session 固定 unavailable | **Done** |
| B-PAY-3 | 移除记录文档 | **Done** — `STRIPE_BILLING_REMOVAL_2026-07-13.md` |

**禁止回归：** 重新引入 Stripe 结账/门户/验签处理；用户可见「笔记本」作主名。

---

## 2. 目标 Taxonomy 与放置（开发契约）

实现时必须对照；偏离需在 PR/提交说明中写清。

| ID | 语义 | 文案 zh | 放置 |
|----|------|---------|------|
| G-CREATE-WS | 新建工作区 | 新建工作区 | **仅 Dashboard 主 CTA**；Workspace **不得**再做实心主按钮 |
| C-CREATE-SESSION | 新建会话 | 新建会话 | 仅左 History（`+` 必须带文字） |
| C-ADD-SOURCE | 添加资料 | 添加资料 | 右栏 Sources |
| C-CREATE-NOTE | 新建笔记 | 新建笔记 | 右栏 Notes |
| G-ACCOUNT | 账户菜单 | **账户** | Dashboard + Workspace **同一组件**（资料/账单/登出） |
| G-APPEARANCE | 外观 | 外观 | 顶栏快捷 **或** 仅链 Settings（二选一，本计划选 **顶栏保留 popover + Settings 权威**） |
| W-SHARE | 分享 | **分享**（替换「传播」默认） | Workspace 次要 |
| W-API | API | API | Workspace 次要 |
| W-ANALYZE | 分析 | 分析 | 有路由则挂；否则删死 i18n |
| G-FOOTER | 法律/帮助/品牌 | 页脚链接族 | Dashboard / Settings / **Workspace 壳** |

```
Global: Brand(官网) | 上下文标题 | … | Share API [Analyze?] | Appearance | Account▾
Dashboard 独有: [+ 新建工作区]
Workspace 顶栏: 无实心「再建工作区」；可选 ghost/菜单项

History: [+ 新建会话]  Rail: [+ 添加资料] [+ 新建笔记]
Footer: 官网 · 工作台 · 帮助 · 定价 · 法律中心 · 协议 · 隐私 · 开源
```

---

## 3. 波次编排（DAG）

```text
Wave 0  文档与基线冻结 ──────────────────────────┐
Wave 1  账户菜单统一 (G-ACCOUNT)  ───────────────┤
Wave 2  Workspace 顶栏创建降权 (G-CREATE-WS)  ───┤  可并行 1∥2 后
Wave 3  文案/i18n/Share 命名 + Analyze 决策  ────┤
Wave 4  Workspace 页脚 + API Access i18n  ──────┤
Wave 5  notebook residual + Admin/e2e  ──────────┤
Wave 6  （可选）Stripe schema residual 硬删  ────┘  独立 O-wave，默认不做
```

依赖：

- Wave 1 **不依赖** Wave 2；建议先 1（账户心智）再 2（创建心智）。  
- Wave 3–5 依赖 Taxonomy 文案 key 稳定（Wave 1–2 后改 key 成本更低）。  
- Wave 6 **仅**在明确要清库表时启动；默认 **Out of scope**。

---

## 4. Wave 明细

### Wave 0 — 文档与基线冻结（0.5d）

| 任务 | 验收 |
|------|------|
| 本计划 + 审计 + Stripe 移除文档互链 | 三份 doc 状态一致 |
| `.env` 运维：删除 `STRIPE_*`（本机/VPS） | 无运行时依赖 Stripe |
| 冒烟：Settings 管理订阅展开方案；页脚可点 | 手工 5 分钟 |

**状态建议：** 文档已齐 → Wave 0 视为 **Ready / 可标 Done**（运维 env 由人确认）。

---

### Wave 1 — 全局账户菜单（P0 剩余）**【优先】**

**目标：** Dashboard 与 Workspace 同一 `AccountMenu`：图标 user、文案「账户」、菜单含 个人资料 / 账单 / 登出（可选：法律中心、帮助）。

| 步骤 | 文件（预期） | 说明 |
|------|----------------|------|
| 1.1 | 新建 `components/account-menu.tsx`（或 `product-chrome/account-menu.tsx`） | popover：资料 → `/settings?tab=profile`；账单 → `?tab=billing`；登出 `auth.logout` |
| 1.2 | `dashboard-header.tsx` | 替换直链「账户」为 `AccountMenu` |
| 1.3 | `workspace-top-bar.tsx` | 替换「账号信息」菜单为同一组件；去掉 `dashboardProfileLink` 分裂 |
| 1.4 | i18n | 收敛 key：优先 `dashboardAccountLink` 或新 `productChrome.account`；删/弃用双文案 |
| 1.5 | 测试 | `settings-surface` / dashboard 相关 unit；必要时加 account-menu 单测 |

**验收：**

- 两面可见文案一致为「账户」（en: Account）。  
- Dashboard 可登出。  
- 无「账号信息」残留主路径。

**验证：**

```bash
cd frontend_next
pnpm exec vitest run tests/settings/settings-surface.test.tsx
# 若有 dashboard 单测一并跑
```

---

### Wave 2 — Workspace 顶栏创建降权（P1-4）

**目标：** Workspace 内「再建工作区」不再抢主 CTA；左栏「新建会话」为唯一实心 `+`。

| 步骤 | 说明 |
|------|------|
| 2.1 | 顶栏 `topBarPrimaryButton` 新建工作区 → **ghost / 次要** 或收进「⋯ / 账户旁菜单」 |
| 2.2 | History「新建会话」保持 primary 视觉（可选：图标 `message-plus` 区分，非必须） |
| 2.3 | i18n：顶栏与 Dashboard 均用「新建工作区」同源 key（`workspaceNewWorkspace` / `dashboardNewWorkspace` 收敛） |
| 2.4 | aria-label 与可见文案同源 |

**验收：**

- 新用户在 Workspace 默认视线落在「新建会话」而非「再建工作区」。  
- 仍可从某处创建新工作区（菜单项或 Dashboard）。

**验证：** workspace surface / history 相关 unit + 手工。

---

### Wave 3 — 文案与入口决策（P2 前半）

| 步骤 | 决策默认 | 验收 |
|------|----------|------|
| 3.1 | `workspaceDistribute` 默认文案 **分享 / Share**（或 i18n 改键保留兼容） | 顶栏无「传播」作默认 |
| 3.2 | Analyze：若路由 `/dashboard/[id]/analyze`（或现有 analyze 路径）可用 → 顶栏次要入口；否则 **删除** 未挂 i18n 或文档标注「未挂载」 | 无死文案 |
| 3.3 | 品牌 `Context-OS` 全站一致（Admin shell 若不一致则改） | 无 `Context OS` 混用（用户可见） |
| 3.4 | Source/Note 文案表：中文「添加/新建」策略写进 i18n 注释 | 中英不打架 |

**验证：** i18n 抽检 + 顶栏手工。

---

### Wave 4 — Workspace 页脚 + API Access i18n（P1-7/8）

| 步骤 | 说明 |
|------|------|
| 4.1 | `workspace-surface` 挂 `ProductChromeFooter`（或账户菜单已含法律时可仅菜单 + 精简 footer） |
| 4.2 | `workspace-api-access-surface.tsx` 硬编码中文 → `formatUiMessage`（新 keys 放 `workspace.ts` 或 `help.ts`） |
| 4.3 | 返回链统一「返回工作区」类 key |

**验证：**

```bash
pnpm exec vitest run tests/workspace/  # 触及的子集
# API access 若有测试一并跑
```

---

### Wave 5 — notebook residual + Admin/e2e（P2 后半）

| 步骤 | 说明 |
|------|------|
| 5.1 | `data-testid="notebook-list"` → `workspace-list`；同步 e2e POM |
| 5.2 | Admin copy / 英文 notebooks 残留（用户可见优先） |
| 5.3 | e2e helpers 变量名 notebook → workspace（不改行为） |
| 5.4 | 清理 `page-frame` placeholder 误用风险（删除或隔离） |

**验证：** e2e 命名更新后 smoke 可选；`rg notebook` 用户可见字符串趋近零。

---

### Wave 6 — （可选）Stripe schema residual 硬删

**默认：不做。** 仅当需要 schema 洁癖时：

1. 盘点 `billing_provider='stripe'` 行 → 标记 canceled / 迁移通知。  
2. 新 migration：DROP `stripe_customer_id` 等（注意 0035/0036 历史）。  
3. 删除 `BillingProvider::Stripe`、端口 `save_stripe_customer_id`、死代码 `subscription.rs` Stripe 辅助（现已有 dead_code 警告）。  

**验收：** 迁移可逆说明 + 无 Stripe 字符串于产品配置。

---

### Wave U — 用量计量修复 + 账单 UI 可读（2026-07-13 增补）**【优先于部分 P2】**

**用户反馈（截图 10/11）：** 5h/7d 显示 `0/100K`、`0/400K`（失灵）；下方「令牌 132.9K / 未设置」「文档 149 / 未设置」难懂且与配额无关。

#### 诊断（根因）

| 表面 | 数据源 | 问题 |
|------|--------|------|
| **5h / 7d 个人用量** | `llm_usage_events.usage_units`（`billable=true`）via `/api/auth/usage-limit` | 表上 **FORCE RLS**（`owner_user_id = app.current_user`），但 `PgUsageLimitStoreAdapter` **插入/汇总未 set_config** → 写入失败静默、读取恒 0 |
| **令牌 / 文档** | 旧表 `usage_events` 月聚合 via `/api/v1/billing/usage` | **第二套数字**；前端 `limit_tokens/documents` **写死 0** → 永远「未设置」；与 rolling 配额无关 |
| **产品真相** | ADR-0006：rolling `usage_units` 为权威 | UI 必须只展示 rolling；禁止第二套对账数字 |

**用量单位（给产品/用户的白话）：**

- 不是「原始 token 个数」直接当限额。  
- `usage_units ≈ ceil(输入 token/1000 × 1 + 输出 token/1000 × 2)`（可按模型权重表调整）。  
- Free：默认 **5h = 100K units**，**7d = 400K units**（`usage_limit_plan_policies`）。  
- Worker 内部 embedding 等可写 `billable=false`，**不进**客户配额。

#### 任务

| ID | 任务 | 验收 |
|----|------|------|
| U-1 | `pg_usage_limit_store` 所有 `llm_usage_events` 读写 txn + `set_current_user(owner)` | 新对话后 5h/7d **used > 0** |
| U-2 | `billing_sql` 窗口汇总同样 set RLS | `usage_window` API 一致 |
| U-3 | 账单页 **删除**「令牌/文档 / 未设置」区块 | Settings 账单无双轨数字 |
| U-4 | 个人用量副标题说明「用量单位 + 窗口」 | 用户可看懂 |
| U-5 | 更新本计划 + 审计 doc | 诊断可追溯 |
| U-6 | （可选后续）breakdown 展示 feature 分项时用中文 | 非阻塞 |

**验证：**

```bash
# 对话一轮后
# SELECT sum(usage_units) FROM llm_usage_events WHERE billable;  -- 需 set app.current_user
# Settings 个人用量 5h/7d used 非 0
cargo check -p app-bootstrap
pnpm exec vitest run tests/settings/settings-surface.test.tsx
```

**状态：** U-1–U-5 **Done**（2026-07-13）。历史已产生的 0 用量需 **重启 API 后新对话** 才会写入 `llm_usage_events`；旧会话不会回溯。

---

## 5. 非目标

- 不重做 Product App 架构；不新增 AppState 业务方法。  
- 不引入新支付提供商（含 Stripe）。  
- 不强制真卡支付 E2E / 全 Playwright 作为波次门禁。  
- 不统一 Admin 与 B2C 的完整图标体系（可 P2 另立）。  
- 不新建独立 docs 站（产品帮助仍为 `/help`）。

---

## 6. 每波交付物与 Definition of Done

| 项 | 要求 |
|----|------|
| 代码 | 行为符合 Taxonomy；i18n 无新增硬编码中文主路径 |
| 测试 | 触及包的 unit 绿；不强制全仓 Playwright |
| 文档 | 本计划波次表勾选 Done；重大偏离写回审计 doc |
| Git | 本地 commit 即可（Solo）；push 仅备份/部署时 |

**单波 DoD 模板：**

1. 验收表 3–5 条手工步骤写清。  
2. `pnpm exec vitest run <相关>` 或 `cargo test -p …` 通过。  
3. `rg` 抽检：禁止字符串（如「新建笔记本」、`StripeClient`）。

---

## 7. 建议执行顺序（Solo 日历）

| 顺序 | 波次 | 预估 | 说明 |
|------|------|------|------|
| 0 | **Wave U** | 0.5d | **用量 RLS 修复 + 去掉令牌/文档双轨**（用户现报） |
| 1 | Wave 1 | 0.5–1d | 账户菜单，用户感知最强 |
| 2 | Wave 2 | 0.5d | 创建语义，与审计截图问题同族 |
| 3 | Wave 3 | 0.5d | 文案与 Analyze 决策 |
| 4 | Wave 4 | 0.5–1d | Workspace 页脚 + API Access |
| 5 | Wave 5 | 0.5d | residual 命名 |
| — | Wave 6 | 1d+ | 仅明确要做时 |

合计 **约 3–4.5 人日**（不含 Wave 6）；Wave U 优先于 UI 打磨。

---

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 账户菜单抽组件破坏现有菜单样式 | 先复制 Workspace 菜单交互再抽；Dashboard 只换调用 |
| 顶栏降权后找不到「新建工作区」 | 菜单保留入口 + Dashboard 主 CTA 不变 |
| i18n key 双套（shell vs 运行时） | Wave 3 收敛一张表，禁止新增第三套 |
| e2e testid 改名断流水线 | Wave 5 同提交改 POM；本地 e2e 可选 |
| Stripe 残留 SQL 误导新人 | 文档 + 注释 `BillingProvider::Stripe` residual only |

---

## 9. 验证命令速查

```bash
# Frontend（按波次收窄）
cd frontend_next
pnpm exec vitest run tests/settings/settings-surface.test.tsx
pnpm exec vitest run tests/workspace/workspace-surface.integration.test.tsx  # 若触及

# Billing / backend
cd avrag-rs
export CARGO_BUILD_JOBS=2
cargo test -p avrag-billing --lib
cargo check -p app-bootstrap -p transport-http

# 回归抽检
rg -n "StripeClient|新建笔记本|STRIPE_SECRET" \
  frontend_next avrag-rs/crates --glob '!**/target/**' --glob '!**/node_modules/**'
```

---

## 10. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-13 | 初稿：据 UI Chrome 审计 + Stripe 移除文档编排 Wave 0–6；基线 Done 项固化 |
| 2026-07-13 | **Wave U**：5h/7d RLS 失灵诊断与修复计划；去掉令牌/文档双轨；执行顺序把 U 提到最前 |

---

## 11. 开工检查清单（下一位 Agent / 自己）

- [ ] 读完本计划 §1–§4 与两份上游 doc  
- [ ] 确认本地无 `STRIPE_*` 依赖  
- [ ] 从 **Wave 1** 开工，勿跨波大爆炸  
- [ ] 每波结束更新本表状态（Done + 日期）  
- [ ] 禁止引入 notebook 主名与 Stripe 支付路径  
