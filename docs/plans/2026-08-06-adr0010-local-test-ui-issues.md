# ADR-0010 本地测试问题记录( UI / 文案 / 页面逻辑)

> **已整理为开发文档:`docs/plans/2026-08-06-adr0010-ui-overhaul-dev-doc.md`(执行以那份为准;本文件保留为原始记录与截图锚点)。**
> 来源:本地测试 (localhost:3000) 逐批上报。状态列:⬜ 待改 / 🔧 改中 / ✅ 已改。
> 关联:docs/adr/0010-share-service-business-model.md(商品定义翻转后,前端叙事未跟上)。

## 批次 1(2026-08-06)

### #1 /pricing 页未改 ✅

- 现象:定价页仍是旧叙事(token 套餐/rolling 限额为主权益)。
- 位置:`frontend_next/app/(marketing)/pricing/pricing-page-client.tsx`、`frontend_next/components/billing/PricingCards.tsx`。
- 期望:按 ADR-0010 §2/§3 重写——客户端免费、私有使用免费;主商品 = 可分享 Workspace 名额 Free 3 / Plus 10 / Pro 100;月付+年付(年付约 10 个月价);钱包充值(代购模型调用)作为辅商品;年付 SKU 已存在于后端(plus_annual/pro_annual)。

### #2 /settings?tab=billing 仍展示 5h/7d 限额 ✅

- 现象:设置页账单 tab 仍把 5h/7d 滚动窗口当作套餐额度展示。
- 位置:`frontend_next/components/billing/UsageMeter.tsx`(token 计量条)、`frontend_next/lib/i18n/messages/paywall.ts:3-19`(5h/7d 付费墙文案)、`frontend_next/lib/i18n/messages/usage.ts:3+`、`frontend_next/lib/billing/planLimits.ts`。
- 期望:ADR-0010 §1.1 T4 后,rolling 限额已降级为「无 BYOK 且无余额时的保护性 hard stop」,不再是主权益;页面不应再以「用量额度」心智呈现。用量数据可保留为「消费明细/参考」性质。

### #3 「云端 BYOK(自带 API Key)」说法太专业 ✅

- 位置:`frontend_next/lib/i18n/messages/settings.ts:129-130`(标题)、`:165-170`(payer_funds_required 提示文案)。
- 期望:大众用户能懂的说法,如「自己的模型 Key」「使用自己的 API Key」等;提示文案同步改。

### #4 BYOK 配置只有 LLM,没有 embedding / rerank ✅

- 现象:设置页云端 Key 配置只有 LLM provider 一项。
- 位置:`frontend_next/components/settings/settings-billing-panel.tsx`(purpose 硬编码 "llm")。
- 背景(代码现实):后端 `ProviderSecretPurpose::Embedding` 仅存在于枚举,**没有任何 resolve 路径**;embedding/rerank 目前恒走平台 key 并计入钱包扣费(db8bdd27 起 embedding 按 usage_kind 拆分计费)。
- 期望:二选一——(a) 补 embedding/rerank 的 Key 配置(UI + 后端 resolve);(b) 产品上明确「平台托管模型调用含向量」,UI 文案说清哪些走自己的 Key、哪些走平台。需要产品决定。

### #5 「模型代购钱包」说法不对 ✅

- 位置:`frontend_next/lib/i18n/messages/settings.ts:461`(标题)、`:465`(说明)、`:493,497`(加载/标签)。
- 期望:通俗易懂,如「余额」「调用余额」「充值余额」;说明文案同步(目前「平台代购模型调用」也偏技术)。

### #6 「账单与计划」概念已变 ✅

- 位置:`frontend_next/lib/i18n/messages/settings.ts:61`。
- 期望:改为「会员状态」心智——当前档位、分享名额用量(x/3、x/10、x/100)、到期时间、续费/升级,而非「账单与计划」。

### #7 整页概念不应是「订阅与用量」 ✅

