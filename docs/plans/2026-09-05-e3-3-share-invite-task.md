# E3.3 任务记录：分享中心、公开分享、分享者主页与工作区邀请 (`/share/*`, `/shared/*`, `/invite/*`)

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §6 (E3.3)
门禁目标：E3.3 切片完成

---

## 1. 任务目标与交付范围

按全量路由迁移矩阵，交付分享与邀请路由族（共 6 个端点）：
1. **工作区分享中心 (`/dashboard/:workspace_id/share`)**：
   - 包含分享开关状态、生成公开分享链接（带 Token）、复制分享链接；
   - 子页面：`/dashboard/:workspace_id/share/access-logs`（访问审计日志）、`/dashboard/:workspace_id/share/analytics`（分享会话互动与流量分析）。
2. **公开分享知识库查看与问答 (`/shared/kb/:token`)**：
   - 访客通过只读 Token 访问已分享的工作区只读知识库；
   - 复用受限 `ChatCanvas`：用户仅能基于该被分享的知识库提问，**绝不能扩大 scope 访问私有文档或发起写操作**；
   - 失效/未授权 Token 时展示清晰的失效友好提示。
3. **分享者公开主页 (`/shared/u/:userId`)**：
   - 展示公开分享者的对外展示信息（头像、昵称、个人简介、已公开的工作区列表）；
   - 若用户未开启 `public_profile_enabled`，展示“分享者未公开主页”状态。
4. **工作区成员邀请接受页面 (`/invite/:workspace_id/:member_id`)**：
   - 邀请卡片呈现：工作区名称、邀请角色；
   - 已登录用户点击“接受邀请”调用加入接口，未登录用户提供引导先登录/注册后自动回跳接受。
5. **安全与工程规范**：
   - 访问者受限上下文：`/shared/kb/:token` 请求自带 `source_token`，不携带用户全局凭据；
   - 样式遵从 `--cos-*` 变量，字重 400，无裸十六进制颜色与未授权阴影。

---

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E3.3.1 | SDK 分享与邀请契约及 REST 接口补齐 | 已完成 | `web-sdk/src/share_api.rs`, 测试 3/3 passed |
| E3.3.2 | 实现工作区分享管理三页面 (`/share`, `/access-logs`, `/analytics`) | 已完成 | `share_manage_page.rs` 三页面就位 |
| E3.3.3 | 实现公开知识库问答页面 (`/shared/kb/:token`) | 已完成 | `shared_kb_page.rs` 正常与失效路径覆盖 |
| E3.3.4 | 实现分享者主页与工作区邀请页面 (`/shared/u/:userId`, `/invite/*`) | 已完成 | `shared_user_page.rs`, `invite_page.rs` 就位 |
| E3.3.5 | 路由挂载、样式集成与自动化测试断言 | 已完成 | `share-invite-journey.spec.ts` 4/4, 全量 35/35 passed |
| E3.3.6 | 验证收敛、图谱更新与本地提交 | 进行中 | 待提交本地 commit |
