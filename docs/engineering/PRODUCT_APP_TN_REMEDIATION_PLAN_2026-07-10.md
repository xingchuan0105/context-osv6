# Product App TN 修复计划（删复杂度，不是再贴一层）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done — R0–R5 complete**；残留收口见 [`PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md`](./PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md) |
| 起因 | Thermo-Nuclear review：W0–W6 完成的是 **命名 + 入口门禁**，非 ADR-0007 终态；薄包装 / 双分发 / Write 三真相源 |
| 约束 | Solo local trunk；日常 **L1**；行为保持；**优先删概念**，不扩 CI |
| 上游 | [`PRODUCT_APP_ARCHITECTURE_MIGRATION_PLAN_2026-07-10.md`](./PRODUCT_APP_ARCHITECTURE_MIGRATION_PLAN_2026-07-10.md)（结构迁移初稿） |
| 评审 | TN review 2026-07-10（FAIL / REQUEST CHANGES 结构） |
| 铁律延续 | T1–T6；Write ∉ ReAct ToolCatalog；C4 不做 |

---

## 0. 诚实现状（与「Done」对齐）

| 已完成（保留） | **未**完成（本计划） |
|----------------|----------------------|
| Bound 目录拆除；`product_apps/*` 命名 | AppState 仍是胖上下文字段，非 `Arc<*App>` composition root |
| Transport 部分走 `agent()` / `write_app()` | AgentApp/WriteApp 大面积 `ChatContext` 透传 |
| Catalog skip `write_refine_*` + 锁测 | Write 仍进 `execute_chat` → pipeline Write arm |
| ADR-0007 方向正确 | `write_control_tool_meta` + SkillRegistry 双轨；transport 三处 write if |

**本计划目标一句话：**  
把「半独立 + 双分发 + 旁路 meta」收成 **单一会话入口、Write 真出 chat 执行管道、工具单一真相**；文档与代码同口径。

---

## 1. 非目标

- 大爆炸拆 crate / 一次 PR 删光 AppState 字段  
- Write 并入 ToolCatalog / UnifiedAgent ReAct  
- C4（Capability/Skill/Tool 合并）  
- 强制真 LLM / 全 Playwright  
- frontend_rust / 无关产品功能  

---

## 2. 目标形状（修复后）

```
Transport / MCP
    │  零 mode if（不在 handler 里写 if write）
    ▼
ConversationApp::execute / execute_stream   ← 唯一会话执行入口
    │
    ├─ chat | rag | search  →  domain agent path（现有 UnifiedAgent + ToolCatalog）
    └─ write                →  WriteApp / writer 公开 API（不经 execute_chat 的 write arm）

AppState（阶段 R 收口）
    产品路径只暴露 *App 访问器；raw chat()/documents 仅 bootstrap/tests 或 deprecated
```

Write 工具：

```
SkillRegistry / ToolCatalog / Capabilities product union  →  无 write_refine_*
write-core + modes/write_refine.yaml（或本地 ToolSpec） →  唯一披露与执行定义
```

---

## 3. 波次编排（默认顺序）

### R0 — 文档诚实化（0.5d）

| 步 | 内容 | 验收 |
|----|------|------|
| R0.1 | 迁移计划状态改为：**Phase A（命名+门禁）Done；终态未完成** | plan/handoff 无 overclaim |
| R0.2 | ADR-0007 补 **Implementation status**：Phase A shipped；Phase B = 本计划 | 读者不被「Done」误导 |
| R0.3 | Handoff「下一主线」= 本 TN 修复计划 | 入口清晰 |

**不写代码。** 可与 R1 同 commit。

---

### R1 — 单一会话入口（删 transport 双分发）**【优先 code-judo】**