- 位置:`frontend_next/lib/i18n/messages/settings.ts:13`(tab 标题)、`:65`(描述文案);同样叙事还出现在 `dashboard.ts:133`、`usage.ts:10`。
- 期望:产品逻辑已从「订阅 token 套餐 + 用量墙」翻转为「会员档位 + 分享名额 + 余额 + 自己的 Key」,页面信息架构应整体对应:会员状态 / 分享名额 / 充值余额 / 自己的模型 Key /(参考性)消费明细。需要一次页面 IA 调整,不只是改词。

## 批次 2(2026-08-06)

### #8 profile tab:面向分享传播,资料字段太薄 ✅

- 现象:个人资料只有 `fullName` 一个字段(`settings-profile-panel.tsx:17-60`)。
- 期望:分享给他人时,Owner 资料应支撑社媒传播——**简介、banner、头像、联系方式**等;分享页(/shared/kb/[token])可展示 Owner 名片。
- 涉及:前端表单 + 后端用户资料 schema 扩展(当前仅 full_name)+ 分享页名片展示。属产品功能,不只是文案。

### #9 appearance tab:内容太薄,不配单独成页 ✅

- 现象:外观选项撑不满一页(`settings-appearance-panel.tsx`)。
- 期望:选项改成**下拉式**紧凑布局;考虑并入其他页,或**不单独成页、只做弹窗**(已有 `settings-quick-modal.tsx` 可承载)。

### #10 security tab:交互层级要收 ✅

- 期望:
  1. 先点「修改密码」按钮,再展开密码输入框(当前直接展示);
  2. **退出登录**折叠进主页头像弹窗(`account-menu.tsx`),不单独做;
  3. 去掉「当前会话状态」展示。
- 位置:`settings-security-panel.tsx`、`account-menu.tsx:119,127`(现链接到 settings?tab=security/notifications)。

### #11 notifications tab:通知体系重做(账户级 + 分享 + 官方广播) ✅

- 期望:
  - **不做 Workspace 级**通知;
  - 做**账户级 + 分享通知**:修改密码、分享成功、余额不足等事件触发;
  - **开发者→用户**的官方通知通道;
  - 「这个接口要打通」——事件产生 → 通知落库 → 前端展示的链路。
- 现状:通知列表 + 偏好设置已有(`settings-notifications-panel.tsx`,`listNotifications`/`markNotificationRead`);后端已有零星发通知点(如订阅过期,`billing_sql/core_webhooks/maintenance.rs`)。缺:上述业务事件的发射点、余额不足阈值触发、admin 广播 API 与 UI(`pg_admin_store` 有相关基础)。

### #12 api-access 弹窗缺帮助文档入口 ✅

- 现象:完整页 `workspace-api-access-surface.tsx:375-408` 有帮助入口卡(「先读人类说明」→ `/help/api-access`;「再读稳定 agent 文档」→ `/docs/api-access-for-agents.md`),但顶栏弹窗 `workspace-api-access-modal.tsx`(自 `workspace-top-bar.tsx:217` 打开)里**没有任何文档链接**,只有一句纯文本「完整说明与 agent 文档可在完整页面查看」(surface:238)。
- 期望:弹窗内同样给出这两个文档入口(链接或内嵌摘要),不要让用户必须跳到完整页。

---

## 参考样式(2026-08-06 截图 a–j:Perplexity 设置体系 + X 个人主页)

**总体 IA(e/f/g/h):设置是居中弹窗,不是独立页面**——左导航顶部带「搜索设置」搜索框;条目按组排列(帐户组 / Computer 组 / 其他)。对应 #7 #9。

