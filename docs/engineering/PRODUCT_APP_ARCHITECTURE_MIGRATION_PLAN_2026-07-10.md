# 产品 App 架构迁移计划（Composition Root + 用例服务）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done — W0–W6 complete** |
| 动机 | 后续功能上线节奏快，AppState 门面债会指数堆积；现在迁到最佳实践形态，比事后还债便宜 |
| 约束 | Solo local trunk；日常 **L1**；行为保持；**不**做大爆炸重写 |
| 相关 | [`TN_REMEDIATION_HANDOFF_2026-07-09.md`](./TN_REMEDIATION_HANDOFF_2026-07-09.md)、[`TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md`](./TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md)、ADR-0006 §5a |

---

## 0. 一句话

把 `AppState` 从 **「百方法业务遥控器」** 变成 **「只组装产品 App 的 composition root」**；Transport 只调 **窄而深的用例服务**；**Write 永远独立 Agent 环**（不是 ToolCatalog 里的 tool）。

---

## 1. 目标架构（最佳实践 × 本产品）

### 1.1 分层

```
Transport (HTTP / MCP / SSE)     ← 薄：解析、鉴权上下文注入、状态码
        │
        ▼
Product Apps（用例入口，按产品面）  ← 厚：编排、权限组合、事务边界
        │
        ▼
Domain / Ports / 现有 crates       ← 规则与实现（storage、share、billing、loop…）
        │
        ▼
Bootstrap / AppState               ← 仅装配 Arc<App>，无新业务方法
```

### 1.2 产品 App 切分（锁定）

| App | 产品职责 | 主要现有归属 | 明确不包含 |
|-----|----------|--------------|------------|
| **WorkspaceApp** | 工作区 CRUD、文档上传/完成、源列表、笔记 | `Bound/documents`、`app-documents` | Agent loop、写作 refine |
| **AgentApp** | Chat / RAG / Search 会话与 SSE | `app-chat`、`agent-loop`、`agent-tools` | Write 状态机 |
| **WriteApp** | 写作任务、refine 环、材料/草稿 | `write-core`、`app-chat` writer | **禁止**进入 `ToolCatalog` |
| **ShareApp** | 成员、链接、访问级别、校验 | `bound/share`、`avrag_share` | LLM |
| **BillingApp** | 订阅、用量、checkout | `bound/billing`、`app-billing` | 检索 |
| **AdminOpsApp** | 运营面：org/user、flags、健康、审计 | `bound/admin*` | 用户侧 chat |
| **PrefsApp**（可薄） | 用户偏好 / agent prefs | `bound/prefs` | — |

> 现有 crate（`app-documents`、`share`、`write-core`…）**继续做深模块**；App 层是 **用例门面**，不是再抄一份业务。

### 1.3 终态 AppState（示意）

```rust
// 目标形态（名字可微调，语义锁定）
pub struct AppState {
    pub workspace: Arc<WorkspaceApp>,
    pub agent: Arc<AgentApp>,
    pub write: Arc<WriteApp>,
    pub share: Arc<ShareApp>,
    pub billing: Arc<BillingApp>,
    pub admin_ops: Arc<AdminOpsApp>,
    pub prefs: Arc<PrefsApp>,
    // 请求级：auth 由 middleware 注入到 App 调用，不在此堆方法
    // 基础设施句柄仅 bootstrap 内部使用，不对外涨业务 API
}
```

Handler 形态：

```text
// 现在
state.docs().create_workspace(...)
state.share().check_access(...)

// 目标
state.workspace.create(CreateWorkspaceCmd { auth, ... }).await
state.share.check_access(CheckAccessCmd { auth, workspace_id }).await
```

### 1.4 铁律（迁移全程不可破）

| # | 铁律 |
|---|------|
| T1 | **停增**：禁止向 `AppState` / 旧 `Bound/*` **新增**业务方法（新功能只进 Product App） |
| T2 | **Write 独立**：`write_refine_*` 是写作 Agent 控制环，**永不**进 `ToolCatalog` / mode `tool_pool` / Capabilities 全表 |
| T3 | **ReAct 单点**：Chat/RAG/Search 工具执行只走 `dispatch_tool` |
| T4 | **C4 不做**：Capability / Skill / Tool 三层保留（ADR-0006 §5a） |
| T5 | **行为保持**：每切片可回滚；L1 绿；相关面定向测 |
| T6 | **Solo**：本地 trunk；日常 `test-l1`；不默认扩 CI |

---

## 2. 为何现在做（与「以后还债」对比）

| | 现在迁移 | 继续堆 AppState |
|--|----------|-----------------|
| 新功能成本 | 用例进对应 App，边界清晰 | 每加一个方法，全库耦合 +1 |
| 回归面 | 按 App 测 | 改分享易碰到 chat 编译 |
| 新人/Agent 上手 | 按产品面找代码 | 百方法菜单 |
| 测试 | 可 mock 单 App | 常举整机 AppState |

