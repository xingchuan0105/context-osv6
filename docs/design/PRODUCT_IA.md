# Product IA v1 — Context-OS（登录后 + 营销完成页）

**状态**: 现行权威（Path A，2026-08-06）  
**范围**: SaaS 登录后产品、营销转化页、客户端发现；不含 Admin 后台细部。  
**关联**: ADR-0010 商业模式 · `frontend_next/lib/navigation/nav-config.ts`（**登录后 + 营销 canonical 目的地单一数据源**，footer / 主导航 / Cmd+K / 产品地图从此渲染）· `frontend_next/lib/site-map.ts`（**仅**多站点发现图）· T7 workspace 唯一产品真相  

**规则**: 改 `frontend_next` 主导航 / 全局入口 / 计费完成页前，**必须先改本文**再改代码。禁止新增「第三完成页」。

---

## 1. Jobs（用户意图）

| Job ID | 意图（用户语言） | 成功态 |
|--------|------------------|--------|
| J1 | 建知识库并提问 | 工作区内有来源 + 能对话得到 grounded 回答 |
| J2 | 配置自己的模型（BYOK） | Provider 可用；对话可走自有额度 |
| J3 | 用平台模型 / 访客问答不断供 | 余额足够或已 BYOK |
| J4 | 对外分享知识库 | 分享开启；访客可浏览/提问（Owner-pays） |
| J5 | 看分享效果 | 汇总或单库访问趋势 |
| J6 | 升级可分享名额 | 档位生效，配额提高 |
| J7 | 本机私有 / 给桌面 Agent 用 | 客户端安装 + 可选 MCP/CLI |
| J8 | 管理账户与安全 | 设置完成目标项 |

分组原则：**按意图，不按内部表或团队**。

---

## 2. Object model

```text
User
 ├─ Wallet（余额 · 充值包）
 ├─ Subscription（Free / Plus / Pro · 可分享名额）
 ├─ ProviderSecrets（BYOK）
 └─ Workspace[]（产品唯一容器 · T7）
      ├─ Sources / Notes / Chat
      ├─ Share settings + public surface
      └─ Share analytics（对象级）
Client (desktop) — 同能力本机形态；分享上云仍走 Subscription；模型默认平台 key 走钱包余额（云登录后下发 relay 凭据），BYOK 为登录后高级选项
```

**禁止**新产品对象绕过 Workspace 成为主 API 真相。

---

## 3. Sitemap（树）

### 3.1 营销 / 发现（Marketing shell）

```text
/pricing                 会员档位 + 本页充值（canonical 钱）
/desktop                 客户端介绍与下载（canonical 客户端）
/desktop/buy             历史/授权相关（非主推；客户端免费叙事优先）
/legal/*                 法律
/help                    帮助全文（含上手说明的长文入口）
/help/api-access         Agent/API 接入（公开 SSR，未登录可读；2026-08-11 GEO A3）
/help/api-access/agents  Agent 可读 API 文档（公开 SSR）
/help/faq                产品事实 FAQ（公开 SSR；MCP / 密钥 / 名额 / BYOK / 定价）
/help/compare            选型对比（公开 SSR；Context OS vs 笔记 AI / 第二大脑 / 通用 RAG；中立、无编造竞品数据）
```

多站点 hub/blog/tools → 见 `site-map.ts` + `docs/engineering/MULTI_SITE_IA_INTEGRATION_PLAN_2026-07-14.md`。  
编辑型长文/系列文仍可走 `blog.contextlm.top`（Ghost）；**产品事实型 FAQ 与可引用对比表** 以 app 公开 SSR 为 canonical（GEO Phase C，2026-08-12）。  
**不要**把 family nav 写进登录后 App shell。

### 3.2 认证

```text
/login · /register · /reset-password/*
桌面壳内云登录门            客户端首次打开（非路由；登录后下发官方 key relay 凭据，见 docs/plans/2026-08-15-desktop-cloud-login-official-keys.md）
```

### 3.3 登录后 App（无永久「百科侧栏」）

