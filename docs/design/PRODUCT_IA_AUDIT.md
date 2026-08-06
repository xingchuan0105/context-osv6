# Product IA 审计 — 2026-08-06（Path A）

对照 `PRODUCT_IA.md` v1。状态：`open` / `done` / `wontfix`。

---

## 1. 入口矩阵（As-Is → 目标）

| 用户动作 | 当前入口 | Canonical（目标） | 状态 |
|----------|----------|-------------------|------|
| 升级会员 | 顶栏升级弹窗、pricing 卡、paywall、分享转化 | `/pricing` 档位区 | 弹窗已改为说明→详情；**done** 方向 |
| 充值 | pricing `#topup` 本页套餐、升级弹窗 CTA、分享转化、曾链设置账单 | `/pricing#topup` | pricing 本页充值 **done**；设置账单可保留展示 |
| BYOK | settings providers、pricing 次链、产品地图 | `/settings?tab=providers` | ok |
| 客户端 | 顶栏、workspace 顶栏、footer、marketing、help、地图 | `/desktop` | ok |
| 上手/地图 | **曾：Dashboard 13.5rem 主侧栏** + 弹窗 | 弱入口 + 弹窗，非主栏 | **P0 本轮降级** |
| 工作台 | 品牌、footer | `/dashboard` | ok |
| 分享汇总 | dashboard tab 数据分析 | `/dashboard/analytics` | ok；tab 是否独立后续 |
| 单库分享 | workspace share 路由 | 对象内 | ok |
| 帮助长文 | 账户菜单 /help | `/help` | ok |

---

## 2. 发现的问题

### P0

| ID | 问题 | 证据 | 处置 |
|----|------|------|------|
| P0-1 | 产品地图做成 primary 侧栏，抢工作区主路径 | `ProductGuideSidebar` + `dashboard-body` | **本轮**: 移除侧栏，改为顶栏「上手」+ 空状态 |
| P0-2 | 无登录后 PRODUCT_IA 真相源 | 仅有多站点 IA + 文案诊断 | **本轮**: `PRODUCT_IA.md` |
| P0-3 | 充值曾只跳设置 | 历史 pricing CTA | 已本页 topup；审计关闭 |

### P1

| ID | 问题 | 建议 | 状态 |
|----|------|------|------|
| P1-1 | 「数据分析」夹在工作区筛选 tab | 工具栏独立「分享访问」入口 | **done** |
| P1-2 | 单库 analyze vs share/analytics 双路径 | 均 redirect → share；删独立 analyze surface | **done** |
| P1-3 | Settings 默认 tab=billing | 默认 profile；顺序账户→模型→账单 | **done** |
| P1-4 | desktop/buy、licenses 与「客户端免费」 | help 去掉购买主链；buy 页顶部导向免费下载 | **done** |
| P1-5 | App shell 不跨 Workspace 稳定 | 轻量 `AppPrimaryNav` 工作台\|设置（非百科侧栏） | **done** (light) |
| P1-6 | 设置账单仍可能双份 topup UI | 账单只展示余额 + 链 `#topup` 或共享组件 | **done**（2026-08-06：去套餐结账，链 `/pricing#topup`） |

### P2

| ID | 问题 | 状态 |
|----|------|------|
| P2-1 | 术语 Owner-pays / RAG 中英（copy-catalog） | **done**（高触达 share/pricing/help/degrade） |
| P2-2 | 升级弹窗与 pricing 文案重复 | 可接受；保持弹窗短、pricing 全 |
| P2-3 | 无 Cmd+K 命令面板 | **done**（`CommandPaletteHost`，Canonical 路由） |

---

## 3. Shell 对照

| Shell | 合规？ | 备注 |
|-------|--------|------|
| Marketing chrome | 是 | pricing/desktop/legal |
| App top bar | 基本是 | 本轮加「上手」弱入口 |
| Dashboard 百科侧栏 | **否** → 本轮删除 | 违反 anti-pattern §7.1 |
| Workspace chrome | 是 | 客户端+升级保留 |
| Settings tabs | 是 | 5 tab |

---

## 4. 路由清单抽查（frontend_next/app）

| 区域 | 路径模式 | IA 归类 |
|------|----------|---------|
| App | dashboard、settings、help、upgrade | App / 旁路 |
| Marketing | pricing、desktop、legal | Marketing |
| Auth | login、register、reset | Auth |
| Desktop runtime | activate、setup | Client |
| Account | licenses | 旁路 |
| Shared | shared/kb | 访客 |
| Admin | admin/* | 超出 v1 用户 IA |

未发现完全无链的核心 monetization 页；**客户端**与 **pricing** 发现已修复（相对 2026-07 多站点审计）。

---

## 5. 本轮关闭项

- [x] PRODUCT_IA.md v1  
- [x] 本审计文件  
- [x] P0-1 产品地图降级  
- [x] AGENTS.md 导航三条  
- [x] P1-6 设置账单去掉第二套充值结账 → `/pricing#topup`  
- [x] P1-1/P1-2 分享访问两层：汇总 `/dashboard/analytics` · 单库 `/share`  
- [x] P1-3 Settings 默认 profile  
- [x] P1-4 客户端购买降权  
- [x] P1-5 light AppPrimaryNav  
- [x] P2-1 高触达术语（Owner-pays / RAG / 降级原因）  
- [x] 邀请页 / 同意框 / 法律页脚 / 客户端状态徽章 i18n 收口  
- [x] API Access 面 + providers 行标签 i18n  
- [x] admin 导航/状态术语（检索健康、后台任务、限速策略、护栏文案）  
- [x] P2-3 Cmd/Ctrl+K 命令面板  
- [x] 命令面板：工作区搜索 + 最近打开  
- [x] 命令面板：全局搜索会话 / 文档（`GET /api/v1/search`）+ 工作区 `?session=` 深链选中  

## 6. 建议下一迭代（非本轮）

1. 后端通知 / 邮件模板化（copy-catalog P7）。  
2. ~~命令面板文档 `?source=` 深链~~ **done**（命中 → viewer；打开后剥离 one-shot query）。  
3. ~~打开会话后同步/清理 URL `?session=`~~ **done**（选会话 / 新建 / 流式建会话 / 删活动会话 → `router.replace`；深链 `preferredSessionId` 选中；不默认改写无 query 的落地页）。  
4. 命令面板：跨页动作（非跳转类，如「新建工作区」）若产品需要再开。

---

*审计作者: Path A 落地会话 · 2026-08-06 · 续 2026-08-07*