- **#9 外观/语言(a/b/c/f)**:侧边栏头像菜单里「外观」「语言」是**行内 flyout 子菜单**(行尾箭头展开,当前项 ✓);设置弹窗内偏好页用**预览卡片选主题**(浅色/深色/跟随系统)+ **行右下拉**(语言 默认 ▼、首选响应语言 ▼、快捷键提示 ▼)+ **开关 toggle**。选项全部下拉化,无独立页面。
- **#10 点击再编辑(e/g)**:帐户页每行 = 左标签右操作按钮(更改头像/更改全名/更改用户名;出生日期/性别 = 管理 >);订阅区右侧 管理/转移订阅/升级计划;安全区「双重身份验证 + 设置」——均先点按钮再展开表单。安全页不直铺输入框。
- **#10 退出登录(a)**:在侧边栏**头像弹窗**菜单末位(添加账户/所有设置/升级计划/安装应用/外观/语言/帮助/退出登录),无独立安全页入口。头像弹窗底部还有通知铃铛(i)。
- **#11 通知(i)**:铃铛图标在头像旁,点击弹**通知浮层卡片**(图标+标题+状态如「Action needed/深度研究·Completed」+相对日期 + 右上 … 菜单),账户级,非设置页签。
- **#8 资料页(j,X 主页)**:banner 大图 + 叠放圆形头像 + 显示名+认证标 + @handle + 多行简介 + 链接(blog:…)+ 操作按钮(关注/私信/…)。分享页 Owner 名片参考此结构;字段与 #8 的 简介/banner/头像/联系方式 对应。
- **#5 命名参考(h)**:Perplexity 用「使用情况和抵用金」——「抵用金」可作余额命名的通俗化候选之一。
- **#2/#7 用量展示(h)**:分析页 = UTC 说明 + 7d/30d/90d 切换 + 统计卡 + 分组下拉(Model ▼)+ 下载;用量以「消费分析」心智呈现,无配额墙样式。

## 组件尺寸分析(2026-08-06,基准机:2560×1600 @175% 缩放,Win11)

### 显示环境换算

- 逻辑桌面:2560/1.75 × 1600/1.75 = **1463×914 CSS px**。
- 截图浏览器窗口 2559×1306 物理 → 视口 ≈ **1462×746 CSS px**(全宽、非全高)。**设计画布应按 ~1460×745 CSS px**——175% 缩放后可用高度比 1440×900 笔记本还矮,纵向密度要克制。
- 截图物理像素 ÷ 1.75 = CSS px。

### 参照实测(截图)vs 当前实现

| 组件 | Perplexity 实测(CSS px) | 当前 frontend_next | 差距 |
|------|------------------------|--------------------|------|
| 设置容器 | **弹窗 ~1009×614**(视口的 69%×82%),左导航 ~226 + 内容 ~780 | **独立页面** max-width 1152(`app-page-center`),无搜索、页签式 | 形态不同:页面 → 弹窗 |
| 头像菜单 | **宽 ~220**,行高 ~36,含用户卡头(头像+名+Pro 标+邮箱)≈64;总高 ~510(68% 视口高) | `min-width: 10.5rem`=**168**,无用户卡头,行高 ~33(13px 字 + 8/12 padding) | 窄 24%,缺卡头 |
| flyout 子菜单 | 宽 ~183,行高 ~36,长列表滚动(语言) | 无 flyout(外观/语言是独立 tab) | 缺失 |
| 通知浮层 | **~400×300**,锚定左下角头像旁铃铛,条目 ~63 高(图标+标题+状态+相对日期) | 无浮层(仅 settings tab) | 缺失 |
| 外观主题卡 | 3 张预览卡并排,各 ~86×54,✓ 标当前 | 独立页选项 | 改卡片+下拉 |
| 资料名片(j,X) | banner ~1090×~300 比例 3:1 上下,圆形头像叠放偏移,操作按钮右排 | 仅 fullName 一行 | #8 结构参考 |

### 建议尺寸 token(适配 1463×914 逻辑桌面)

