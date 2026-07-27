# IA & Navigation Flow（V1）

更新时间：2026-04-13

## 1. 关键交互决策（最新）

- Workspace 左栏 `New Thread` 下方改为“历史”按钮（非输入框）。
- 点击“历史”进入线程历史视图（当前 workspace 内切换，不新增历史路由）。
- 历史视图仅保留 `Thread` 单 Tab。
- 历史视图仅保留关键词检索，不保留“选择/类型”筛选行。
- 点击历史候选项后，跳转到对应 session。

- Dashboard 与 Workspace 顶栏新增用户菜单入口（头像按钮）。
- 用户菜单包含：账户与额度、设置、帮助文档、退出登录。

- Settings 提供：
  - 视觉主题：浅色 / 深色
  - 界面语言：中文 / 英文
  - 帮助入口：跳转 Wiki 帮助文档

- 设计稿要求：页面不混语（品牌名 `Context-OS` 可保留）。

## 2. 页面与路由映射

1. `/` 首页  
2. `/login` 登录  
3. `/register` 注册  
4. `/reset-password` 找回密码  
5. `/reset-password/verify` 验证码  
6. `/reset-password/confirm` 重置密码  
7. `/dashboard` Workspace 列表  
8. `/dashboard/:notebook_id` Workspace  
9. `/dashboard/:notebook_id/share` 分享中心  
10. `/dashboard/:notebook_id/api-access` API 访问  
11. `/settings` 设置  
12. `/shared/kb/:token` 外链只读  
13. `/invite/:notebook_id/:member_id` 邀请接收

## 3. 核心导航规则

1. 未登录访问受保护路由 -> `/login`
2. 登录成功 -> `/dashboard`
3. 打开 notebook -> `/dashboard/:notebook_id`
4. Workspace 顶栏 logo（图标/文字）-> `/dashboard`
5. Workspace 顶栏 `Share/API/Settings` -> 对应模块页
6. 顶栏头像 -> 用户菜单
7. 用户菜单：
   - 账户与额度 -> 账户额度页
   - 设置 -> Settings 页
   - 帮助文档 -> Wiki 帮助页
   - 退出登录 -> `/login`
8. Workspace 左栏“历史”按钮 -> 历史检索视图
9. 历史候选项 -> 对应 session

## 4. 帮助文档范围（Wiki）

- 账户认证（注册、登录、改密、重置）
- Workspace 与资料源管理（文件上传、URL 导入）
- 会话与历史检索
- 分享与邀请协作
- API Key 管理
- 通知与偏好
- 额度与限制
- 运行状态与运维端点
