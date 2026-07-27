# Frontend 接线清单 V1（按最新定稿原型）

更新时间：2026-04-13  
Figma：`https://www.figma.com/design/b4Umj3ZtgO8VReZfa6tBCh`

## 0. 核心约束

- [ ] Workspace 与 Dashboard 必须有用户菜单入口（头像按钮）
- [ ] 用户菜单必须可到：账户与额度、设置、帮助文档、退出登录
- [ ] Workspace 左栏 `New Thread` 下方为“历史”按钮，不是搜索输入框
- [ ] 历史页仅保留 `Thread` Tab，移除“选择/类型”筛选行
- [ ] 支持两套主题：浅色、深色
- [ ] 支持两套语言：中文、英文
- [ ] 单一页面不得中英文混搭（品牌名 `Context-OS` 例外）

## 1. P0 页面接线（必须）

### 1.1 认证与主流程

- [ ] `/login` -> `HiFi_Login`（`8:4`）
- [ ] `/dashboard` -> `HiFi_Dashboard`（`8:22`）
- [ ] `/dashboard/:notebook_id` -> `HiFi_Workspace`（`8:67`）

### 1.2 登录状态帧

- [ ] Focus -> `HiFi_Login_Focus`（`30:2`）
- [ ] Loading -> `HiFi_Login_Loading`（`30:32`）
- [ ] Error -> `HiFi_Login_Error`（`30:62`）

### 1.3 Dashboard / Workspace 状态帧

- [ ] Hover -> `HiFi_Dashboard_Hover`（`30:92`）
- [ ] Empty -> `HiFi_Dashboard_Empty`（`30:151`）
- [ ] Loading -> `HiFi_Workspace_Loading`（`30:210`）
- [ ] Error -> `HiFi_Workspace_Error`（`30:277`）

## 2. 用户菜单与账户额度（新增 P0）

- [ ] 用户菜单（中文浅色）-> `HiFi_UserMenu_ZH_Light`（`56:22`）
- [ ] 账户与额度（中文浅色）-> `HiFi_AccountQuota_ZH_Light`（`56:107`）
- [ ] 用户菜单（英文深色）-> `HiFi_UserMenu_EN_Dark`（`60:2`）
- [ ] 账户与额度（英文深色）-> `HiFi_AccountQuota_EN_Dark`（`60:87`）

接口对接：
- [ ] `/api/auth/me`：账户基本信息
- [ ] `/api/auth/usage-limit`：额度信息（总量/已用/剩余/重置时间）

## 3. Settings + 主题语言（新增 P0）

- [ ] 设置页（中文浅色）-> `HiFi_Settings_ZH_Light`（`56:197`）
- [ ] 设置页（英文深色）-> `HiFi_Settings_EN_Dark`（`58:150`）

主题帧：
- [ ] 中文浅色 -> `HiFi_Workspace_ZH_Light`（`58:2`）
- [ ] 中文深色 -> `HiFi_Workspace_ZH_Dark`（`61:2`）
- [ ] 英文浅色 -> `HiFi_Workspace_EN_Light`（`61:76`）
- [ ] 英文深色 -> `HiFi_Workspace_EN_Dark`（`58:76`）

实现规则：
- [ ] 主题切换只改变视觉 token，不改变语言
- [ ] 语言切换只改变文案与本地化资源，不改变业务态
- [ ] `设置` 按钮在中文流跳中文设置，在英文流跳英文设置

## 4. 帮助页面（Wiki，新增 P0）

- [ ] 帮助页（中文浅色）-> `HiFi_Help_Wiki_ZH_Light`（`56:291`）
- [ ] 帮助页（英文深色）-> `HiFi_Help_Wiki_EN_Dark`（`60:175`）

帮助内容必须覆盖（基于后端能力）：
- [ ] 账户认证（注册/登录/改密/重置）
- [ ] Workspace 管理（创建/更新/删除）
- [ ] 资料源（文件上传、URL 导入）
- [ ] 会话与历史检索
- [ ] 分享链接与访问分析
- [ ] 邀请协作（邀请/接受/拒绝/移除）
- [ ] API Key 管理（创建/列表/吊销）
- [ ] 通知与偏好保存
- [ ] 额度与 usage-limit
- [ ] 运维端点（health/ready/metrics/openapi/docs）

## 5. Workspace 历史检索（P0）

- [ ] 历史视图：`HiFi_Workspace_History`（`51:17`）
- [ ] 候选跳转：`Session_1`（`51:110`）、`Session_2`（`51:182`）、`Session_3`（`51:254`）
- [ ] 点击“历史”进入历史页，点击候选回对应 session

## 6. 扩展路由（P1）

- [ ] `/dashboard/:notebook_id/share`
- [ ] `/dashboard/:notebook_id/api-access`
- [ ] `/settings`
- [ ] `/shared/kb/:token`
- [ ] `/invite/:notebook_id/:member_id`

## 7. 提测前验收

- [ ] Dashboard / Workspace 可打开用户菜单
- [ ] 用户菜单可到 账户与额度 / 设置 / 帮助 / 退出
- [ ] 设置页可切主题、切语言，且页面不混语
- [ ] 中文流与英文流都能独立闭环
- [ ] 历史检索链路可用（按钮 -> 候选 -> session）
- [ ] 帮助文档内容与后端能力一致