| 步 | 内容 | 验收 |
|----|------|------|
| R1.1 | 新增 `ConversationApp`（或等价名）：`execute` / `execute_stream` **内部一次** `match agent_type` | 唯一分发点 |
| R1.2 | chat/rag/search → 现有 agent 路径；write → Write 路径（本波可暂仍调 `execute_chat`，R2 再拔） | 行为不变 |
| R1.3 | `handlers/chat.rs` POST + SSE、MCP query：**只调** `state.conversation().…` | **零** `if is_write` |
| R1.4 | 删除 `AgentApp`/`WriteApp` 上互斥 gate 的 **重复**（保留 Conversation 内一次）；`is_write_agent_type` **单一定义** | 无双份 helper |
| R1.5 | 锁测：handler 源码/静态 rg 无 write 分支；conversation empty-query 仍走真实路径 | L1 相关绿 |

**刻意不做：** 本波不要求 Write 已出 pipeline（避免一刀太大）。

**成功图像：** handler 变薄；概念从「两 App 互斥 + 三处 if」变成「一个 Conversation 入口」。

---

### R2 — Write 真出 `execute_chat` 管道

| 步 | 内容 | 验收 |
|----|------|------|
| R2.1 | 公开稳定入口：`app_chat` 或 `WriteApp` 调 `run_write_mode`（及 stream 等价），**不**经 `ChatContext::execute_chat` 的 write arm | WriteApp 方法体直达 writer |
| R2.2 | `pipeline_steps::dispatch_mode` 的 `AgentKind::Write` arm：改为 `unreachable`/返回明确 internal error，或删除并保证无调用方 | rg 无「execute_chat 吞 write」 |
| R2.3 | Conversation 的 write 分支只调 R2.1 | 集成路径一条 |
| R2.4 | 测：Write 空 query / 模式校验走 **Write 入口**；chat 空 query 不进 Write | 定向 + L1 |

**风险：** stream 与 preflight/session 解析目前与 chat pipeline 共享 — 允许 Write 入口 **复用** preflight/session 辅助函数，但 **不**再走 `dispatch_mode` Write arm。

---

### R3 — Write 工具单一真相（删旁路）

| 步 | 内容 | 验收 |
|----|------|------|
| R3.1 | `write_refine_*` **不再** `SkillRegistry::register`（或 register 但不进任何 ReAct/Capabilities 路径且无 stub 必要） | 优先：完全不注册 |
| R3.2 | 删除 `write_control_tool_meta` 及 `CapabilityRegistry::tool` 旁路 | registry 无 write 特殊分支 |
| R3.3 | Write ModeHost / tools_for_retrieve：从 YAML + **write-local ToolSpec** 构建（不依赖 ToolCatalog） | write refine 环仍能拿到 tool specs |
| R3.4 | 保留并收紧：`write_refine_* ∉ ToolCatalog`；product_mode 的 chat/rag/search pool 无 write_refine | 现有 + 新测 |
| R3.5 | Capabilities：write 模式披露策略写清（要么 write 模式单独列表，要么不进 product union）— **拍板默认：write 披露不经 ToolCatalog** | 文档 + 测 |

---

### R4 — API 收口与命名（半迁移清扫）

| 步 | 内容 | 验收 |
|----|------|------|
| R4.1 | `AppState::chat()`：产品路径 `#[deprecated]` 或 `pub(crate)`；测试改 `agent()` / `conversation()` | 生产 transport 无 `state.chat()` |
| R4.2 | 统一命名（一次做完，deprecate 旧名）：`write()` 替代 `write_app()`；`billing()` 评估替代 `billing_api()`；`docs()` → 仅 alias 标 deprecated，主推 `workspace()` | 文档一表 |
| R4.3 | 删除 Agent/Write 上未使用的 `auth` 字段（若仍无用）；删 `WRITE_REFINE_OUTSIDE_TOOL_CATALOG` 装饰常量，靠结构测 | 无死字段 |
| R4.4 | `delegate_contract` / security contract 等测试改新入口 | 测试不教错 face |

---

### R5 — Composition 加深（可选，可拆下波次）