功能上线越快，**门面债加速度越大**——适合 **绞杀者迁移**（strangler），不适合等「有空再拆」。

---

## 3. 迁移策略：绞杀者（Strangler），非大爆炸

```
Phase A  立宪 + 骨架 + 第一个完整切片（样板）
Phase B  按产品面迁完（Share → Workspace → Billing → Admin → Prefs）
Phase C  Agent/Write 接线清晰化（不合并 Write）
Phase D  拆除旧 Bound 转发 / 压缩 AppState / 取消 soft warn
```

**每一垂直切片定义：**

1. 新建或扩展 `XxxApp`（结构体 + 用例方法）。  
2. 把 **现有 Bound 方法体** 搬进 App（逻辑不重写）。  
3. Handler / MCP 改调 `state.xxx`（或临时 `state.xxx_app()`）。  
4. 旧 Bound 方法标 `#[deprecated]` 或删（同切片内优先删调用点）。  
5. **L1** + 该面定向测（storage/share lib 或 transport 相关）。  
6. 提交：**一个面一个 commit 序列**，可独立回滚。

---

## 4. 分阶段计划

### Phase A — 立宪与样板（约 2–4 人日）

| 步 | 内容 | 验收 |
|----|------|------|
| A0 | 本文锁定；handoff 链到本文；`bound/mod.rs` freeze 与 T1–T6 一致 | 文档一致 |
| A1 | 建目录惯例：`app-bootstrap/src/product_apps/` 或 `crates/app-*/` 下 `app.rs`（先 bootstrap 内聚，避免过早 crate 爆炸） | 编译 |
| A2 | **样板切片：ShareApp**（Bound 已较独立、面清晰、测相对好控） | 见下 |
| A3 | ADR 短文：Product Apps + Write 独立 + Composition Root | `docs/adr/` 或 engineering 一页 |

**ShareApp 样板验收：**

- `ShareApp` 持有 ports（share store、auth 入参）。  
- 下列 **BoundShare 方法**全部迁入 `ShareApp`（逻辑搬移，不重写）：

| 方法族 | 方法 |
|--------|------|
| 访问 | `check_access` |
| 链接/token | `create_share_token`, `create_share_link`, `revoke_share_link`, `validate_share_token` |
| 设置 | `get_share_settings`, `update_share_settings`, `update_share_access_level` |
| 分析 | `get_share_analytics`, `get_share_access_logs` |
| 成员 | `list_share_members`, `invite_share_member`, `accept_share_invite`, `decline_share_invite`, `remove_share_member`, `share_member_count` |
| 解析 | `get_shared_workspace`, `share_enabled_for_workspace`, `resolve_share_chat_workspace_scope` |

- `transport-http` share handlers + `auth_guard` 中 `state.share()` **只调 ShareApp**（或薄 `state.share` face 转发到 App）。  
- 旧 `BoundShare`：同切片内删调用点，或 `#[deprecated]` 且无剩余生产调用。  
- 无新 AppState 业务方法；`test-l1` + share 相关 lib/contract 测绿。  
- 作为后续面的 **复制模板**（目录、命名、测试习惯）。

**为何 Share 做样板：** 与 LLM 解耦、不碰 Write、Bound 已成块（~234 行 / ~19 方法）、失败成本低于 Workspace 上传链。

### Phase B — 业务面迁完（约 1–2 周，可穿插功能）

| 序 | 切片 | 来源 | 风险 |
|----|------|------|------|
| B1 | **WorkspaceApp** | BoundDocuments + app-documents 用例 | 中（上传/源） |
| B2 | **BillingApp** | BoundBilling | 低～中 |
| B3 | **AdminOpsApp** + Admin API keys | BoundAdmin / AdminOps | 中 |
| B4 | **PrefsApp** | BoundPrefs | 低 |

规则：**功能开发若落在未迁面，新代码只写进目标 App**，禁止回写 Bound。

### Phase C — Agent / Write 接线（约 3–5 人日）

| 步 | 内容 | 验收 |
|----|------|------|
| C1 | **AgentApp**：包装「发 chat / 建会话 / SSE」用例，内部仍用 UnifiedAgent + ToolCatalog | handler 变薄 |
| C2 | **WriteApp**：包装 `run_write_mode` / refine；**明确类型名**可逐步偏向 `WriteControl`（可选，低优） | 文档+测试：write_refine_* ∉ ToolCatalog |
| C3 | pipeline 只依赖 AgentApp / WriteApp，不直接 new 一堆 store | 编译边界清晰 |

**非目标：** 合并 Write 与 ReAct dispatch。

