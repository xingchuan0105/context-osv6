# context-osv6 长线开发计划

> Date: 2026-03-21
> Scope: `avrag-rs` + `frontend_rust`
> Goal: 将当前“可联调、可验收”的状态推进到“可稳定内测、可发布、可治理”

## 1. 计划目标

这份计划覆盖四类长线工作：

- 审批流
- E2E 端到端测试
- 发布级治理
- 其余影响上线质量的长线项

目标不是继续堆功能，而是把系统补到：

- 可持续迭代
- 可稳定回归
- 可安全上线
- 可观测、可回滚、可审计

---

## 2. 总体优先级

### P0：下一阶段必须做

- 核心用户路径 E2E
- 发布 smoke 测试
- API / 数据迁移 / 回滚治理最小闭环
- 关键后台危险操作的二次确认与审计补强
- RAG 质量评测从占位变为真实执行

### P1：进入内测前应完成

- feature flags 审批流
- API 契约治理完善
- 运行可观测性与告警
- 分享滥用与外部 API 安全策略补强

### P2：正式上线前视规模补齐

- 更完整的运营后台工作流
- 更细的配额、风控和灰度发布能力
- 更完整的发布自动化和质量门禁

---

## 3. 工作流拆分

## Workstream A：审批流

### A1. Feature Flags 审批流

当前状态：

- 已支持 feature flag 列表、更新、审计日志
- 但仍是“管理员直接修改立即生效”

目标：

- 任何高风险开关变更都经过申请、审批、执行、回滚四步

后端任务：

- 新增 `feature_flag_change_requests` 表
- 字段至少包含：
  - `id`
  - `flag_key`
  - `target_value`
  - `reason`
  - `requested_by`
  - `approved_by`
  - `status`
  - `created_at`
  - `approved_at`
  - `executed_at`
- 新增接口：
  - 创建变更申请
  - 查询申请列表
  - 审批 / 拒绝申请
  - 执行申请
  - 回滚最近一次生效变更
- 审计日志补充：
  - `feature_flag_change_requested`
  - `feature_flag_change_approved`
  - `feature_flag_change_rejected`
  - `feature_flag_change_executed`
  - `feature_flag_change_rolled_back`

前端任务：

- 管理台 feature flags 页面增加：
  - 申请变更表单
  - 待审批列表
  - 已执行历史
  - 回滚入口
- 显示变更原因、申请人、审批人、时间线
- 危险开关增加确认弹窗

验收标准：

- 管理员不能直接无痕改 flag
- 每次开关变更都有申请链路和审计记录
- 能按记录回滚

必要性：

- 单人开发：中
- 团队内测：高
- 正式上线：高

### A2. 高风险后台操作审批 / 二次确认

覆盖操作：

- Block / Unblock organization
- 删除 API key
- 删除分享成员
- 撤销分享链接
- 未来的计费、配额、租户级变更

目标：

- 至少做到二次确认
- 关键操作带理由输入
- 更高等级场景可复用审批流框架

任务：

- 为后台写操作增加 `reason` 字段支持
- 前端确认弹窗补齐文案和风险提示
- 审计日志记录操作前后值

验收标准：

- 高风险写操作不再是“一键无说明执行”

必要性：

- 内测：中高
- 上线：高

---

## Workstream B：E2E 端到端测试

## B1. 搭建 E2E 基础设施

推荐：

- 前端 E2E：Playwright
- 测试环境：单独 `.env.e2e`
- 数据准备：测试组织 / 用户 / notebook fixtures

任务：

- 增加 E2E 启动脚本
- 提供测试数据库与测试对象存储配置
- 增加测试数据清理脚本
- 固化登录账号与种子数据

交付物：

- `tests/e2e/`
- `scripts/e2e-up.sh`
- `scripts/e2e-down.sh`
- `scripts/e2e-reset.sh`

## B2. 核心用户流 E2E

必须覆盖：

1. 注册 / 登录 / 登出
2. 创建 notebook
3. 上传文档
4. 等待 ingestion 完成
5. 发起 SSE chat
6. 验证 citations 出现
7. 点击 citation 跳转 source viewer
8. 创建 share link
9. 打开 shared notebook 页面并提问

扩展覆盖：

1. 添加 URL source
2. Session 切换
3. API key 创建 / 撤销
4. 设置页更新 profile / password

验收标准：

- 每次主干变更后能自动验证主用户流程未坏

必要性：

- 内测：高
- 上线：高

## B3. 管理台 E2E

覆盖：