```text
/dashboard                         工作台 · 工作区列表（主路径）
/dashboard/analytics               分享访问 · 跨库汇总（横切；工具栏入口，非列表筛选 tab）
/dashboard/:id                     工作区（对话 / 来源 / …）
/dashboard/:id?session=:sid        工作区并选中会话（Cmd+K / 外链深链）
/dashboard/:id?source=:src         工作区并打开文档 viewer（Cmd+K 命中；打开后剥离 query）
/dashboard/:id/share               分享中心 = 单库分享设置（链接/邀请 + API 密钥——API 视为一种分享方法）+ 访问趋势 + 访客活动（canonical 对象级）+「分享者主页」全局开关（owner 级，控制 /shared/u/:userId 与分享页入口）
/dashboard/:id/share/analytics     → 301/redirect 至 share（兼容旧链）
/dashboard/:id/analyze             → redirect 至 share（兼容旧链；禁止再实现第二套分析页）
/settings                          默认 tab=profile（账户优先，非付费墙）
/settings?tab=…                   设置（profile|providers|billing|preferences|security）
/settings/usage                    用量（深链；非主导航顶级）
```

### 3.4 旁路 surface（非主导航）

| Surface | 角色 |
|---------|------|
| 升级弹窗 | **说明**会员 vs 充值 → 详情进 `/pricing` |
| 设置快开弹窗 | 短路径编辑；完整页 `/settings` |
| **产品地图弹窗** | **上手 / 模块关系（onboarding）**，**不是** primary nav |
| 分享公开页 `/shared/kb/:token` | 访客 |
| 分享者公开主页 `/shared/u/:userId` | 访客；仅当所有者在分享中心开启「分享者主页」开关后可访问，否则 404 |
| Paywall `/upgrade/*` | 限速恢复路径 → 链回 pricing/settings |

---

## 4. Canonical routes（任务 → 唯一完成页）

| 任务 | Canonical | 允许的入口（只 deep-link） |
|------|-----------|---------------------------|
| 升级会员 | `/pricing`（档位区） | 顶栏分享组「升级」、升级弹窗「详情」、paywall、分享转化「升级」 |
| 充值余额 | `/pricing#topup` | 升级弹窗充值 CTA、分享转化充值、产品地图、设置账单可链回 |
| BYOK | `/settings?tab=providers` | pricing 次要链、产品地图、账单提示 |
| 下载/了解客户端 | `/desktop` | Dashboard 工具栏「客户端」、footer、help、产品地图 |
| 开分享 | `/dashboard/:id/share` | 工作区内「分享」、转化条 |
| 分享数据（汇总） | `/dashboard/analytics` | 顶栏分享组「访问」 |
| 分享数据（单库） | `/dashboard/:id/share`（页内 insights / 活动） | 汇总下钻、顶栏分享组「访问」、旧 `/analyze` 与 `/share/analytics` |
| API 密钥管理 | `/dashboard/:id/share`（API 区块；一种分享方法） | 分享弹窗内同一区块 |
| 上手 / 产品地图 | **弹窗**（Dashboard 入口）或 `/help` 长文 | 顶栏「上手」、空状态、账户→帮助 |
| 工作台 | `/dashboard` | 品牌标题、footer |
| 快速跳转 | **Cmd/Ctrl+K** 命令面板 | 登录后 App shell；条目仅链到本节 Canonical；会话 → `?session=`；文档 → `?source=`（打开后剥离）。Dashboard 工具栏「全局搜索」弹窗走同一 `/search` 索引（会话/工作区/资料分组） |

**禁止**: 在设置账单再实现第二套充值 checkout（可保留余额展示 + 链到 `#topup`）。  
**禁止**: 升级弹窗内完成支付（仅营销说明 + 跳转 canonical）。

---

## 5. Shell rules

| Shell | 何时 | 内容 |
|-------|------|------|
| **Marketing chrome** | pricing / desktop / legal | 定价 · 客户端 · 法律 · 语言 · 进入应用 |
| **App top bar** | dashboard / workspace / settings（产品内） | 品牌 · **分享组**（T0；含 访问/API/升级）· 通知 · 账户；**不设**工作台/设置 wayfinding（品牌即回工作台，设置在账户菜单），**不设**客户端/升级胶囊 |
| **Dashboard main** | `/dashboard*` | 全宽内容；筛选 tab（全部/我的/收藏）；横切「分享访问」与「客户端」入口在工具栏；**无**百科主侧栏 |
| **Workspace chrome** | `/dashboard/:id*` | 标题 · 新建 · **分享**（T0 单胶囊单行为，直开分享弹窗；访问=弹窗完整页入口，API 作为分享方法区块合入弹窗/分享中心）· 通知 · 账户 |
| **Settings** | `/settings` | 左侧 tabs（≤5）+ 面板 |
| **深层工具页** | `/dashboard/analytics`、`/dashboard/:id/share`、`/settings/usage`、`/help/*`（公开页 `/help/api-access*` 除外，用公开轻 chrome） | **统一 App top bar**（同上行 App top bar，经 `AppTopBar` 组件）；页内保留对象级返回链（如 share → 所属工作区）作 breadcrumb，**禁止裸返回链作为唯一出口** |
| **Onboarding map** | 按需 | 弹窗；入口是次要控件（上手），不占 240px 业务侧栏 |
| **Desktop shell** | 客户端 | 打开 → 云登录门（无 cloud_session 时）→ 工作台；抽屉「客户端设置」= 账户（云账户/余额）· 模型来源 · 数据 · 关于；栈管道诊断折叠收起，顶栏不设「已激活」胶囊。顶栏「账户」菜单 = **云身份**（名字/邮箱取 cloud session，不显示本机 B2C 内部账户，不显示订阅徽章）；退出登录 = `cloudLogout` + 回云登录门（不走 web `/login`，本机数据面会话不面向用户）；订阅/管理台探测仅 web |