- 设置弹窗:`width: min(1000px, 92vw)`,`height: min(86vh, 720px)`;左导航 224px;内容区 ~740px;内部滚动。
- 头像菜单:宽 224–240px;用户卡头 ~64px(头像 32 + 两行文字);行高 36px;`max-height: 70vh` 滚动。
- flyout 子菜单:宽 184–200px;行高 36px;`max-height: 60vh`。
- 通知浮层:宽 384–400px;`max-height: min(60vh, 480px)`;条目 60–64px。
- 密度:控件字 13–14px(现 `--font-size-control: 0.8125rem` 可沿用),区块间距 16–24px;纵向空间紧张,避免大留白标题区。
- 现行 `app-auth-card` 28rem(448px)、`app-page-center` 72rem(1152px)在该视口下比例正常,无需动。

### 分享面板(k,Perplexity「分享此会话」)

- **形态**:右上「分享」按钮弹出的**浮层卡片**,~388×358 CSS px(物理 678×627 ÷1.75)。
- **信息结构(自上而下)**:
  1. 标题「分享此会话」;
  2. **邮箱邀请输入框 + 添加按钮**(协作邀请在面板顶部);
  3. **有权限的人员**列表(头像+邮箱+角色徽标「所有者」);
  4. **常规访问权限** radio 组(图标+文字,当前项 ✓):「仅有权限的人员可查看」/「任何拥有链接的人都可查看」;
  5. 底部:**链接预览 + 复制链接主按钮**。
- **对应我们的组件**:`components/share/workspace-share-quick-modal.tsx`(弹层)+ `parts/share-control-bar.tsx`;radio 组 = ADR §4 访客模式(须注册 ↔ 仅有权限的人员;匿名 ↔ 任何拥有链接的人)。
- **我们比参照多出的(ADR-0010 必有)**:选中「任何拥有链接的人」时展示 **Owner 成本提示**(访客消耗计入你的余额/Key)+ 名额占用提示(x/3);可放在 radio 下方一行小字,不挡主操作。

### #13 邮箱邀请不落邮件,只有手动复制链接 ✅

- 现象(用户问:「邮箱邀请要给邀请邮箱发邮件、链接,实现了吗?」):**没有**。`invite_member` 只写 `workspace_members`(`invite_status='pending'`,按邮箱解析已注册用户 id,`pg_share_store/invite.rs:18-64`),**无任何邮件投递**;前端只能手动复制 `/invite/{workspace_id}/{member_id}` 链接(`invite-surface.tsx:29`)再自行发给对方。
- 现状可用地基:SMTP(lettre,163 邮箱,`.env.example:254-257`)已用于**密码重置**(`app-bootstrap/src/services/password_reset.rs`);分享域/产品层均未接。
- 期望:邀请时向被邀邮箱发送含邀请链接的邮件(复用 SMTP 通道);保留手动复制链接作为辅助。注意与 #11 通知体系共用同一邮件通道;未注册邮箱的邀请邮件应同时带注册引导(与 #6 邀请码可叠加)。

### #14 分享访客模式改为两档 + 按分享配置提问次数上限(产品规则变更) ✅

- **新规则(用户拍板,2026-08-06)**:
  1. Workspace 分享只分两种:**匿名**(持有链接即可)与**定向邀请**(仅被邀请的人);
  2. **匿名:限制提问次数,默认 10 次,Owner 可自选**;必须提示「访客消耗计入你的 token/余额」(因为花的都是 Owner 的);
  3. **定向邀请:也要限制提问次数**,按人配置,**最高可无限制**。