1. Admin 登录
2. 组织列表加载
3. Users / Usage 页面按 org 切换
4. Feature flags 变更申请
5. 审计日志查看
6. Health / RAG Health / Workers / Degradation 页面加载

验收标准：

- 后台主操作入口至少 smoke 级通过

必要性：

- 内测：中
- 上线：高

## B4. 外部 API / 接入流 E2E

覆盖：

1. 创建 notebook API key
2. 用 API key 访问 `/api/v1/notebooks/{id}/query`
3. 用 OpenAI Compatible API 发请求
4. MCP ready / tool call 基本链路

验收标准：

- 前端接入面板给出的示例是能工作的，不只是展示代码片段

必要性：

- 有对外接入目标时：高

---

## Workstream C：发布级治理

## C1. 发布 smoke 测试

目标：

- 每次发布前 5 到 10 分钟内完成最关键回归

覆盖：

- 服务启动
- `/health`
- `/ready`
- `/openapi.json`
- 登录
- notebook 列表
- 上传
- chat
- share
- admin 健康页

任务：

- 编写 `scripts/release-smoke.sh`
- 输出明确成功 / 失败摘要

验收标准：

- 发版前能一键知道“能不能发”

必要性：

- 内测：高
- 上线：高

## C2. 回滚治理

目标：

- 发布失败时能快速退回上一版本

任务：

- 约定前后端版本号与发布标记
- 数据迁移分级：
  - 可回滚迁移
  - 不可回滚迁移
- 为每个 migration 标注回滚策略
- 形成回滚 runbook：
  - 应用回滚
  - 静态资源回滚
  - DB 回滚 / 前滚补救

验收标准：

- 发布失败时有明确文档和脚本，不靠人工临场判断

必要性：

- 上线：高

## C3. API 契约治理

当前状态：

- 已有更完整的 `/openapi.json`
- 但仍是手写维护，不是自动契约体系

目标：

- API 变更可追踪、可 diff、可提示破坏性变化

任务：

- 统一每个 API 的：
  - `operation_id`
  - `summary`
  - `tags`
  - `security`
  - `errors`
- 建立 OpenAPI 快照检查
- 增加契约 diff 检查脚本
- 明确 `/api/v1` 与未来 `/api/v2` 变更规则
- 对破坏性变更补：
  - `Deprecation`
  - `Sunset`
  - 迁移说明

验收标准：

- 新接口不会再“加了就加了”
- 破坏性变更有制度化入口

必要性：

- 内测：中
- 对外 API / 上线：高

## C4. 可观测性与请求追踪

目标：

- 问题出现时能沿 `request_id` 找到完整链路

任务：

- 前后端日志统一带 `request_id`
- 聊天链路关键阶段日志化：
  - auth
  - upload
  - parse
  - index
  - chat
  - SSE done/error
- 错误聚合：
  - API error rate
  - upload failure rate
  - SSE error rate
  - guard degrade frequency
- 关键指标面板：
  - QPS
  - ingestion queue
  - failed docs
  - share access spikes

验收标准：

- 出问题时不是“靠猜”

必要性：

- 内测：中高
- 上线：高

## C5. CI / 质量门禁

任务：

- CI 阶段拆分：
  - format
  - lint
  - unit test
  - integration test
  - E2E smoke
  - OpenAPI diff
  - migration check
- 对主分支设置必须通过项

验收标准：

- 高风险回归不能直接进主干

必要性：

- 团队协作：高

---

## Workstream D：RAG 质量与评测

## D1. 真实评测接线

当前状态：

- `tests/rag_quality` 仍有 placeholder

目标：

- 跑真实 retrieval / answer / citation 质量评估

任务：

- 接通真实 runtime
- 输入黄金集
- 记录：
  - answer correctness
  - citation coverage
  - citation precision
  - degrade occurrence
  - latency

验收标准：

- 每次重要模型 / 检索 / prompt 变更都有量化结果

必要性：

- RAG 产品：高

## D2. 质量门限

任务：

- 给关键指标设门限
- 回归超阈值时阻止发布或至少报警

建议门限：

- citation 缺失率
- 错误引用率
- 无答案率
- degrade 上升幅度

必要性：

- 上线：高

---

## Workstream E：安全与风控长线项

## E1. 分享滥用与公开访问防护

任务：

- shared notebook 路由增加速率限制
- 记录 share token 滥用
- 异常访问触发告警
- 为公开 link 加更清晰的期限和状态展示

必要性：

- 如果开启 public / link share：高

## E2. API Key 生命周期治理

任务：

