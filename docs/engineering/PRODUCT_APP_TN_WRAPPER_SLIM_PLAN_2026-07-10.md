# Product App TN 收口计划（删 execute 包装层 + 重复门禁）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done — S0–S5 complete** |
| 起因 | TN 复审 **CONDITIONAL APPROVE**：Phase B（R0–R5）入口/Write lane/工具真相已过关；残留为 **execute 路径包装过多、is_write 多层检查、SkillComponent 死 stub、chat() 半迁移** |
| 约束 | Solo local trunk；日常 **L1**；行为保持；**优先删概念**，不扩 CI |
| 上游 | [`PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md`](./PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md)（R0–R5 **Done**） |
| 评审 | TN review 复审 2026-07-10（Conditional Approve + P1 残留） |
| 铁律 | T1–T6；Write ∉ ReAct ToolCatalog；C4 不做；Transport 保持 **零 mode if** |

---

## 0. 诚实现状

| 已完成（R0–R5，保留） | **本计划要删/收口** |
|------------------------|---------------------|
| Transport → `conversation().execute[_stream]` | `ConversationApp` → `WriteApp`/`AgentApp` → `ChatContext` 的 **execute 透传链** |
| `PipelineLane::{Agent,Write}` + write pipeline | `is_write` 在 Conversation / WriteApp / AgentApp / pipeline / dispatch **重复** |
| write_refine 不在 SkillRegistry/Catalog | write_refine 仍实现完整 `SkillComponent` + 不可达 `execute` stub |
| 文档 Phase A+B Done | `chat()`/`docs()` deprecated 但生产+测试仍调用 |
| — | AppState 胖字段 composition **明确不在本计划**（见 §5 非目标 / 可选 S4） |

**一句话目标：**  
execute 路径 **只剩「Conversation 一次分 lane + pipeline 按 lane 跑」**；WriteApp/AgentApp 不再做 execute 门禁包装；write_refine 退化为 **纯 ToolSpec**；deprecated 调用清完或撤销。

---

## 1. 非目标

- 大爆炸 `Arc<*App>` composition root（可选 **S4**，默认可延后）  
- Write 并入 ToolCatalog / UnifiedAgent  
- C4；真 LLM / 全 Playwright  
- 重写 share/workspace/billing 业务逻辑  
- 为收口扩 CI  

---

## 2. 目标形状

```
Transport / MCP
    │  零 mode if（保持）
    ▼
ConversationApp::execute / execute_stream
    │  唯一 is_write → lane 决策
    ├─ Write  → ChatContext::execute_write[_stream]  → execute_write_pipeline  → run_write_mode
    └─ Agent  → ChatContext::execute_chat[_stream]   → execute_chat_pipeline   → dispatch_agent_mode
                 （agent 路径不再经 AgentApp::execute_*）

AgentApp   → 仅 sessions / search / citations / runtime_tools（非 execute）
WriteApp   → 可选删除 execute API，或仅保留将来 write 任务面；本计划默认 **去掉 execute***

write_refine → tool_specs_for_pool()  only（无 SkillComponent / 无 stub execute）
```

**Mode 检查预算（目标 ≤2）：**

| 层 | 是否检查 is_write |
|----|-------------------|
| ConversationApp | **是**（唯一产品路由） |
| PipelineLane 入口 | **可选防御** 一次（lane 与 agent_type 交叉）；或仅 `debug_assert` |
| WriteApp / AgentApp execute | **否**（无此 API 或无门禁） |
| dispatch_agent_mode Write arm | **unreachable / 内部错误**，非产品错误码分叉 |

---

## 3. 波次编排

### S0 — 文档与成功标准（0.25d）

| 步 | 内容 | 验收 |
|----|------|------|
| S0.1 | 本文锁定；handoff「下一主线」= 本计划 | 入口清晰 |
| S0.2 | 上一 TN plan 标注「残留 → 本计划」 | 无双 Done 冲突 |

---

### S1 — 压扁 execute 包装（**优先 code-judo**，1–2d）

| 步 | 内容 | 验收 |
|----|------|------|
| S1.1 | `ConversationApp::execute[_stream]` **直接**调 `chat.execute_write*` / `chat.execute_chat*`（或 `app_chat` 公开 pipeline 入口） | 调用链少一层 |
| S1.2 | **删除** `WriteApp::execute` / `execute_stream`（及测试改为 Conversation 或 ChatContext） | rg 无 WriteApp::execute |
| S1.3 | **删除** `AgentApp::execute_chat` / `execute_chat_stream` 门禁透传；若 MCP/测试仍用，改 Conversation 或 `chat.execute_*` | AgentApp 无 execute_* |
| S1.4 | 去掉 Conversation 内 `fn agent()` / `fn write()` 仅为 execute 的临时构造 | Conversation 更短 |
| S1.5 | 锁测：Conversation 空 query 仍走真实路径；transport 仍只 `conversation()` | L1 相关绿 |

**刻意不做：** 改 session CRUD 归属（仍可在 AgentApp）。

---

### S2 — Mode 检查只留一处（0.5–1d，可与 S1 同 PR）

| 步 | 内容 | 验收 |
|----|------|------|
| S2.1 | 产品路由只在 Conversation | 单点 |
| S2.2 | `run_pipeline`：lane 与 agent_type 交叉校验 **保留一次** 或改为 `debug_assert` + 日志（拍板默认：**保留一次** hard check，防直接调 pipeline） | 文档写明 |
| S2.3 | `dispatch_agent_mode` 的 `AgentKind::Write`：改为 `unreachable!` 或 `internal`「lane invariant」；错误码不再当产品 API | 无「假装可调用」的产品错误 |
| S2.4 | rg `use_write_entry` / `write_mode_required`：仅剩 pipeline 防御（若保留）或测试 | 计数 ≤ 约定 |