- **与 ADR §4 的关系**:现行 ADR 访客模式 = 匿名|须注册;新规则用「定向邀请」取代「须注册」档位,并新增按分享/按人的次数上限——**ADR §4 需要修订**。
- **代码现状差距**:
  - 次数上限目前只有**全局 env**(`SHARE_CHAT_DAILY_LIMIT` 默认 200/天/访客、`SHARE_CHAT_RATE_LIMIT_RPM` 30),无按分享配置、无 Owner UI;
  - 匿名限次身份键 = 边缘 IP(`anon:{edge_ip}`,可轮换绕过,配合 Turnstile 缓解);定向邀请限次可键 user_id/member_id(可靠);
  - 存储:需在 share settings 增「匿名提问上限」字段 + 邀请(member)侧增「个人提问上限」字段(含"无限制"选项);
  - **计费归属待核对**:share-token 路径已 Owner-pays;但**成员(被邀人)在工作区内提问目前疑为成员自负**(成员走普通 chat 路径,payer=自己)——若定向邀请也由 Owner 请客,需要把成员路径纳入 Owner-pays 或明确产品口径;
  - UI:分享面板(参照 k.png radio 结构)在「任何拥有链接的人」选项下暴露次数输入(默认 10)+ 成本提示;邀请成员行暴露每人的次数上限(含 ∞)。

### #15 分享 Workspace 要有独立「数据分析」按钮和页面 ✅

- **需求(用户拍板,2026-08-06)**:Workspace 要有一个**单独的按钮和数据分析页面**,看**访问、活跃、访问者**信息;**仅限已分享的 Workspace**(未分享不展示/不可点)。
- **代码现状**:
  - 完整分析页组件已存在但**未被任何路由挂载(孤儿代码)**:`components/share/workspace-analyze-surface.tsx`(自带 CSS module,调 `getShareAnalytics`/`getShareAccessLogs`);
  - 现行展示只是分享管理页内嵌的 `ShareInsightsPanel`(`workspace-share-surface.tsx:84`);
  - 后端数据面已有:`/share/analytics`(总访问量/独立访客/按天趋势)、`/share/access-logs`(访客 ID/时间/动作),i18n 还有「近 30 天活跃天数」。
- **要做的**:
  1. 挂载独立路由,把孤儿 surface 接上(注:`/dashboard/[id]/analyze/page.tsx` 路由存在但只是 `redirect → /share/{id}#insights`);
  2. Workspace 顶栏/操作区加**专用入口按钮**,仅 `share_enabled=true` 时可见(或置灰引导去开分享);
  3. 数据口径核对:「活跃」建议含提问次数/提问访客(目前 analytics 偏访问量);「访问者」列表对匿名访客只有 IP/指纹级信息,文案不要说满;
  4. 与 #7 的设置 IA、#14 的限次数据可呼应(分析页可展示限次消耗)。

### #16 对话区 UI 打磨(截图 2026-08-06_133951) ✅

- **① 中间栏没有自己的滚动条(用户主诉)**。症状:整页在窗口最右缘滚动,中间对话栏无滚动条。代码疑点:
  - `.transcript`(`workspace-chat.module.css:13-22`)本身写了 `overflow-y:auto; min-height:0`,看似正确 → 大概率是**父链高度断裂**(某层没传 `height:100%/min-height:0`,transcript 随内容撑高,页面级滚动接管),沿 `workspace-surface.tsx:452` → `WorkspaceChatPane` 的容器链查;
  - 且 `scrollbar-width: thin` **只对 Firefox 生效**,Chromium 需 `::-webkit-scrollbar` 规则——目前全项目只有 `.historyList`(`workspace-shell.module.css:617-621`)写了 webkit 滚动条,对话栏/右栏都没有。修复时两条都要落:恢复栏级滚动容器 + 补 Chromium 滚动条样式。
- **② Markdown 标题泄漏**:回答里 `#### 2. ISO新标准发布与更新` 以字面 `####` 显示——渲染器没覆盖 h4(或输出清洗把 `#` 转义/放行了),查 assistant 内容的 markdown 渲染链。
- **③「回到底部」浮钮压字**:sticky 按钮(`workspace-chat.module.css:33+` `.scrollToBottomButton`)与正文文本重叠,文本在按钮后面被裁/透出;需调偏移、背景不透明度或避开文本流。