| 步 | 内容 | 验收 |
|----|------|------|
| R5.1 | AppState **产品对外**只保留 `*App` 访问器；infra 字段 `pub(crate)` 且文档标明非产品 API | 新功能无法「顺手加 AppState 方法」 |
| R5.2 | （可选）长期 owned `Arc` deps 进 App，lifetime face 变薄 | 编译边界清晰 |
| R5.3 | 迁移计划终态勾选；ADR-0007 Implementation = **Phase B Done** | 与代码一致 |

**Solo 默认：** R5 可在 R1–R4 稳定后再做，**不阻塞** R1–R4 宣称「TN 结构 FAIL 项已关」。

---

## 4. 建议执行顺序与预估

| 波次 | 交付 | 预估 | 阻塞关系 |
|------|------|------|----------|
| **R0** | 文档诚实化 | 0.5d | 无 |
| **R1** | Conversation 单入口 | 1–2d | 无 |
| **R2** | Write 出 execute_chat | 2–3d | 依赖 R1 更干净 |
| **R3** | 工具单一真相 | 1–2d | 可与 R2 并行思路，落地宜在 R2 后 |
| **R4** | API/命名收口 | 1–2d | R1 后 |
| **R5** | 真 composition（可选） | 2–4d | R1–R4 后 |

中间可插产品功能：**新功能只进目标 App / Conversation**，禁止回写 `state.chat().execute_*`。

---

## 5. 验证（每波 + 收口）

| 变更 | 验证 |
|------|------|
| 每波 | `cargo test -p app-bootstrap --lib`；触碰面 `cargo test -p transport-http --lib` 或定向；**`bash scripts/test-l1.sh`** |
| R1 | rg transport：`is_write_agent_type` / `if write` 仅存在于 Conversation 内（若有） |
| R2 | rg：`dispatch_mode` 无 Write 成功路径；Write 入口单测 |
| R3 | `write_refine_not_in_react_tool_catalog`；无 `write_control_tool_meta`；write refine lib 测绿 |
| R4 | rg 生产：`state.chat()` 仅 deprecated 或零 |
| 不要求 | 每波真 LLM / 全 Playwright |

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| R2 拆 pipeline 漏 preflight/session | Write 入口显式复用同一 preflight/session 辅助；对照现网 stream 契约测 |
| R3 去掉 SkillRegistry 后 ModeHost 无 schema | 先抽 ToolSpec 构建函数再删 register |
| 命名 deprecate 编译噪音 | 同波改完调用点，不留一波 deprecated |
| 范围膨胀 | **默认只做到 R4**；R5 单独立项 |

---

## 7. 决策清单（执行前勾选）

- [x] 接受 **现状诚实化**（迁移计划不是终态 Done）  
- [x] R1 采用 **ConversationApp 单入口**（推荐；否决则写替代）  
- [x] R2 **Write 必须出** `execute_chat` write arm（推荐；若只做门禁则降级为 defer）  
- [x] R3 **write_refine 不进 SkillRegistry+Catalog**（推荐）  
- [x] R4 命名：`write()` / deprecate `docs()`+`write_app()`  
- [x] R5 composition 加深：**本轮可选 / 下轮**（默认下轮）  
- [x] 日常验证仍 **L1 only**  

---

## 8. 与原迁移计划的关系

| 原计划声称 | 本计划修正 |
|------------|------------|
| W0–W6 **Done** = 终态 | 改为 **Phase A Done**；终态 = R1–R4（+可选 R5） |
| Product Apps 已是厚用例层 | Phase A = 命名 face；R 波次加厚真正入口与 Write 边界 |
| write_refine ∉ ToolCatalog | 保留，并消灭旁路 meta / 错误注册 |

原文件保留作历史；**执行以本文为 source of truth**，直到状态改为 Done。

---

## 9. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：TN review FAIL 项 → R0–R5 修复编排 |
| 2026-07-10 | **R0–R5 执行启动** |
| 2026-07-10 | **R0–R5 执行完成**：Conversation 单入口；Write 出 agent lane；工具单一真相；API 收口 |