**Rail pattern（Grok 式）**：多分区的设置/功能 surface 统一「左导航 + 右内容」——设置页、设置快开弹窗、分享弹窗、分享中心共用 `components/ui/nav-rail.tsx`（`NavRail`）+ AppModal `size="xl" bodyVariant="rail"`；单用途页（usage/analytics/help 长文）不强制。分享中心深链 `/share#api` 选中对应分区。

**Wayfinding**

- 顶栏品牌回工作台；工作区内应能感知「在某个 Workspace」。  
- 设置用 tab active；营销用 nav active。  
- 深度页优先对象内 sub-nav，不把设置项塞进业务 tab。

**顶级全局动作建议 ≤5 可见**: 分享 · 通知 · 账户（+ 可选「上手」弱样式）。客户端/升级不占顶栏——分别在 Dashboard 工具栏与分享组菜单。

---

## 6. Taxonomy（对外词）

| 用 | 不用（用户可见） |
|----|------------------|
| 工作区 | notebook 主叙事、org |
| 可分享名额 / 会员档位 | 「token 套餐」作主商品（已废除） |
| 充值 / 余额 | 仅写「模型调用余额」却链到设置且无法付 |
| 自定义 Provider / 自备 Key | 裸 BYOK 无解释（可括号补充） |
| 官方模型（走余额） | 「平台 key」「代购 key」等内部词 |
| 分享访问 / 独立访客 | 代理、墙钟、RAG（中英不一致处改掉） |
| 所有者付费 | 裸 Owner-pays 无解释 |
| 上手 / 产品说明 | 把帮助标成「主导航」 |

完整文案债见 `docs/copy-catalog/00-diagnosis.md`。

---

## 7. Anti-patterns（本产品禁止）

1. **Help 伪装 Primary nav** — 百科主题常驻业务侧栏。  
2. **第三完成页** — 同任务第二套 checkout/支付 UI。  
3. **孤立页** — 有 route 无发现入口且不在 sitemap。  
4. **设置混进业务 tab** — 如把「账单」放进工作区主 tab。  
5. **无当前位置** — 弹窗连环无「返回/完整页」出口（AppModal fullPage 已是最低要求）。  
6. **按表建导航** — 按 DB 表或内部服务名生成用户菜单。  
7. **客户端买断主推** — 与 ADR-0010「客户端免费」冲突的主 CTA。  
8. **改导航不改本文** — Agent/人直接画入口。

---

## 8. 产品地图（onboarding）正确形态

| 项 | 规定 |
|----|------|
| 是什么 | 模块关系 + 上手步骤 + 链到 canonical |
| 不是什么 | App 信息架构的 primary sidebar |
| UI | `ProductGuideModal`；Dashboard **上手**入口 + 空状态；完整说明可沉 `/help` |
| 主题 | 总览 · 两种 LLM · 工作区 · 分享 · 客户端 · 会员与充值 · 设置 · 相关入口 |

---

## 9. 变更流程

1. 改 §3–§5 与 §4 Canonical。  
2. 同步 `PRODUCT_IA_AUDIT.md` 若关闭审计项。  
3. 再改 `frontend_next` 入口。  
4. 验证：每个新按钮的 `href` 落在 §4 表。

**维护人**: 改导航的作者（solo 主干）。  
**下一修订触发**: 统一 App shell 侧栏（仅业务：工作台/设置）或合并单库分析路由时。