---

### S3 — write_refine 纯 ToolSpec（1d）

| 步 | 内容 | 验收 |
|----|------|------|
| S3.1 | `write_refine.rs`：删除 `SkillComponent` impl 与 `execute` stub；保留/整理 `all_tool_specs` + `tool_specs_for_pool` | 无 SkillComponent for write_refine |
| S3.2 | 现有 schema 单测改为针对 `ToolSpec` 构建函数 | 测绿 |
| S3.3 | `AppWriteRefineMode::tool_specs` 不变接口 | write refine 环行为不变 |
| S3.4 | 模块文档：唯一真相 = YAML pool + local ToolSpec | 无「仍是 Skill」叙事 |

---

### S4 — deprecated 清场（1–2d）

| 步 | 内容 | 验收 |
|----|------|------|
| S4.1 | 生产：`profile.rs` 等 `state.chat()` → `agent()` 或合适 product API（usage limit 若仅 ChatContext 有，可 `agent().chat()` **禁止**；优先在 AgentApp 加薄方法或 usage 面） | transport **零** `state.chat()` |
| S4.2 | 契约/集成测：`delegate_contract`、`api_key_security_contract`、auth tests → `agent()` / `conversation()` | 测试不教错 face |
| S4.3 | `docs()`：主调用改 `workspace()`；或 **撤销** deprecated 仅文档 Prefer（拍板默认：**改调用点**） | rg 生产+测试策略一致 |
| S4.4 | `write_app()`：无调用则删 alias；有则改 `write()` 或 Conversation | 无半 deprecated |
| S4.5 | 若某 API 半年内无法迁移：去掉 `#[deprecated]`，改文档 Prefer，避免警告噪音 | 二选一，禁止长期 deprecated+仍全量调用 |

---

### S5 — Composition 加深（**可选 / 默认可延后**）

| 步 | 内容 | 验收 |
|----|------|------|
| S5.1 | 仅当 S1–S4 稳定后：评估 `AppState` 产品路径只暴露 `*App`，infra `pub(crate)` | 非本波默认范围 |
| S5.2 | 真 `Arc` 装配另开波次 | 不阻塞 S1–S4 Done |

---

## 4. 建议顺序与预估

| 波次 | 交付 | 预估 | 阻塞 |
|------|------|------|------|
| **S0** | 文档挂接 | 0.25d | 无 |
| **S1** | 压扁 execute 包装 | 1–2d | 无 |
| **S2** | Mode 检查预算 | 0.5–1d | 宜紧随 S1 |
| **S3** | write_refine 纯 ToolSpec | 1d | 可与 S2 并行 |
| **S4** | deprecated 清场 | 1–2d | S1 后更顺 |
| **S5** | composition（可选） | 2–4d | **默认 defer** |

**默认交付范围：S0–S4。** S5 单独立项。

---

## 5. 验证

| 变更 | 验证 |
|------|------|
| 每波 | `cargo test -p app-bootstrap --lib`；`cargo test -p app-chat --lib`；触碰面 transport；**`bash scripts/test-l1.sh`** |
| S1 | rg：`WriteApp::execute`、`AgentApp::execute_chat` 无生产路径；transport 仅 `conversation()` |
| S2 | rg：`is_write_agent_type` / `use_write_entry` 出现次数符合预算 |
| S3 | rg：`impl SkillComponent for WriteRefine` 为零；write refine 相关测绿 |
| S4 | rg transport：`state.chat()` 为零（或仅 allow 列表为空） |
| 不要求 | 真 LLM / 全 Playwright |

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 删 WriteApp.execute 后测试只测 Conversation | 保留 Conversation + pipeline lane 测；行为不变 |
| 直接调 `execute_chat` 绕过 Conversation | pipeline lane 防御保留；文档禁止产品路径直调 |
| write_refine 去 Skill 后丢 schema 细节 | 先迁 ToolSpec 再删 impl；单测钉 schema |
| S4 改测面大 | 按 crate 分 commit：transport 生产 / app tests |

---

## 7. 决策清单（执行前勾选）

- [x] 接受 **S1 压扁 execute**（Conversation 直调 ChatContext pipeline）  
- [x] WriteApp/AgentApp **去掉 execute API**（推荐）  
- [x] Mode 检查：Conversation + pipeline 防御（默认）  
- [x] S3 write_refine **纯 ToolSpec**  
- [x] S4 **改调用点**而非长期 deprecated  
- [x] S5 composition **本轮不做**（默认）  
- [x] 日常仍 **L1 only**  

---

## 8. 与前序计划关系

| 计划 | 角色 |
|------|------|
| 架构迁移 W0–W6 | Phase A 命名 face |
| TN R0–R5 | Phase B 入口 / Write lane / 工具真相 — **Done** |
| **本文 S0–S4** | Phase C 删包装与重复门禁 — **当前 source of truth** |

---

## 9. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：TN Conditional Approve 残留 → S0–S5 收口编排 |
| 2026-07-10 | **S0–S5 执行启动** |
| 2026-07-10 | **S0–S5 执行完成**：execute 压扁；mode 预算；ToolSpec only；call-site 清场 |