### Phase D — 拆除与收口（约 2–3 人日）

| 步 | 内容 | 验收 |
|----|------|------|
| D1 | 删除无引用 Bound 方法 / 文件 | 无 dead Bound |
| D2 | AppState 字段只保留 `Arc<*App>` + 必要 runtime | 方法数大幅下降 |
| D3 | file-size soft warn 消失或显著下降 | gate 脚本 |
| D4 | handoff「架构目标」改为 **Done**；更新 EXTENDING / ADR | 文档 |

---

## 5. 目录与命名建议（执行时二选一，默认 A）

### 方案 A（推荐先 A）：bootstrap 内 `product_apps/`

```text
app-bootstrap/src/
  product_apps/
    mod.rs
    workspace.rs    // WorkspaceApp
    share.rs
    billing.rs
    admin_ops.rs
    prefs.rs
    agent.rs        // 薄封装，重逻辑仍在 app-chat
    write.rs        // 薄封装，重逻辑在 write-core
  app_state/        // 逐渐变瘦
```

- **优点：** 迁移动作小、少动 Cargo workspace。  
- **缺点：** bootstrap 暂时仍偏「中心」。

### 方案 B（后期）：每域 crate 暴露 `*App`

```text
app-documents::WorkspaceApp
share::ShareApp 或 app-share::ShareApp
```

- 在 Phase D 或 B 中后期再拆 crate，避免前期过度工程。

---

## 6. 与测试金字塔的配合

| 变更类型 | 验证 |
|----------|------|
| 每个 App 切片 | `test-l1` + 该面 `cargo test -p …` |
| Share / Workspace HTTP | 现有 transport-http 相关测；PG 契约测需 honest bootstrap |
| Agent / Write | app-chat / write-core / agent-loop lib |
| 波次末 | L2 mock smoke；L3 短旅程（可选） |
| **不要求** | 每切片真 LLM / 全 Playwright |

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 半迁导致双 API（Bound + App）长期并存 | 切片内删调用点；Bound 方法 deprecated 最长 **一个波次** |
| 功能开发与迁移冲突 | T1：新功能只进 App；迁移切片穿插小功能 |
| 一次切太大 | Share 样板必须先绿；Workspace 再拆「CRUD / 上传」两个子切片若过大 |
| Write 被误并 | C2 测试锁 + EXTENDING 铁律 |
| Solo 精力 | 默认 **每波只完成 1 个 App 面**，不追求日历上「两周全完」 |

---

## 8. 成功图像

**开发新功能时：**

1. 找到产品面 → 对应 `*App`。  
2. 加用例方法 + ports。  
3. Handler 三行接线。  
4. `test-l1` + 定向测。  

**不再出现：**

- `AppState` 再涨 20 个业务方法。  
- 「改个分享要懂半个 chat」。  
- 把 `write_refine_*` 注册进 ToolCatalog。

---

## 9. 建议执行顺序（拍板后默认）

| 波次 | 交付 | 预估 |
|------|------|------|
| **W0** | 立宪（本文 + ADR 短文 + freeze） | 0.5d |
| **W1** | ShareApp 样板完整切片 | 2–3d |
| **W2** | WorkspaceApp（可再拆上传子切片） | 3–5d |
| **W3** | BillingApp + PrefsApp | 2–3d |
| **W4** | AdminOpsApp | 2–3d |
| **W5** | AgentApp + WriteApp 接线 | 3–5d |
| **W6** | 拆 Bound / 瘦 AppState / 文档 Done | 2–3d |

中间可随时插入 **纯产品功能**（走已迁 App 或 T1 新 App 方法）。

---

## 10. 决策清单（执行前勾选）

- [x] 接受 **绞杀者** 而非大爆炸  
- [x] 样板面选 **ShareApp**（若改 Workspace 先做，请改勾）  
- [x] Write **永久独立**（已口头确认）  
- [x] 目录先用 **方案 A**（bootstrap 内 product_apps）  
- [x] 每面单独可回滚 commit  
- [x] 日常验证仍 **L1 only**

---

## 11. 非目标

- 本计划内合并 Capability/Skill/Tool  
- Write 迁入 ToolCatalog  
- 为迁移恢复 PR 强制真 LLM / 全 Playwright  
- 一次 PR 删光 AppState  
- frontend_rust / archive  

---

## 12. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：Product App 迁移计划（用户确认要最佳实践架构、防债加速） |
| 2026-07-10 | 补全 ShareApp W1 方法清单；handoff 挂接下一主线 |
| 2026-07-10 | **W0 立宪落地**：ADR-0007 + 决策清单勾选；状态 In Progress |
| 2026-07-10 | **W0–W6 全部完成**：product_apps 绞杀 Bound；Write∉ToolCatalog；composition root |
