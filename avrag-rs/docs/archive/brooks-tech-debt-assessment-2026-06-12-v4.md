# Brooks-Lint Review — 技术债深度评估

**Mode:** Tech Debt Assessment
**Scope:** `avrag-rs`（34 workspace crate）+ `frontend_next` + `contracts` + `desktop`（全项目深度探测 v4，含新增 Tauri 桌面壳）
**Health Score:** 59/100
**Trend:** 34 → 58 → 70 → **59**（−11 vs v3,见下方口径说明）

**一句话结论：** v3 路线图大面积兑现——`mineru`/`notebooks`/`rag_prompts`/前端 chat hook 全部拆分完成、契约治理门禁健在；本次方法级深挖将 **ReAct loop 两个 450+ 行深嵌套方法** 升级为 Critical,并发现三个新热点(前端 7 份 fetch 包装重复、desktop IPC 第三份事件 schema、`llm/client.rs` 三合一)。

> **口径说明（重要）：** 分数下降 **不代表代码退化**。v3 对 ReAct loop 仅按文件行数定级(Warning);本次深入到方法级(单方法 ~465/~475 行、36% 行处于 ≥6 层缩进),按 Severity Guide(>50 行 + 嵌套 >5)升级为 Critical。**若按 v3 同口径,本次约为 69 分,与 v3 的 70 持平**——存量大额偿还与新热点出现大致相抵。
>
> **归档：**
> - v1 → [`archive/brooks-tech-debt-assessment-2026-06-12-v1.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v1.md)（Health 34）
> - v2 → [`archive/brooks-tech-debt-assessment-2026-06-12-v2.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v2.md)（Health 58）
> - v3 → [`archive/brooks-tech-debt-assessment-2026-06-12-v3.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v3.md)（Health 70）

---

## 1. 审计范围与方法

| 维度 | 说明 |
|------|------|
| Workspace | 34 crate(运行时循环依赖:**无**;`app-chat → app-bootstrap` 仅 dev-dep,合法) |
| 新增范围 | `desktop/` Tauri 2 桌面壳(src-tauri 230 行,阶段 0–2 进行中) |
| 六类衰减风险 | 全部启用(无 `.brooks-lint.yaml`) |
| 优先级公式 | Pain × Spread(各 1–3,最高 9);7–9 Critical debt / 4–6 Scheduled / 1–3 Monitored |
| 验证方式 | `cargo metadata` 依赖图 DFS、方法级行数与缩进深度统计、生产路径 `unwrap` 上下文核查 |

### 1.1 关键指标对比（v3 → v4）

| 指标 | v3 | v4(本次) | 变化 |
|------|-----|------------|------|
| `mineru.rs` | 1886 单文件 | **mineru/ 目录,最大 v4.rs 439** | ✅ 拆分完成 |
| `handlers/notebooks.rs` | 924 单文件 | **notebooks/ 目录,最大 share.rs 389** | ✅ 拆分完成 |
| `rag_prompts.rs` | 1739 单文件 | **prompts/ 目录,最大 plan.rs 267;rag_prompts.rs 仅 3 行** | ✅ 拆分完成 |
| `use-chat-stream.ts` | 518 | **232** + stream-{event-handlers,assistant-updates,typewriter} 三模块 | ✅ 拆分完成 |
| `settings-share-messages.ts` | 725 平行 i18n | **14 行 deprecated shim**(已并入 `lib/i18n/messages/`) | ✅ 合并完成 |
| Plus 用量倍数文案 | 6× vs 10× 矛盾 | **6× = 5h 窗口、10× = 7d 窗口**,注释指向 planLimits.ts | ✅ 统一 |
| `UserTier` 过渡别名 | 残留 | **0 处** | ✅ 移除 |
| eval framework | 1633 行在主树 | `eval/framework.rs` + **`#![cfg(feature = "eval")]` 门控** | ✅ 编译隔离 |
| `common` contracts re-export | 大量 | **0 处**;normal fan-in 24 → **22** | ✅ 分层落实 |
| storage-pg 运行时直连 | 8+ crate | **4 个 domain crate**(app/app-chat 退为 dev-dep,admin 退出) | ✅ 收敛中 |
| contracts golden fixtures | 仅 chat | + **notebook/billing/admin** roundtrip(`module_fixtures.rs`) | ✅ 扩展 |
| CI 契约漂移门禁 | frontend-unit.yml | 健在;`check_contract_governance.sh` 健在 | ✅ 维持 |
| ReAct loop 目录 | 6009 行 | **6060 行** | ❌ 三轮唯一未动项 |
| 生产 TODO/FIXME | — | **3 处**(全项目,极干净) | ✅ |

---

## 2. Findings

### 🔴 Critical

**Cognitive Overload — ReAct loop 两个 450+ 行、6 层嵌套的核心方法**

Symptom: `agents/loop/mod.rs` 的 `run()` 占 **125–589 行(~465 行)**,其中 89 行处于 ≥6 层缩进;`agents/loop/iteration.rs` 的 `dispatch_skill_tool()` 占 **78–553 行(~475 行)**,其中 **172 行(36%)处于 ≥6 层缩进**。loop 目录合计 6060 行,v1→v4 四轮评估期间其他热点全部拆完,唯独此处未动。抽样确认内部是 loop-within-match-within-if 的真实深嵌套,非线性代码。
Source: Fowler — *Refactoring*, Long Method; McConnell — *Code Complete*, Ch. 7: High-Quality Routines; Ousterhout — *A Philosophy of Software Design*, Deep vs Shallow Modules
Consequence: 这是所有 agent 行为的主执行路径——每次调整 exit policy、disclosure、synthesis gate 都要在 450 行方法内定位上下文;新人无法在工作记忆内装下完整控制流;修改时回归风险集中。policy/ 子目录已就位但主方法未受益。
Remedy: 不动行为,按阶段提取:`run()` 内的 budget 检查、turn-end 遥测、auto-fallback 触发各提取为私有方法(每个 <60 行);`dispatch_skill_tool()` 按 skill 类型分发表拆为 `dispatch_codegen` / `dispatch_search` / `dispatch_native` 等子函数。现有 loop 测试(mod.rs/iteration.rs 内嵌 20+ 用例)是安全网,先跑通再提取。
Priority: Pain 3 × Spread 2 = **6**(Scheduled) | Intent: **[accidental]**

---

### 🟡 Warning

**Knowledge Duplication — 前端 7 个 `client.ts` 各自手写 fetch + Authorization 包装**

Symptom: `lib/{workspace,admin,share,settings,auth,dashboard,api-access}/client.ts` 共 **2288 行**,每个文件独立实现 `headers.set("Authorization", Bearer ...)` + `await fetch(buildApiUrl(path))` + 错误处理;`admin/client.ts` 内部就有两份(191 行、211 行);无共享 http helper。
Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY; Fowler — *Refactoring*, Duplicate Code
Consequence: auth 注入、错误语义、重试策略是同一个决策的 7 份拷贝;desktop IPC 接缝(`transport.ts` 的 `restRequest`)落地时,每个 client 都要单独分叉——重复在桌面化阶段会指数放大。
Remedy: 提取 `lib/http/request.ts`(注入 token、buildApiUrl、统一错误归一),7 个 client 改为薄域层;与 desktop `restRequest` 共用同一入口,IPC/HTTP 分叉只发生一次。
Priority: Pain 2 × Spread 3 = **6**(Scheduled) | Intent: **[accidental]**

**Knowledge Duplication — desktop IPC 手写第三份 chat 事件 schema**

Symptom: `desktop/src-tauri/src/lib.rs` 的 `chat_stream` 手写 emit `{kind:"start"|"token"|"done"}` JSON;`tauri-ipc.ts` 以 `e.payload as WorkspaceChatStreamEvent` 无校验 cast。Web 链路是 contracts `ChatEvent`(generated)→ `WireToWorkspace` 派生;桌面链路完全绕过 codegen,同一聊天流事件决策现有 **三处表达**(contracts、stream.ts 映射、desktop 手写)。
Source: Ousterhout — *A Philosophy of Software Design*, Information Leakage; Hunt & Thomas — DRY
Consequence: `ChatEvent` 增加事件类型(如 citations、reasoning)时桌面端静默落后;cast 无运行时校验,schema 漂移只能在运行期发现。当前是阶段 2 占位(`TODO: 接入真正的 LLM 调用`),但 emit 格式一旦被前端依赖就会按 Hyrum's Law 固化。
Remedy: desktop 已依赖 `common`;让 emit 端直接序列化 contracts `ChatEvent`(serde 输出与 SSE wire 一致),前端 IPC 路径复用 `parseWireChatEvent`,在阶段 2 完成前收口。
Priority: Pain 2 × Spread 2 = **4**(Scheduled) | Intent: **[intentional]**(路线图阶段 2,有 desktop/AGENTS.md 文档与 owner)

**Cognitive Overload — `chat_private.rs` 1122 行:记忆画像 JSON 操纵函数群 + 生产路径 unwrap**

Symptom: `build_rag_session_context` 占 28–221 行(~193 行);`apply_slot_updates`(~127 行)/`apply_singleton_update`(~100 行)/`apply_hint_updates` 等函数群直接操纵 `serde_json::Value`,测试模块之前有 **8 处 `.unwrap()`**(如 `profile.as_object_mut().unwrap()`),而 profile delta 来源于 **LLM 输出**(不可信输入)。
Source: McConnell — *Code Complete*, defensive programming at boundaries; Fowler — *Refactoring*, Long Method
Consequence: LLM 返回畸形 JSON(profile 非 object、slot 非 array)时聊天主路径 panic;JSON 形状假设散落在各函数,无单点 schema 校验。
Remedy: 入口处用 typed struct(serde Deserialize + default)替代裸 Value 操纵,或在 `parse_structured_json_response` 后增加形状归一化,使后续 `unwrap` 不可达;`build_rag_session_context` 按 memory/quota/visibility 三段提取。
Priority: Pain 2 × Spread 2 = **4**(Scheduled) | Intent: **[accidental]**

**Cognitive Overload — `llm/client.rs` 1262 行:流解析器 + 限流 + 三个 complete 方法同文件**

Symptom: 单文件含 SSE `ChatCompletionStreamParser`(~160 行)、rate-limit/usage 记账、`complete_with_tools`(~190 行)/`complete`(~130 行)/`complete_stream`(~140 行)三个请求方法及多 provider 适配(DeepSeek thinking 字段映射等);生产代码约 940 行。
Source: Fowler — *Refactoring*, Long Method / Divergent Change; Martin — *Clean Architecture*, SRP
Consequence: 新增 provider 或调整流式协议时,三个 complete 方法需同步检查;解析器与传输逻辑耦合在一处,文件因多个不同原因被修改。
Remedy: 拆 `stream_parser.rs`(已有完善内嵌测试可随迁)、`rate_limit.rs`;三个 complete 方法收敛共用 request-build/usage-record 路径。
Priority: Pain 2 × Spread 2 = **4**(Scheduled) | Intent: **[accidental]**

---

### 🟢 Suggestion

**Dependency Disorder — storage-pg 运行时直连剩 4 个 domain crate(收敛趋势良好)**

Symptom: `cargo metadata` 确认 normal 依赖仅剩 `app-admin`、`billing`、`chatmemory`、`share`(+ 合法的 `app-bootstrap`/`worker`);`app`、`app-chat` 已退为 dev-dep,`admin` 退出。
Source: Martin — *Clean Architecture*, DIP
Consequence: 4 个域 crate 的 schema 耦合仍在,但爆炸半径较 v3 显著缩小。
Remedy: 按 app-chat 模式经 repository port 注入,逐个收口。
Priority: Pain 1 × Spread 2 = **2**(Monitored) | Intent: **[intentional]**(收敛路径已被验证)

**Change Propagation — `stream.ts` kind 映射层 ~200 行手写解析(v3 残留)**

Symptom: `WorkspaceChatStreamEvent` 已从 generated `ChatEvent` 派生,但 `parseWireChatEvent`/`chatEventToWorkspace` 仍 ~200 行手写(event→kind 转换 + runtime 窄化)。
Source: Ousterhout — Information Leakage
Consequence: 协议字段变更时映射层可能 drift;DTO 已统一,风险有限。
Remedy: 评估 reducer 直接消费 `ChatEvent.event`,删除 kind 层——若 desktop IPC 收口方案落地,此层可一并消失。
Priority: Pain 1 × Spread 2 = **2**(Monitored) | Intent: **[intentional]** UI 适配层

**Cognitive Overload — `storage-pg/repository_retrieval.rs` 1222 行 25+ 方法混域**

Symptom: 单 impl block 混合 chunk 检索(text/bm25)、document 生命周期(status/upload 校验)、清理任务(cleanup lease/derived rows)三个子域。
Source: Fowler — *Refactoring*, Divergent Change
Consequence: repository 模式下文件偏大可接受,但三个变更原因共存一文件。
Remedy: 按 retrieval / document-lifecycle / cleanup 拆三个 impl 文件(Rust 允许跨文件 impl 同一类型)。
Priority: Pain 1 × Spread 2 = **2**(Monitored) | Intent: **[accidental]**

**Cognitive Overload — `transport-http/auth_secondary.rs` 1040 行 25 个 handler**

Symptom: profile、preferences、agent-preferences、password-reset 四个子域的 handler 同文件;单个 handler 结构清晰(40–90 行)。
Source: McConnell — *Code Complete*, Ch. 7
Consequence: 文件级导航成本;merge conflict 面偏大。
Remedy: 按子域拆 `auth/{profile,preferences,reset}.rs`,参照 notebooks/ 拆分先例。
Priority: Pain 1 × Spread 1 = **1**(Monitored) | Intent: **[accidental]**

**Knowledge Duplication — `settings-share-messages.ts` shim 的 10 个调用方未迁移**

Symptom: 文件已减至 14 行 `@deprecated` 转发层,但 share/settings 下 10 个文件仍 import 旧路径。
Source: Fowler — *Refactoring*, 渐进迁移收尾
Consequence: shim 长期滞留会变成永久转发层(Middle Man)。
Remedy: 一次 codemod 把 import 切到 `lib/i18n/messages`,删除 shim。
Priority: Pain 1 × Spread 1 = **1**(Monitored) | Intent: **[intentional]**(迁移收尾)

**Cognitive Overload — `use-workspace-context-rail.ts` 750 行 / 39 个 hook 调用**

Symptom: 单 hook 文件含 39 个 useState/useEffect/useCallback,状态管理密集。
Source: Fowler — *Refactoring*, Long Method(hook 形态)
Consequence: context rail 行为变更时状态交互难追踪。
Remedy: 按 selection/filter/expansion 等关注点拆子 hook,参照 chat-session/ 拆分先例。
Priority: Pain 1 × Spread 1 = **1**(Monitored) | Intent: **[accidental]**

---

## 3. Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|-------------|----------------|--------|
| Cognitive Overload | 5 | 3.4 | Scheduled | accidental |
| Knowledge Duplication | 3 | 3.7 | Scheduled | mixed |
| Change Propagation | 1 | 2.0 | Monitored | intentional |
| Dependency Disorder | 1 | 2.0 | Monitored | intentional |
| Accidental Complexity | 0 | — | **Clean**(eval 已门控,无投机抽象) | — |
| Domain Model Distortion | 0 | — | **Clean**(UserTier/BillingTier 统一完成) | — |

**Recommended focus:** Cognitive Overload(ReAct loop 方法级拆分——三轮未动的最后堡垒)→ Knowledge Duplication(前端统一 http helper + desktop 事件 schema 在固化前收口)

**系统性判读:** 六风险中两类已清零,债务高度集中在「方法级认知负载」与「桌面化引入的接缝重复」。前者是存量(loop),后者是增量(desktop 阶段 0–2)——增量债当前全部 intentional 且有路线图,关键是阶段 2 完成前不让占位 schema 固化。

---

## 4. 通往 100 分的偿还路线图(v4 更新)

### 4.0 已完成 ✅(v3 路线图 Phase 3 全部 + Phase 4 大部分)

| 任务 | 验收 |
|------|------|
| `mineru.rs` 按解析阶段拆分 | mineru/ 目录,最大 439 行 |
| `notebooks.rs` 再拆 | notebooks/{share,notes,analysis,crud}.rs |
| `rag_prompts.rs` 外置 | prompts/ 目录,最大 267 行 |
| 合并 `settings-share-messages.ts` | 14 行 shim |
| 统一 Plus 倍数文案 | 窗口语义标注 |
| `common` 移除 contracts re-export | 0 处 |
| eval framework 编译隔离 | `#![cfg(feature = "eval")]` |
| `UserTier` 别名移除 | 0 处 |
| contracts fixtures 扩展 | notebook/billing/admin roundtrip |
| `use-chat-stream` 按 event 拆 reducer | 232 行 + 3 模块 |

### 4.1 计分模型:59 → 100 的差额构成

当前 59 分,距满分 41 分,正好对应 11 项未清问题:

| 档位 | 数量 | 单项分值 | 合计 | 清完后累计 |
|------|------|---------|------|-----------|
| 🔴 Critical | 1 | +15 | +15 | 59 → **74** |
| 🟡 Warning | 4 | +5 | +20 | 74 → **94** |
| 🟢 Suggestion | 6 | +1 | +6 | 94 → **100** |

> 注:100 分的含义是"本轮扫描口径下零发现",不是代码从此完美。桌面端阶段 2–4 会持续进新代码,分数需要靠下方"维持机制"保住。

### 4.2 第一档 — Critical,+15 分(59 → 74)

| # | 任务 | 做法 | 验收 |
|---|------|------|------|
| 1 | 拆掉 ReAct loop 两个超长函数 | `run()`(~465 行)内的预算检查、turn-end 遥测、auto-fallback 触发各提取为私有方法;`dispatch_skill_tool()`(~475 行)按 skill 类型拆为 `dispatch_codegen`/`dispatch_search`/`dispatch_native` 等分发函数。**不改任何行为**,现有 20+ 内嵌测试是安全网,拆一步跑一次 | 单方法 <150 行;≥6 层缩进行数减半;`cargo test -p app-chat` 全绿 |

工作量最大、价值也最大的一项:这是所有 agent 行为的主路径,四轮评估唯一没动过的地方。

### 4.3 第二档 — Warning,共 +20 分(74 → 94)

| # | 任务 | 做法 | 验收 |
|---|------|------|------|
| 2 | 前端统一网络请求 | 新建 `lib/http/request.ts`(token 注入、buildApiUrl、错误归一),7 个 `client.ts` 改为调用它 | 7 个 client 无独立 fetch+auth 代码;`pnpm typecheck` + Vitest 全绿 |
| 3 | 桌面端消息格式对齐 | desktop emit 端直接序列化 contracts `ChatEvent`(desktop 已依赖 `common`,零新增依赖);`tauri-ipc.ts` 复用 `parseWireChatEvent`,删除 `as` 裸转 | IPC payload 与 SSE wire 同源;chat 事件 schema 表达点 3 → 1 |
| 4 | 记忆画像入口 typed 化 | `chat_private.rs` 的 profile delta 在 `parse_structured_json_response` 后转 typed struct(serde + default),后续 8 处 `unwrap` 不可达后删除;`build_rag_session_context` 按 memory/quota/visibility 三段提取 | 生产路径 0 `unwrap`;畸形 LLM 输出不再 panic |
| 5 | 大模型客户端拆文件 | `llm/client.rs` 拆出 `stream_parser.rs`(内嵌测试随迁)、`rate_limit.rs`;三个 `complete_*` 方法收敛共用 request-build/usage-record | 单文件 <600 行;`cargo test -p avrag-llm` 全绿 |

**第 3 项有时间窗口**:趁桌面端 `chat_stream` 还是占位代码改起来几乎零成本;等真接上 LLM 后端再改就要兼容旧格式(Hyrum's Law 固化)。

### 4.4 第三档 — Suggestion,共 +6 分(94 → 100)

| # | 任务 | 做法 | 验收 |
|---|------|------|------|
| 6 | storage-pg 剩余直连收口 | `app-admin`/`billing`/`chatmemory`/`share` 4 个 crate 改经 repository port 注入(app-chat 已有成例可照抄) | `cargo metadata` 中 storage-pg normal 依赖仅剩 bootstrap/worker |
| 7 | 删除 stream.ts kind 转换层 | reducer 直接消费 `ChatEvent.event`;与第 3 项联动,做完 3 后此层基本自动可删 | `stream.ts` 手写解析 ~200 行清除 |
| 8 | 数据库检索文件按域拆分 | `repository_retrieval.rs` 按 retrieval / document-lifecycle / cleanup 拆三个 impl 文件(Rust 允许跨文件 impl 同一类型) | 单文件 <600 行 |
| 9 | 登录辅助接口按子域拆分 | `auth_secondary.rs` 拆 `auth/{profile,preferences,reset}.rs`,参照 notebooks/ 先例 | 单文件 <500 行 |
| 10 | 删除文案转发垫片 | 10 个调用方 import 从 `settings-share-messages` 切到 `lib/i18n/messages`(一次 codemod),删除 14 行 shim | 文件删除;`pnpm typecheck` 全绿 |
| 11 | 工作台侧栏状态拆分 | `use-workspace-context-rail.ts`(750 行 / 39 hooks)按 selection/filter/expansion 拆子 hook,参照 chat-session/ 先例 | 单 hook 文件 <300 行 |

### 4.5 建议执行顺序

```
快赢热身:  #2 前端统一请求  →  #10 删垫片        (低风险,1 天内)
时间窗口:  #3 桌面消息格式对齐                    (趁占位未固化)
主攻坚:    #1 loop 拆分                          (慢慢拆,每步跑测试)
收尾警告:  #4 画像 typed 化  →  #5 LLM 客户端拆分
建议清零:  #7(与 #3 联动) → #6 → #8 → #9 → #11
```

每完成一批跑一次集成门禁,全绿再继续;全部完成后重跑 Brooks-Lint 复测确认 100。

### 4.6 维持机制(防止得而复失)

- **桌面端节奏依赖**:第 3、7 两项的最佳时机取决于桌面端路线图;若近期不动桌面端,至少先把消息格式写进 `desktop/AGENTS.md` 设计约定,防止占位写法被悄悄固化。
- **新代码门禁**:桌面端阶段 2–4 的新增 Rust/TS 代码沿用现有 CI 契约漂移检查;新文件超过 500 行、新函数超过 150 行时在 PR review 中拦截。
- **定期复测**:每完成一个 Phase 跑一轮 Brooks-Lint,趋势记录在 `.brooks-lint-history.json`。

### 集成门禁

```bash
cd avrag-rs && cargo test -p app-chat -p transport-http -p avrag-llm
cd contracts && cargo test
cd frontend_next && pnpm check:contracts-drift && pnpm typecheck
./scripts/check_contract_governance.sh
```

---

## 5. 预期成果

| 维度 | v4 | 第一档完成 | 第一+二档完成 | 全部清零 |
|------|-----|-----------|--------------|---------|
| Health Score | 59 | **74** | **94** | **100** |
| Critical | 1 | 0 | 0 | 0 |
| Warning | 4 | 4 | 0 | 0 |
| Suggestion | 6 | 6 | 6 | 0 |
| 最大单方法 | ~475 行 | <150 行 | <150 行 | <150 行 |
| chat 事件 schema 表达点 | 3 | 3 | 1(contracts 单源) | 1 |
| 生产路径 unwrap(画像链路) | 8 | 8 | 0 | 0 |

---

## 6. 附录:关键文件索引

| 路径 | 行数 | 说明 |
|------|------|------|
| `app-chat/src/agents/loop/mod.rs` | 1091(`run()` ~465) | 🔴 主病灶 |
| `app-chat/src/agents/loop/iteration.rs` | 1009(`dispatch_skill_tool` ~475) | 🔴 主病灶 |
| `app-chat/src/chat_private.rs` | 1122 | 🟡 profile JSON + unwrap |
| `llm/src/client.rs` | 1262 | 🟡 三合一 |
| `frontend_next/lib/*/client.ts` × 7 | 2288 合计 | 🟡 fetch 包装重复 |
| `desktop/src-tauri/src/lib.rs` | 221 | 🟡 手写事件 schema(占位) |
| `storage-pg/src/lib_impl/repository_retrieval.rs` | 1222 | 🟢 混域 |
| `transport-http/src/lib_impl/auth_secondary.rs` | 1040 | 🟢 多子域 |
| `frontend_next/.../use-workspace-context-rail.ts` | 750 | 🟢 39 hooks |
| `frontend_next/lib/workspace/stream.ts` | 506 | 🟢 kind 层残留 |
| `ingestion/src/parser/mineru/` | 最大 439 | ✅ 已拆分 |
| `transport-http/src/handlers/notebooks/` | 最大 389 | ✅ 已拆分 |
| `app-chat/src/prompts/` | 最大 267 | ✅ 已拆分 |

---

*生成工具:Brooks-Lint Tech Debt Assessment · 2026-06-12 v4(深入探测,方法级口径)*