### #17 内容源/笔记/citation 一律改中间弹窗(不在原地切换) ✅

- **需求(用户拍板,2026-08-06)**:点击**内容源条目**→中间弹窗(不在中栏原地切换);**新建笔记、笔记条目**→弹窗;**citation**→弹窗。
- **现状锚点**:
  - citation 点击现在是**锚点定位**式(`chat-message-list.tsx:239-247` 传 `anchorRect`,`citation-renderer.tsx:442-452`),不是居中弹窗;
  - 内容源/笔记在中栏原地切换(`workspace-sources-pane.tsx`、`workspace-notes-pane.tsx` + note editor);
  - 已有可复用的弹窗基建:`settings-quick-modal.tsx`、`workspace-share-quick-modal.tsx`、`workspace-api-access-modal.tsx` 的形态,`modalEnter` 动画(`_app-shared.css:431`)已存在。
- **要点**:统一一个居中大弹窗组件(尺寸建议参考尺寸分析:内容区可达 ~740-1000px,内部滚动);文档/笔记内容长,弹窗要有关闭(✕/Esc/点遮罩)与返回列表的层级;新建笔记弹窗保存后直接体现在列表。

### #18 对话栏顶部模式指示重复表达,删除 ✅

- 现象(截图 2026-08-06_134349 红框):对话栏顶部 header 显示「知识库」标题 + 「rag」模式 chip(`workspace-chat-pane.tsx:225-230`,`activeModeLabel`/`activeModeCode`),与**底部输入框的能力开关**(`chat-composer.tsx:29-30`,知识库/网络搜索 toggle)重复表达同一信息。
- 期望:删除该 header(整行移除,含标题与 mode chip);当前模式以底部 composer 开关为准。
- 备注:每条消息气泡上还有**逐条**的 capability chip(`chat-message-list.tsx:363-380`,`mode-indicator`)——那是单条回答的来源标记,与本项无关,保留(若要一并精简再议)。

### #19 顶栏胶囊按钮组样式对齐(截图 2026-08-06_134425) ✅

- 现象:顶栏右侧按钮组(升级 / +新建工作区 / 分享 / API / 账户)各胶囊样式不齐——「升级」实心暖橙、「新建工作区」浅底、「分享」「API」无底色幽灵态且带小图标、「账户」白底带边,高度/圆角/底色不统一。
- 期望(用户拍板):
  1. **胶囊样式对齐**:统一高度、padding、圆角、字重(共用基类已有,见下);
  2. **分享、API 去掉小图标,只留文字**;
  3. 底色在**暖橙 / 灰色**中选一套(升级保持暖橙主强调,其余灰底中性,或按此原则定稿);
  4. **「账户」胶囊底色与整组对齐**。
- 位置:`workspace-top-bar.tsx:135-205`(`.topBarActions` 组,分享/API 按钮含 `actionIcon` svg + label);样式 `workspace-shell.module.css:131-254`(`.topBarPrimaryButton`/`.topBarActionButton` 基类:min-height ≈36px、padding 8/16、radius `--radius-button`);「升级」= `plan-entry` 组件,「账户」= `account-menu` 触发器,**两处是独立样式,需并轨**。

### #20 Workspace 默认命名太短/出现裸「1」 ✅

- 现象(截图 2026-08-06_134627):顶栏标题只有「1」。用户要求:默认名换长一点、有意义的方法,例如「**新建工作区1**」。
- 代码现状:默认名由前端生成——`lib/dashboard/default-title.ts:18-22` `formatDefaultWorkspaceTitle` = `工作区{N}`/`Workspace{N}`,N 来自 **localStorage 计数器**(按 locale 分键;清缓存归零→重名,删工作区不回退→跳号);两个创建入口都用它(`dashboard-surface.tsx:156`、`use-workspace-data.ts:125-132`),后端把 name 原样写入 title 列(`storage-pg/src/lib_impl/repository_bootstrap.rs:111-122`)。
- **疑点**:现行生成器产出的是「工作区1」而非裸「1」,截图里的「1」对不上——修复时先新建一个工作区复现,确认是否还有别的命名路径(旧构建/桌面本地运行时/手动改过)。
- 期望:默认名改为「新建工作区N」(en:「New Workspace N」)或更有信息量的方案(如带日期);计数器改服务端口径可免清缓存重名,前端先改 base 即可解主诉。

