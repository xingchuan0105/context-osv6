# context-osv6 E2E 用户全生命周期操作清单

> Date: 2026-03-21
> Goal: 用真实浏览器模拟用户和内容所有者的完整使用路径，按 PRD 验证核心链路
> Execution note: 本轮只测试、记录问题，不修复

## 1. 访客与注册阶段

### C01. 打开首页与公共路由
- 访问 `/`
- 验证是否正确落到公开入口或重定向逻辑
- 打开 `/login`
- 打开 `/register`
- 打开重置密码相关页面

### C02. 新用户注册
- 使用新邮箱注册
- 验证注册后是否进入已登录态或可顺利登录
- 验证登录态跳转是否正确

### C03. 登录与登出
- 使用刚注册账号登录
- 验证登录后跳转到 `/dashboard`
- 刷新页面，验证登录态是否持久化
- 进入 `/settings`
- 执行登出
- 验证受保护页面被拦到 `/login`

## 2. Workspace 所有者工作流

### C04. 创建 Workspace
- 登录后进入 `/dashboard`
- 验证空状态 / 列表状态
- 创建一个新 notebook
- 验证创建后 notebook 出现在列表中
- 点击 notebook 进入工作区

### C05. 工作区基础结构
- 验证三栏布局
- 左栏包含 sessions / sources / drafts
- 中栏为 chat
- 右栏包含 evidence / trace / session
- 验证 workspace 标题正确显示 notebook 名

### C06. 文档上传
- 打开上传文档入口
- 选择一个小体积测试文档
- 提交上传
- 验证上传状态从 processing/queued 过渡到 ready 或 failed
- 验证 sources 列表出现该文档

### C07. URL Source
- 在 Sources 面板输入一个 URL
- 提交添加
- 验证来源条目是否出现
- 验证状态轮询是否结束

### C08. 文档查看与预览
- 点击 source 打开详情
- 验证能看到 parsed preview 或 content
- 验证 reindex / delete 按钮可见

## 3. 核心聊天与引用链路

### C09. RAG Chat
- 在 notebook 工作区发送一个和上传文档相关的问题
- 验证聊天状态机：submitting -> streaming -> done|error
- 验证首 token 流式出现
- 验证最终回答渲染
- 验证 citations 面板有内容
- 验证 trace/rag 信息出现

### C10. Citation 跳转
- 点击回答中的 citation
- 点击 Evidence 面板中的 citation
- 验证左栏切到 Sources
- 验证目标文档被自动选中
- 验证 parsed preview 定位、高亮、滚动行为

### C11. Session 切换
- 发起第二轮对话
- 返回 Sessions 面板
- 点击历史 session
- 验证消息历史重新加载
- 验证 session 切换没有明显上下文污染

### C12. Search / General 模式
- 切换到 Search 模式提问
- 验证搜索答案与 sources 分组结果
- 切换到 General 模式提问
- 验证无 notebook 依赖的普通回答链路

### C13. Degrade 可见性
- 在聊天过程中观察是否出现 degrade banner
- 若出现，验证至少展示原因
- 记录是否缺少阶段 / 可信度影响描述

## 4. 分享与协作链路

### C14. Share Center
- 打开 `/dashboard/:notebook_id/share`
- 验证 settings / analytics / access logs 标签
- 创建分享链接
- 可选填写过期时间
- 验证生成后的分享 token / access level 显示

### C15. 协作者邀请
- 在 share center 邀请一个邮箱
- 验证成员列表是否出现 pending 成员
- 记录邀请成功 / 错误状态

### C16. 公共分享页
- 打开 `/shared/kb/:token`
- 验证 notebook 标题、描述、权限、过期时间、sources 列表
- 在共享页提问
- 验证 streaming、citations、retrieved sources、degrade banner

## 5. 账号与可编程接入

### C17. Settings
- 打开 `/settings`
- 更新 profile
- 修改密码
- 查看 notifications
- 验证主路径可用性

### C18. API Access
- 打开 `/dashboard/:notebook_id/api-access`
- 创建 API key
- 配置权限、rate limit、expires_at
- 验证 key 列表出现
- 验证 revoke 可操作
- 验证 REST / OpenAI / MCP 示例展示正确

## 6. Admin Smoke

### C19. Admin 基本页
- 打开 `/admin`
- 验证 organizations / users / usage / billing / health / rag-health / feature-flags / workers / degradation / audit-logs
- 验证 AdminShell 左侧导航

### C20. Feature Flags
- 打开 feature flags 页面
- 查看 flag 列表与 change requests
- 提交一个 change request
- 审批或拒绝一个 request
- 记录审批链路实际表现

## 7. 结果记录方式

每个用例记录以下结果：
- `PASS`
- `FAIL`
- `BLOCKED`
- `NOT_APPLICABLE`

每条失败记录至少包含：
- 用例编号
- 页面 / 路由
- 用户操作
- 实际结果
- 预期结果
- 是否阻断后续测试

