# E3.2 任务记录：工作区控制台与持久知识库管理 (`/dashboard/*`)

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §6 (E3.2)
门禁目标：E3.2 切片完成

---

## 1. 任务目标与交付范围

按全量路由迁移矩阵，交付工作区控制台与管理路由族：
1. **工作区列表与概览 (`/dashboard`)**：
   - 展示用户拥有的工作区卡片列表（名称、文档数、成员数、更新时间）；
   - 支持创建新工作区（名称、描述），创建后直达新工作区控制台；
   - 快速入口回到个人对话。
2. **工作区工作台 (`/dashboard/:workspace_id`)**：
   - 顶部工作区信息展示、成员/分享/分析快速跳转；
   - 侧边栏/主区域：
     - 复用 W2/W2.8 完整聊天（`ChatCanvas`，`scope_kind=Workspace`，`model_role=agent`）；
     - 持久知识库来源文件管理面板（来源列表、上传持久资料、入库状态 `pending`/`processing`/`completed`/`failed` 流转、删除文件）；
     - 工作区笔记面板（创建笔记、保存 Markdown、笔记列表，复用 E1 确立的 Tiptap 纯 JS Bridge 方案）。
3. **工作区深度分析 (`/dashboard/:workspace_id/analyze`) & 全局分享统计 (`/dashboard/analytics`)**：
   - 工作区文档切片/向量检索健康度概览与分享流量只读展示。
4. **安全与工程不变量**：
   - T7/T8：Workspace 是唯一跨会话复用的持久知识库容器；会话文件与工作区资料生命周期隔离；
   - 遵从设计系统规范：字重 400，使用 `--cos-*` 变量，无裸十六进制颜色与未授权阴影。

---

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E3.2.1 | SDK 工作区与资料 REST 接口补齐 | 已完成 | `web-sdk/src/workspace_api.rs`, 测试 4/4 passed |
| E3.2.2 | 实现工作区概览与创建面板 (`/dashboard`) | 已完成 | `dashboard_overview.rs`, 支持卡片与弹窗建库 |
| E3.2.3 | 工作台持久来源与笔记面板集成 (`/dashboard/:id`) | 已完成 | `workspace_workbench.rs`, 聊天+右轨管理 |
| E3.2.4 | 分析与统计页面落地 (`/dashboard/analytics`, `/analyze`) | 已完成 | `workspace_analytics.rs` 双页面就位 |
| E3.2.5 | 路由挂载、样式集成与自动化测试断言 | 已完成 | `workspace-journey.spec.ts` 3/3, 全量 31/31 passed |
| E3.2.6 | 验证收敛、图谱更新与本地提交 | 进行中 | 待提交本地 commit |