- 过期 key 清理
- 最近使用时间更新
- 更细粒度权限校验回归
- 风险 key 列表
- 可选的 key rotate 工作流

必要性：

- 对外 API：高

## E3. 管理员权限分层

任务：

- 落实 `super_admin / ops_admin / finance_admin`
- 前端页面与按钮按角色显示
- 后端写操作权限继续细分

必要性：

- 多人运营：高

---

## Workstream F：产品打磨长线项

## F1. 搜索体验继续对齐 v5 / PRD

任务：

- 更完整分组
- 跳转入口更细
- 更好的结果摘要和导航

必要性：

- 产品体验提升，中优先级

## F2. 分享中心继续增强

任务：

- token 生命周期可视化
- 多 token 管理
- revoke / regenerate 更清晰
- 过期、撤销状态展示

必要性：

- 分享能力变多时，中高优先级

## F3. Admin 运营体验增强

任务：

- 更强筛选
- 更细日志查询
- 更真实 billing / usage drill-down
- 更细 health / degradation 说明

必要性：

- 内部运营阶段，中优先级

---

## 4. 分阶段落地计划

## Phase 1：质量闭环基础版

周期建议：

- 1 到 2 周

目标：

- 先补“能不能放心继续开发”

范围：

- 核心 E2E 基础设施
- 核心用户流 E2E
- 发布 smoke
- 回滚 runbook 初版
- RAG 评测接线初版

退出标准：

- 主用户链路有自动化兜底
- 每次发版前有 smoke

## Phase 2：治理基础版

周期建议：

- 1 到 2 周

范围：

- feature flag 审批流
- 高风险操作二次确认
- OpenAPI 契约检查
- CI 质量门禁
- `request_id` 全链路观测补强

退出标准：

- 后台高风险操作可审计
- API 变更有契约边界

## Phase 3：内测准备版

周期建议：

- 1 到 2 周

范围：

- 管理台 E2E
- API key / OpenAI / MCP E2E
- 分享滥用防护
- API key 生命周期治理
- RAG 质量门限

退出标准：

- 产品主链路和平台链路都可回归

## Phase 4：发布准备版

周期建议：

- 1 周

范围：

- 发布 runbook 完整化
- 监控面板与告警
- 灰度 / 回滚演练
- 发布 checklist 固化

退出标准：

- 具备正式上线条件

---

## 5. 任务排序建议

### 第一优先级

1. 核心用户流 E2E
2. 发布 smoke
3. 回滚 runbook
4. RAG 评测接线

### 第二优先级

1. feature flag 审批流
2. 高风险后台操作二次确认
3. OpenAPI diff / 契约门禁
4. CI 质量门禁

### 第三优先级

1. 管理台 E2E
2. 对外 API / MCP E2E
3. 分享滥用防护
4. API key 生命周期治理
5. 管理员权限分层

### 第四优先级

1. 搜索 / 分享 / 管理台产品体验继续打磨
2. 更细粒度灰度与治理能力

---

## 6. 角色建议

### Backend

- 审批流数据模型与接口
- 发布治理
- 契约治理
- 安全与风控
- 评测体系

### Frontend

- 审批流页面
- 管理台确认与时间线展示
- E2E 测试脚本
- 分享与 API Access UX

### QA / 产品验收

- 验收 checklist
- 风险清单
- 发布前回归

---

## 7. Definition of Done

满足以下条件，才算长线项阶段性完成：

- 主链路 E2E 在 CI 可跑
- 发布前 smoke 有脚本且可执行
- 高风险后台操作有确认和审计
- feature flags 有审批链路
- OpenAPI 能做契约检查
- RAG 质量评测可跑且有门限
- 线上问题可以通过日志和 `request_id` 快速定位

---

## 8. 必要性结论

### 对当前阶段最必要

- 核心 E2E
- 发布 smoke
- 回滚治理
- RAG 质量评测

### 对团队化和内测最必要

- feature flag 审批流
- CI 质量门禁
- OpenAPI 契约治理
- 管理台高风险操作二次确认

### 对正式上线最必要

- 可观测性
- 告警
- 分享 / API key 风控
- 权限分层
- 完整发布 runbook

---

## 9. 推荐执行顺序

建议按这个顺序推进：

1. 核心 E2E
2. 发布 smoke
3. 回滚 runbook
4. RAG 质量评测
5. feature flag 审批流
6. 高风险后台操作确认和审计增强
7. OpenAPI diff + CI 门禁
8. 管理台 / API 接入 E2E
9. 安全与风控
10. 产品体验长尾打磨

