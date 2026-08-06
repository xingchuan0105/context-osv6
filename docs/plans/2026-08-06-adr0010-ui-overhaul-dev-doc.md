# ADR-0010 前端叙事翻转与交互重构 — 开发文档

- **日期**:2026-08-06
- **依据**:ADR-0010(商品定义翻转,后端已验收至非 Publish 部分)+ 本地测试问题记录(`docs/plans/2026-08-06-adr0010-local-test-ui-issues.md`,#1–#20)+ 参照截图(Perplexity 设置体系 a–i、X 个人主页 j、分享面板 k)
- **范围**:`frontend_next` 为主;少量后端触点(#8 资料 schema、#11 通知发射、#13 邀请邮件、#14 限次存储与计费口径)
- **非目标**:Publish 上云(PR12–14)、桌面端结构改动

---

## 1. 设计基准

### 1.1 显示环境与画布

- 基准机:2560×1600 @175% 缩放,Win11 → 逻辑桌面 **1463×914 CSS px**,浏览器视口 ≈ **1460×745 CSS px**。
- 纵向空间紧张:所有容器密度按此画布校核,避免大留白标题区。

### 1.2 参照体系(实测 CSS px,物理÷1.75)

| 组件 | 参照实测 | 采用形态 |
|------|---------|---------|
| 设置容器 | 弹窗 ~1009×614(视口 69%×82%),左导航 ~226 + 搜索框 + 内容 ~780 | **设置改居中弹窗** |
| 头像菜单 | 宽 ~220,行高 ~36,顶部用户卡(头像+名+档位标+邮箱)~64 | 菜单加宽 + 用户卡头 |
| flyout 子菜单 | 宽 ~183,行高 ~36,长列表滚动 | 外观/语言改 flyout |
| 通知浮层 | ~400×300,铃铛触发,条目 ~63(图标+标题+状态+相对时间) | 新增铃铛浮层 |
| 分享面板 | ~388×358:邮箱邀请 → 人员列表 → 访问权限 radio → 链接+复制 | 分享弹层重排 |
| 资料名片(X) | banner(约 3:1)+ 叠放圆形头像 + 显示名 + 简介 + 链接 + 操作钮 | 分享页 Owner 名片 |
| 用量展示 | 「分析」页:UTC 说明 + 7d/30d/90d + 统计卡 + 分组下拉 + 下载 | 消费分析心智,无配额墙样式 |

### 1.3 尺寸 token(新组件一律取用)

- 设置弹窗:`width: min(1000px, 92vw)`;`height: min(86vh, 720px)`;左导航 224px;内部滚动。
- 头像菜单:宽 224–240px;用户卡头 ~64px;行高 36px;`max-height: 70vh`。
- flyout:宽 184–200px;行高 36px;`max-height: 60vh`。
- 通知浮层:宽 384–400px;`max-height: min(60vh, 480px)`;条目 60–64px。
- 居中内容弹窗(#17):宽 min(1000px, 92vw),内部滚动,✕/Esc/点遮罩关闭。
- 密度:控件字 13–14px(沿用 `--font-size-control: 0.8125rem`),区块间距 16–24px。现有 `app-auth-card` 28rem、`app-page-center` 72rem 不动。

### 1.4 文案基调(待 D5 定稿后全量替换)

- 「云端 BYOK(自带 API Key)」→「自己的模型 Key」一类通俗说法。
- 「模型代购钱包」→「余额 / 调用余额」(候选「抵用金」)。
- 「账单与计划」→「会员状态」;「订阅与用量」叙事整体退役。

---

## 2. 目标 IA 总览

1. **设置 = 居中弹窗**(带搜索),不再是 `settings?tab=*` 独立页;页签收敛为:会员状态、个人资料、偏好(外观/语言 dropdown 化)、安全(精简)、(通知移出设置)。
2. **通知 = 头像旁铃铛浮层**,账户级;不再有 settings 通知页签。
3. **头像菜单**:用户卡头 + 所有设置 / 升级计划 / 外观 / 语言 / 帮助 / **退出登录**(自安全页迁入)。
4. **顶栏胶囊组**:升级(暖橙强调)/ 新建工作区 / 分享 / API / 账户——同高同圆角,分享、API 去图标纯文字,账户底色并轨。
5. **分享**:右上按钮 → 浮层(参照 k 结构)+ Owner 成本提示与名额占用;已分享 Workspace 才有「数据分析」入口。
6. **对话区**:中栏自有滚动条;源/笔记/citation 统一居中弹窗;顶部模式 header 删除。

---

## 3. 工作波次

### W1 叙事翻转(收钱前必做;纯前端为主)

| 项 | 内容 | 关键位置 | 验收 |
|----|------|---------|------|
| #1 | /pricing 重写:客户端免费、私有使用免费;分享名额 Free 3 / Plus 10 / Pro 100;月付+年付(≈10 个月价);钱包充值为辅商品 | `app/(marketing)/pricing/pricing-page-client.tsx`、`components/billing/PricingCards.tsx` | 页面无 token 套餐叙事;年付 SKU(plus_annual/pro_annual)可下单 |
| #2 | 移除 5h/7d 作为主权益的展示;用量改「消费明细/分析」心智 | `components/billing/UsageMeter.tsx`、`messages/paywall.ts:3-19`、`messages/usage.ts`、`lib/billing/planLimits.ts` | 设置页无滚动窗口配额墙样式;付费墙不再以 5h/7d 为升级理由 |
| #6 #7 | 「账单与计划」→「会员状态」;页面 IA:档位 / 分享名额(x/N)/ 到期时间 / 余额 / 自己的 Key / 消费明细 | `messages/settings.ts:13,61,65`、`dashboard.ts:133`、`usage.ts:10`、`settings-billing-panel.tsx` | 设置页首屏呈现会员状态卡 + 分享名额用量条 |
| #3 #5 | BYOK、钱包通俗命名全量替换(含 payer_funds_required 提示) | `messages/settings.ts:129-130,165-170,461-497` | 全文无「BYOK」「代购」字样(代码标识符除外) |

### W2 分享体系(核心价值面)

| 项 | 内容 | 关键位置 | 验收 |
|----|------|---------|------|
| #14 | 访客模式两档:匿名链接 / 定向邀请;匿名按分享配提问上限(默认 10,Owner 可改,+成本提示);定向邀请按人配上限(可 ∞)。**先修订 ADR §4** | share settings schema、`share/src/types.rs`、`transport-http/src/middleware.rs`(现全局 env 限次)、`parts/share-control-bar.tsx` | 按分享/按人限次生效并落库;超限访客得到明确文案;匿名键 edge_ip+Turnstile,邀请键 user_id |
| #14b | 定向邀请计费归属:成员在他人库内提问的 payer 口径(待 D2 拍板后实施) | `app-chat` payer 解析链 | 与拍板口径一致并有测试 |
| #13 | 邀请发邮件:复用 SMTP(密码重置通道),含邀请链接;未注册邮箱附注册引导(可叠邀请码) | `app-bootstrap/src/services/password_reset.rs`(通道)、`pg_share_store/invite.rs`(发射点)、`invite-surface.tsx` | 邀请后收件箱可收链接邮件;手动复制保留 |
| #15 | 数据分析独立页:挂载孤儿 `workspace-analyze-surface.tsx`(`/analyze` 路由当前只 redirect);顶栏加入口,仅 `share_enabled=true` 可见;补「活跃=提问次数/提问访客」口径 | `app/(app)/dashboard/[workspace_id]/analyze/page.tsx`、`workspace-top-bar.tsx`、`/share/analytics`、`/share/access-logs` | 未分享 workspace 无入口;已分享可进独立页看访问/活跃/访问者 |
| #12 | api-access 弹窗补文档入口(/help/api-access + /docs/api-access-for-agents.md),抽共享卡片段 | `workspace-api-access-modal.tsx`、`workspace-api-access-surface.tsx:238,375-408` | 弹窗内可达两份文档 |

分享面板结构(k 参照)在 W2 内一并重排:邮箱邀请 → 人员列表 → 访问权限 radio → 链接+复制;radio 下方一行小字放 Owner 成本提示与名额(x/N)。

### W3 设置弹窗化与头像菜单(IA 大改)

| 项 | 内容 | 关键位置 | 验收 |
|----|------|---------|------|
| #9 | 外观 tab 取消,选项下拉化(主题预览卡 + 行右下拉 + toggle);并入「偏好」或进头像菜单 flyout | `settings-appearance-panel.tsx` | 无独立外观页;所有选项 dropdown/toggle |
| #10 | 安全页:修改密码先点按钮再展开;退出登录迁入头像菜单;删当前会话状态展示 | `settings-security-panel.tsx`、`account-menu.tsx:119,127` | 三条全满足 |
| IA | 设置页 → 居中弹窗(左导航 224 + 搜索框);头像菜单加宽 224–240 + 用户卡头 + 铃铛 + 退出登录 | `settings-surface.tsx`、`settings-quick-modal.tsx`、`account-menu.tsx`、尺寸 token §1.3 | 旧 `settings?tab=*` 链接重定向或映射到弹窗锚点;**更新引用这些链接的测试**(`tests/dashboard/dashboard-surface.test.tsx:223,226`、`tests/settings/settings-surface.test.tsx:291-300`、`settings-quick-modal.tsx:78`、`help/page.tsx:46`) |
| #19 | 顶栏胶囊组对齐:同高同圆角同字重;分享/API 去图标;底色暖橙/灰一套;账户并轨 | `workspace-top-bar.tsx:135-205`、`workspace-shell.module.css:131-254`、`plan-entry`、`account-menu` 触发器 | 五枚胶囊视觉一致;D3 定稿后落色 |

### W4 通知体系(账户级 + 官方广播)

| 项 | 内容 | 关键位置 | 验收 |
|----|------|---------|------|
| #11 | 铃铛浮层(384–400px)接入现有通知列表;发射点:修改密码、分享成功、余额不足(阈值);admin 广播 API + 最小 UI;**不做 Workspace 级** | `settings-notifications-panel.tsx`(列表复用)、`billing_sql/core_webhooks/maintenance.rs`(既有发射点参照)、`pg_admin_store` | 三类事件触发可见;广播可达全部用户;无 workspace 级开关 |

### W5 对话区打磨与统一弹窗

| 项 | 内容 | 关键位置 | 验收 |
|----|------|---------|------|
| #16① | 中栏滚动:修父链高度断裂(`workspace-surface.tsx:452` → `WorkspaceChatPane` 容器链),补 Chromium `::-webkit-scrollbar`(现仅 `.historyList` 有) | `workspace-chat.module.css:13-22`、`workspace-shell.module.css:617-621` | 中栏自滚动且滚动条可见;页面级不再接管 |
| #16② | 回答内 `####` 字面泄漏:修 markdown 渲染链 h4+ | assistant 内容渲染组件 | h1–h4 均正确渲染 |
| #16③ | 「回到底部」浮钮压字:偏移/不透明背景/避开文本流 | `workspace-chat.module.css` `.scrollToBottomButton` | 不再与正文重叠 |
| #17 | 统一居中大弹窗:内容源条目、新建笔记、笔记条目、citation 全部弹窗化;citation 弃用锚点定位式 | `chat-message-list.tsx:239-247`、`citation-renderer.tsx:442-452`、`workspace-sources-pane.tsx`、`workspace-notes-pane.tsx`;基建复用 `modalEnter` | 三处全部弹窗;Esc/遮罩可关;新建保存后列表即时更新 |
| #18 | 删除对话栏顶部模式 header(标题+rag chip) | `workspace-chat-pane.tsx:225-230` | 顶部无重复模式指示;composer 开关保留 |

### W6 个人资料与传播

| 项 | 内容 | 关键位置 | 验收 |
|----|------|---------|------|
| #8 | 资料扩 schema:简介/banner/头像/联系方式;设置页表单;分享页 Owner 名片(X 结构);头像走对象存储上传链 | `settings-profile-panel.tsx`(现仅 fullName)、users 表迁移、`/shared/kb/[token]` 页面 | 名片在分享页展示;字段可编辑可空 |

### W7 杂项

| 项 | 内容 | 关键位置 | 验收 |
|----|------|---------|------|
| #20 | 默认名改「新建工作区N」/「New Workspace N」;先复现截图中裸「1」的来源(与现行生成器「工作区N」不符,疑旧构建/桌面本地路径) | `lib/dashboard/default-title.ts`;两入口 `dashboard-surface`、`use-workspace-data` | ✅ 默认名已改;裸「1」排查结论:产品创建路径仅上述 helper,不会产出纯数字标题(疑手动改名/旧数据/非 web 客户端) |

---

## 4. 待拍板决策(开工前过一遍)

| # | 决策 | 选项 | 倾向 |
|---|------|------|------|
| D1(#4) | embedding/rerank 是否开 BYOK | (a) 补 resolve+UI;(b) 文案诚实「自己的 Key 只管对话,向量走平台」 | **(b)**,后端无 resolve 路径,成本低 |
| D2(#14b) | 定向邀请成员提问谁付费 | Owner-pays(与分享一致)/ 成员自负(现状) | 建议 Owner-pays,否则「按人限次」无意义 |
| D3(#19) | 胶囊底色定稿 | 升级=暖橙强调,其余=灰底中性 | 按此,等确认 |
| D4(#20) | 命名计数器口径 | 前端 localStorage(现状)/ 服务端计数 | 前端先改 base 解主诉,服务端口径可缓 |
| D5(#3#5) | 通俗命名定稿 | 「自己的模型 Key」「余额/抵用金」 | 等选定后全量替换 |

---

## 5. 全局验收与回归

- `pnpm test` + typecheck 全绿;受影响 POM/单测更新(见 W3 IA 行列出的测试文件;分享/邀请流程 e2e:`e2e/pom/share-page.ts`)。
- i18n 纪律:所有新文案进 `lib/i18n/messages/*` 目录,中英双键,禁止组件内联三元(此前 settings-billing-panel 犯过,已修)。
- 文案复查:全前端 grep 无「BYOK」「代购」「订阅与用量」「5h/7d」主权益叙事残留(代码标识符除外)。
- 文档:ADR §4 修订(#14 两档+限次);本文件状态列回写。
- 结构改动后跑 `code-review-graph update`。
- 后端触点(#8/#11/#13/#14)遵循 prompts-in-md、T1–T8;邮件模板 prose 属产品文案,不进 prompt 目录但也不硬编码多语言——中英走模板选择。

## 附

- 原始问题记录(含逐条截图锚点与代码疑点):`docs/plans/2026-08-06-adr0010-local-test-ui-issues.md`
- ADR:`docs/adr/0010-share-service-business-model.md`