### #21 解析无进度反馈:2 分 22 秒只有转圈(诊断结论:非卡死) ⬜

- **用户报告**:文档解析「始终转圈」(截图 14:02:38,CLAUDE-FABLE-5.md)。
- **诊断结论(2026-08-06,证据链完整)**:**后端没有卡死**。worker 日志全时间线(本地,UTC+8):
  - 14:00:30 任务开始(route=Local)→ parse_validate 1.9s → materialize 18.4s → summary(LLM)9.1s → embed 106 向量 4.2s(14:01:03 完成);
  - **随后 KG 三元组抽取 ~109s(占总时长 76%),期间无任何阶段更新**;
  - 14:02:49 完成(截图后 11 秒),`documents.status='completed'`、chunk 106;curl 实测 API 返回 `completed`;前端轮询(2s,terminal 含 completed)逻辑上会在 ~2s 内落定。
- **真正的问题(UX)**:两分多钟只有一个转圈,无任何阶段提示(「正在抽取知识图谱…」之类),用户无法区分「在跑」和「卡死」;KG 抽取是大头却最沉默。
- **附带**:诊断中发现 RLS 会让 psql 直查返回 0 行造成「库是空的」误判——本地排查先 `set_config('app.current_role','super_admin',false)`。
- **待用户确认**:刷新页面后是否仍转圈。若仍转 → 前端轮询在其会话中停止(token/401 或后台 tab 暂停),转客户端深挖;若已落定 → 本项就是纯进度反馈 UX 问题。
- **期望**:解析过程展示阶段进度(至少阶段文案:解析→切块→摘要→向量化→知识图谱);或 spinner 旁显示已耗时。

---

## 修改记录

| 日期 | 提交 | 范围 |
|------|------|------|
| 2026-08-06 | f2e7516a | W1 叙事翻转(#1,2,3,5,6,7)✅ 验收过 |
| 2026-08-06 | d38143b8 | W7 默认命名(#20)✅ |
| 2026-08-06 | fa33a68b | W2 分享体系(#12–15)⚠️ 首轮打回 |
| 2026-08-06 | 5d23ad00 | W3 设置弹窗/胶囊(#9,10,19,IA)✅ |
| 2026-08-06 | 83f5c044 | W4 通知(#11)⚠️ 首轮打回(广播无鉴权) |
| 2026-08-06 | ac2a52ed | W5 对话区(#16,17,18)✅ |
| 2026-08-06 | 9bb50afb | W6 资料名片(#8)✅ |
| 2026-08-06 | 096b05b6 | 遗留:Keygen crate 删除 + hold reaper ✅ |
| 2026-08-06 | e6a5955d | 红项:广播鉴权✅ 邀请链接✅ 成员 Owner-pays✅(代码对齐 ADR) |
| 2026-08-06 | 98cf31be | 应修 4–9:限次 UI✅ docs 卡✅ e2e✅;分析页口径/余额阈值/后端通知 i18n 部分 |
| 2026-08-06 | a215e3a8 | 润色:档位标/右栏滚动条/cite 跳转/媒体删除/死代码 ✅ |
| 2026-08-06 | (pending) | 复审尾巴: Owner-pays 需 share_enabled; 广播角色单测; 邀请 URL/inviter 名; 限次草稿解耦; 清 anchorRect/appearance 死 CSS |
