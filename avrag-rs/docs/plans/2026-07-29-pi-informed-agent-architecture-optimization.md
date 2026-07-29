# pi 最佳实践对照：Agent 架构代码现实与优化建议

| 项目 | 内容 |
|------|------|
| 状态 | **Wave A 落地中**（A0–A6 代码/文档；见 §10 修订记录） |
| 日期 | 2026-07-29 |
| 对照源 | [pi 的设计艺术](https://zhanghandong.github.io/pi-book/)（pi-mono v0.82.1 基线） |
| 代码基线 | `avrag-rs` master 本地 trunk（含 orchestrator V2、证据平面、通道持久 worker） |
| 关联 | ADR-0006-revised、ADR-0007、ADR-0008、Product Apps ADR-0007、`agent-loop/EXTENDING.md`、`docs/engineering/2026-07-28-handover.md` |
| 非目标 | 把产品改造成 coding CLI；削弱证据/引用/Write 旁路；大爆炸重写 |

---

## 0. 一句话结论

我们**已经吸收了 pi 的主轴**（LLM-driven ReAct、Tool/Skill 分层、少原生 schema、渐进披露），并在产品侧**正确加厚**了证据平面、Synthesis 契约、Write 旁路与 multi-worker。

架构债不在「不够像 pi」，而在：

1. **循环引擎偏胖**（领域依赖与产品解析塞进 `ReActLoop`）
2. **扩展钩子未长成协议**（`LoopHooks` 名义有、注入面几乎死）
3. **「Skill」一词三义**（可执行组件 / MD 正文 / Capability 元数据）
4. **两套循环协议并存**（worker `ReActLoop` vs orchestrator brain）且边界文档不足

**量化锚点（Wave B/C 优先于其他重构的硬理由）**：worker 侧 `react_loop/` 生产代码约 **8.1k** 行，orchestrator 约 **8.9k** 行，两条编排轴合计约 **1.8 万行**；pi 对等的循环核心是一个可组合的 `runLoop` 量级函数。行数上「内核薄、产品厚」尚未成立——厚的是**双轴编排**，不是 UI 壳。

优化方向：**向 pi 借减法与协议纪律，不借产品薄度**——瘦循环、钩子化扩展、统一词汇与边界，**保留**证据/worker/Write 硬规则。

---

## 1. 代码现实（以仓库为准）

### 1.1 分层与入口

```text
Transport / MCP（薄）
  → Product Apps
       ConversationApp::execute[_stream]   # app-bootstrap/product_apps/conversation.rs
       （Write 在产品边界硬拒；真 Write 走 writer 旁路）
  → app-chat
       ChatContext pipeline
       ├─ orchestrator/*（可选 V2 brain + worker 通道）
       └─ UnifiedAgent（Chat/RAG/Search → ReActLoop）
  → agent-loop（ReActLoop + policy + synthesis + answer_contract + output_compiler）
  → agent-tools（ToolCatalog / dispatch_tool / Capability / progressive Skill-MD / SkillComponent）
  → llm / rag-core / search / code-interpreter / write-core / storage
```

**已对齐 pi 的部分**

| 实践 | 仓库落点 | 评价 |
|------|----------|------|
| 单一执行入口 | `dispatch_tool` + `ToolCatalog`（T3） | 强；EXTENDING.md 写死禁区 |
| Mode 配置外置 | `modes/*.yaml` + `ModeConfig` | 强 |
| Progressive disclosure | `DisclosurePlanner` + `ContextAssembler` | **强于 pi**（服务端阶段披露） |
| 事件可观测 | `AgentEvent` + sinks / SSE | 强 |
| Write 不进 ReAct | T2 + ConversationApp 拒写 | 产品纪律，pi 无对应 |
| 不把每个 API 做成 native tool | RAG `tool_pool: []` + codegen 簇 | 与 ADR-0006 引用的 pi 实践一致 |

### 1.2 体量快照

**口径（2026-07-29 机检）**：`wc -l` 对 `*.rs`；「生产」= 路径名不含 `test` 的文件（`hooks.rs` 等文件内 `#[cfg(test)]` 块仍计入文件总行，故单文件「含测试」会另注）。

| 区域 | 行数（口径） | 含义 |
|------|----------------|------|
| `react_loop/` 仅顶层 `*.rs` | **6130** | 不含 `policy/`、`iteration/` 子目录 |
| `react_loop/` 全部 `*.rs` | **9118** | 含子目录 + `*tests*.rs` |
| `react_loop/` 排除 `*test*` 文件名 | **~8104** | **本文默认「worker 循环生产体量」** |
| 其中 `answer_contract.rs` | 1527 | 终答解析/校验/抬升/降级 |
| 其中 `iteration_codegen.rs` | 905 | codegen 沙箱 + bridge + 进度 |
| 其中 `assembler` + `disclosure_plan` | ~878 | 上下文装配 |
| 其中 `hooks.rs` | 289（约 191 行为 `#[cfg(test)]` 区） | **仅** truncate 语义 + 既有 pair-safe 测试 |
| `agent-tools/src` 全部 | 6864 | 含测试模块 |
| `agent-tools` 约去 `#[cfg(test)]` 后 | **~4.7–4.9k** | 量级描述可用；精确以全量 6864 为准 |
| `app-chat/orchestrator/*` | **8901** | brain + host + workers + store + session |
| 其中 `brain.rs` | **2170** | 第二套 LLM 循环（host-intercept tools） |

**结论（加强）**：worker 循环（~8.1k 生产）与 orchestrator（~8.9k）**体量同级**；双轴合计约 **1.8 万行**编排代码。pi 对等物是单循环函数 + 薄状态壳。这不是「该砍证据层」，而是「该把循环收成协议，避免双轴各自继续长胖」。

### 1.3 `ReActLoop` 实际形态

```50:58:avrag-rs/crates/agent-loop/src/react_loop/mod.rs
pub struct ReActLoop {
    llm: Arc<LlmClient>,
    skill_registry: Arc<CapabilityRegistry>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    code_interpreter: Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
}
```

`run()` 固定装配：

- 状态：`IterationState`（messages / disclosed / tool_results / sandbox errors / alias cursor…）
- 钩子：`let hooks = StandardLoopHooks::default();` —— **调用方无法注入**（`mod.rs` ~L112）
- 阶段：`run_retrieval_loop` → `resolve_synthesis_gate` → `run_synthesis_phase`
- 工具：`dispatch_tool_call` → `OwnedToolDeps` → `dispatch_tool`
- 旁路：`dispatch_codegen`（非 native tool schema 的主检索路径，RAG mode）

对照 pi：`agentLoop` **不持有** provider 实现细节以外的业务依赖；依赖经 `AgentLoopConfig` 回调注入。我们的 loop **同时是发动机 + 部分运行时 DI 容器**。

### 1.4 钩子与消息队列：名义协议 vs 实装

| 机制 | 设计意图 | 代码现实 |
|------|----------|----------|
| `LoopHooks::transform_context` | 每轮 LLM 前改消息（ADR-0008） | 仅 `StandardLoopHooks` 中段 drain；`run()` 写死 default |
| `LoopHooks::convert_to_llm` | 边界变换 | 默认恒等；未见产品注入 |
| `LoopMessageQueue` steering/follow-up | 中途插话 / 结束后追加 | **占位**：`_steering`/`_follow_up`；`drain_steering_before_turn` 恒空；源码自注 *「v0.1 placeholder — deferred to ADR-0008 v0.2」* |
| 工具策略 / 拦截 | pi `beforeToolCall` | **策略真相源** = `PolicyEnforcer` + codegen 硬门；**无** loop 级 hook 面 |
| turn 间停止 / 热切换 | pi `shouldStopAfterTurn` / `prepareNextTurn` | 退出在 `exit_policy` / `IterationControl`，非可注入回调 |

`EXTENDING.md` 已写「Prefer `LoopHooks` over forking `ReActLoop::run`」，但 **API 面未兑现**。

**策略边界（后文 B2 必须遵守，避免新双真相）**：

| 关切 | 真相源 | 允许 hooks 做什么 |
|------|--------|-------------------|
| 权限 / tier / risk / deny | **`PolicyEnforcer`（+ catalog 元数据）** | **不得**另立并行 deny 策略；若暴露 `before_tool_call`，默认实现应 **委派** enforcer 或仅观测 |
| codegen SDK 误走 native | iteration/codegen 硬门（B4 可收口到 dispatch） | 观测 + 既有拒绝路径，不复制一套规则 |
| 消息裁剪 / 前缀稳定 | `LoopHooks::transform_context` | hooks **是**真相源 |
| 退出闸门（证据/预算） | `LoopPolicy` / `exit_policy` | hooks 至多观测或包装信号，不静默改写 gate 语义 |

### 1.5 Tool / Skill / Capability 三套登记表

| 类型 | 路径 | 语义 |
|------|------|------|
| `ToolCatalog` + `ToolExecKind` | `agent-tools/catalog.rs` | **唯一 execute 路由**（Rag \| Skill） |
| `SkillComponent` + `SkillRegistry` | `agent-tools/skills/` | **可执行 Rust 工具**（web_search、calculator…） |
| `progressive::Skill` + `PromptRegistry` | `agent-tools/progressive/` | **SKILL.md 披露正文**（无 execute） |
| `CapabilityRegistry` | `agent-tools/capability/` | mode schema、元数据、策略、阶段查询 |
| Orchestrator host tools | `app-chat/orchestrator/brain.rs` | `delegate_*` / `finish_answer` / `evidence_fetch`：**故意不进 ToolCatalog** |

词汇冲突：产品说的「Skill」常指 MD；代码里 `SkillComponent` 实际是 Tool。pi 的 Skill **只有** markdown 一义——我们多出来的层是合理的（SaaS 策略），但**命名未收敛**，扩展时最容易踩坑。

### 1.6 双循环：worker ReAct vs orchestrator brain

```text
User turn
  └─ brain (LLM loop, host-intercept tools, max_rounds≈6)
        ├─ delegate_rag / delegate_search
        │     └─ WorkerSession（通道级持久）
        │           └─ ReActLoop (retrieve → gate → synthesis/handoff)
        │                 └─ codegen / native tools / progressive skills
        ├─ evidence_fetch（读 EvidenceStore）
        └─ finish_answer → chat_exit / 证据水合 / 用户可见答
```

这与 pi「sub-agent = 再套一层 `agentLoop`，当 tool 执行」**同构于意图**，但实现上：

- brain **不是** `ReActLoop` 复用，而是独立消息循环 + 手写 tool_spec
- worker 记忆在 `WorkerSession`（compaction resume），不在通用 Agent 壳
- 证据正确性靠 **代码水合 SELECTED**，不是靠模型自由写全文

**应保留**：证据平面与通道持久 worker 是评测驱动的产品正解（见 handover 2026-07-28）。  
**应治理**：两套循环的「协议共同点」（事件、预算、取消、hooks、tool 结果形态）未抽象，导致改动成本 ×2。

### 1.7 RAG mode 配置现实（示例）

`modes/rag.yaml`：`tool_pool: []`，retrieve 强制 `codegen` 簇，`auto_fallback.dense_retrieval`，synthesis 可空。  
即：**主路径是「LLM + Python SDK 沙箱」**，原生 dense 是兜底——比 pi 的「tools 始终在 schema 里」更激进，也更依赖 `iteration_codegen` 的正确性。

### 1.8 产品边界（不要动）

- T1–T8 / Product Apps 单入口
- Write 永不进 `ToolCatalog`
- Capability / Skill-MD / Tool 三层不合并（T4 No C4）
- workspace / user 真相

这些是 **产品法**，不是 pi 缺口。

---

## 2. pi 实践 → 我们的映射表

| # | pi 实践 | 我们现状 | 落差 |
|---|---------|----------|------|
| P1 | 循环引擎尽量无状态 / 少知 | `ReActLoop` 持有 rag/search/persistence/codegen | **高** |
| P2 | 有状态壳（Agent）与循环分离 | `UnifiedAgent` 薄；状态散落在 IterationState + WorkerSession + pipeline | **中** |
| P3 | 扩展靠回调协议，不靠 fork 循环 | `LoopHooks` 过窄且不可注入；exit 在 policy 硬编码 | **高** |
| P4 | Tool = 可执行接口对象 | `dispatch_tool` + SkillComponent | **低**（已齐） |
| P5 | Skill = MD，索引进 prompt，全文延迟读 | 服务端 Assembler 按阶段注入全文/簇 | **有意不同**（更可控） |
| P6 | 工具执行管道统一（prepare/execute/finalize） | native tools 统一；codegen 独立管道 + bridge | **中** |
| P7 | 错误进事件流 / stopReason，少抛 | 部分；AppError 与 ToolResult 混用 | **中** |
| P8 | 不内建 sub-agent，用 tool 组合 | orchestrator + WorkerSession 产品内建 | **有意不同** |
| P9 | steering / follow-up 双队列 | `LoopMessageQueue` 占位（ADR-0008 v0.2 deferred） | **低优先级**（SaaS 一轮请求） |
| P10 | 极简核心、能力外置 | agent-loop 已拆 crate，但 answer_contract/codegen 仍沉内核 | **中** |
| P11 | 洋葱单向依赖 | Product Apps 较好；loop→rag/search 仍直接耦合类型 | **中** |

---

## 3. 问题诊断（按冲击排序）

### D1. 循环不是协议，是胖服务

**现象**：新增工具依赖、改检索桥、改 persistence 回落路径，都要碰 `ReActLoop` 字段或 `impl` 分裂文件（`run_*` / `iteration_*`）。

**后果**：可测性与可组合性下降；worker 与单 mode 直跑共享同一胖类型，难以为 orchestrator 定制「更瘦的循环配置」。

**pi 对照**：`runLoop(context, config, signal, emit, streamFn)`。

### D2. 钩子未产品化

**现象**：`run()` 内 `StandardLoopHooks::default()`；前缀缓存优化方案（`docs/plans/2026-07-04-llm-prefix-cache-optimization.md`，2026-07-04）已指出 drain 破坏 cache，但改钩子策略仍要改内核默认实现。**该 plan 之后仓库又经历 markitdown / orchestrator 大修**——落地前须对照现代码 rebase（见 A4）。

**后果**：prefix-cache、plan 注入、turn 间 model 切换、测试用 recording hooks 都无法「外挂」。

### D3. Skill 词汇三义

**现象**：`SkillComponent`（可执行）vs `progressive::Skill`（MD）vs 口语「skill 簇」。

**后果**：新人 / 代理改代码时把 MD 注册进 execute 路径，或反过来把工具当披露单元——违反 T4 与 EXTENDING 的风险高。

### D4. Codegen 成为第二执行平面

**现象**：~900 行 `iteration_codegen` + bridge 工具结果回灌；SDK 方法误走 native tool 有硬门拒绝。

**后果**：正确，但是「工具执行管道」双轨。观测、重试、策略、预算对 codegen 与 native 不完全对称。

### D5. answer_contract 膨胀在 loop 内

**现象**：~1.5k 行解析/校验/抬升/降级与 ReAct 同 crate 同目录。

**后果**：循环「知道」过多终答产品形态；与 output_compiler / evidence plane 职责边界模糊。

### D6. Orchestrator 第二协议

**现象**：host-intercept tools 不进 catalog；独立 prompt、独立 round 预算、独立错误路径；~8.9k 行。

**后果**：合理产品层，但缺少「与 ReActLoop 共享的最小循环协议」文档与类型，演进时双改。

### D7. 死代码与文档漂移

- `LoopMessageQueue` 空实现仍在树内，且与 ADR-0008 v0.2「deferred」叙事挂钩——**不可在未决策 ADR 前直接删除**
- ADR-0005 / 0006 系部分已被 0006-revised / 0007 取代；0006-revised 已有废止头，0005 系与部分路径仍易被新人当现行架构
- EXTENDING 承诺的 hooks 注入未实现

---

## 4. 优化原则（先立规再动刀）

1. **产品正确性优先于架构纯度**  
   证据平面、SELECTED 水合、Write 旁路、通道持久 worker、eval 门闩 — **不因 pi 而删**。

2. **向 pi 借的是「协议边界」，不是「coding 产品薄壳」**  
   目标：循环只认 messages / tools / stop / events / hooks；领域经 deps 注入。

3. **行为保持切片（T5）**  
   动 hooks / drain 前先有 characterization；每项可 `cargo test -p agent-loop --lib` 等局部验收。

4. **先命名与注入面，再搬文件**  
   大搬家收益低于「可注入 hooks + 真正解耦的 deps」。

5. **Orchestrator 保持独立产品层**  
   **不做**：强制 brain 改用 `ReActLoop`。  
   **要做**：只抽取共享协议碎片（事件、预算检查、消息构造），见 C4。

6. **策略单真相**  
   工具 allow/deny/tier 的真相源是 **`PolicyEnforcer`（+ ToolCatalog 元数据）**。`LoopHooks` 不得长成第二套策略引擎；可观测、可委派，不可分叉。

---

## 5. 建议工作包（分波次）

### Wave A — 低风险、立刻降认知债（1–3 天量级）

**执行顺序**：A0 → A1 → A2 / A6（文档）→ A3 → A4 → A5。  
**A0 是 A3/A4 的闸门**：无 characterization 不得改 drain 时序。

| ID | 项 | 做法 | 验收 |
|----|----|------|------|
| **A0** | **characterization：`transform_context` 消息序列** | 在动 hooks 注入 / drain 策略前，固化当前行为：固定 `base_message_count` + 超长 ReAct 轨迹（含 `assistant(tool_calls)`/`tool` 成对），记录 drain **前后** role 序列与 tool_call_id 附着关系。`hooks.rs` 已有 pair-safe 测试（~191 行 test 区），**补「整序列快照」**类用例，避免只断言长度。 | 新增/加严测试失败即拦 A3/A4；`cargo test -p agent-loop --lib hooks` |
| A1 | **词汇表写死** | 在 `agent-tools` / `agent-loop` crate 文档与 EXTENDING 增加术语表：`Tool`（可执行）/ `SkillMd`（prompt）/ `SkillComponent`（遗留可执行别名，计划 rename）/ `Capability`（策略元数据）/ `HostTool`（orchestrator 拦截） | 文档 + 代码注释一致；无行为变更 |
| A2 | **占位队列：标记，不删除** | `LoopMessageQueue`：**保留**；加 `#[deprecated]`（或模块级 `//! deprecated`）+ 文档写清「SaaS 一轮请求当前不做 steering/follow-up」；**与 ADR-0008 指针对齐**（注明 v0.2 deferred / 或「产品未排期则长期占位」）。**不提供「删除」选项**——删除须单独决策「ADR-0008 v0.2 已死」后再开任务。 | 注释/ADR 交叉链接一致；`agent-loop` 测试绿 |
| A3 | **钩子可注入** | `ReActLoop::run` 增加 `run_with_hooks`（或 `hooks: &dyn LoopHooks`）；默认路径仍 `StandardLoopHooks`，**字节级等价**于今日 `default()`；`UnifiedAgent` 可选透传。A3 **只做注入面**，不改 drain 算法（算法归 A4）。 | (1) 自定义 hook 被调用的单测；(2) A0 快照测试仍绿；(3) EXTENDING 写明：hooks 默认不承载 Policy deny |
| A4 | **前缀缓存钩子落地** | **先**对照现代码 rebase `docs/plans/2026-07-04-llm-prefix-cache-optimization.md`（确认 `transform_context` 调用点仍在 `run_retrieval` 每轮末、消息角色约定未变）；再实现 high-watermark 折叠替代「每轮中段 drain」。 | rebase 结论写进 PR/提交说明；A0 快照按新语义更新；可选前缀稳定断言 |
| A5 | **边界 README 一张图** | `agent-loop/EXTENDING.md` 补「双循环」与 host-tool 禁入 catalog 的图；并链到 §1.4 策略边界表 | 文档 |
| A6 | **旧 ADR 可读性治理（收 D7）** | 给仍可能误读的 ADR-0005 / 0005-revised / 0006 原文路径补或强化 **superseded 标头**（指向 0006-revised / 0007 / 本 plan）；**不改历史正文逻辑** | 打开旧 ADR 首屏即可看到「勿按此实现」 |

**明确不做**：搬 `answer_contract`、改 worker 语义、改 ToolCatalog 内容、删除 `LoopMessageQueue`、让 hooks 旁路 `PolicyEnforcer`。

### Wave B — 循环协议化（中风险，高杠杆）

| ID | 项 | 做法 | 验收 |
|----|----|------|------|
| B1 | **`LoopRuntimeDeps` 外提 + 类型解耦** | 将 `rag_runtime` / `search_executor` / `chat_persistence` / `code_interpreter` 收成 deps 袋。**禁止**只改字段名、类型仍直持 `avrag_rag_core::RagRuntime` / `avrag_search::…` 于 `react_loop/` 公开表面——应用 **port/trait 对象**（或 `agent-loop` 内窄 trait + app-chat 适配），使循环编译依赖不钉死具体 runtime 类型。 | (1) 测试绿；(2) **机检**：`rg 'avrag_rag_core::\|avrag_search::' crates/agent-loop/src/react_loop` 在生产路径归零或仅限 `deps` 适配一层（允许列表写进 PR）；P1/P11 落差下降可陈述 |
| B2 | **扩展 `LoopHooks` 最小集（观测 + 委派，非第二策略引擎）** | 增加：`after_tool_call`（观测）、`on_turn_end`（观测）、可选 `should_stop_after_turn`（**只暴露**已有 `IterationControl`/exit 信号，默认 no-op 不改 gate）。`before_tool_call` 若加：默认实现 **委派 `PolicyEnforcer`** 或纯观测；**禁止**在 hooks 内复制 tier/risk 规则表。策略真相源与 hooks 边界 **写入 EXTENDING**（与 §1.4 / §4.6 一致）。 | (1) 默认路径与现状行为等价；(2) policy 单测不退化；(3) EXTENDING 有「PolicyEnforcer = 策略真相；hooks ≠ 策略」专节；(4) 代码评审拒绝「在 hook 里写 allowlist」 |
| B3 | **Tool 执行观测对称** | native 与 codegen bridge 共用「开始/结束/失败」事件形状（已有 progress 则收敛到一套） | SSE/事件契约测试 |
| B4 | **Codegen 定位文档化 + 接口收口** | 文档写明：RAG 主执行平面 = codegen；native dense = fallback。SDK-method-as-native 拒绝逻辑收口到 catalog/dispatch 边界（避免 iteration 特判扩散） | 回归现有 iteration 测试；可选 llm_real 子集 |
| B5 | **`convert_to_llm` 真正接上** | 在 LLM 调用边界调用 hooks；与 `transform_context` 分工（context 裁剪 vs provider 消息形状） | 单测 |

**成功标准**：新增一种「只读录制 hook / 假 deps」无需改 `run_retrieval` 内部控制流；且 **不能**通过 hook 绕过 `PolicyEnforcer`。

### Wave C — 职责外移与双循环对齐（中高风险，需波次评审）

| ID | 项 | 做法 | 验收 |
|----|----|------|------|
| C1 | **answer 契约模块边界** | `answer_contract` + `output_compiler` 明确为 `agent-loop` 的 **product-contract 子模块**（或 `agent-contract` crate，仅当编译/依赖证明有收益） | 循环核心文件不再直接增长契约分支 |
| C2 | **Orchestrator HostTool 协议** | 抽出 `HostToolCall` / `HostToolResult` 小类型 + 「禁止注册进 ToolCatalog」测试期断言（已有部分 test） | host 工具列表契约测试 |
| C3 | **WorkerSession 与 Loop 状态接口** | alias cursor、resume messages、channel cap 留在 app-chat；metadata 键与 loop 约定写在单一 constants 模块（已有 `ALIAS_START_METADATA`，扩成 worker↔loop 契约表） | worker 测试 + loop 元数据测试 |
| C4a | **不做：brain 改用 `ReActLoop`** | orchestrator brain 保持独立 host-intercept 循环；不强制统一到 worker 循环类型 | 架构评审签字；无「brain 迁 ReActLoop」任务混入 C 波 |
| C4b | **要做：抽取共享碎片** | 仅抽取可复用的小单元：`turn_budget` / `cancel_check` / `tool_result_message` 构造等（能证明减少重复再抽） | 重复代码下降可度量；brain 与 worker 行为不变 |

### Wave D — 明确不做 / 长期可选项

| ID | 项 | 理由 |
|----|----|------|
| D1 | 把 Write 并入 ReAct ToolCatalog | 违反 T2；控制环语义不同 |
| D2 | 删除 Capability 层「变成 pi 两层」 | 违反 T4；SaaS 策略需要 |
| D3 | 把 Skill-MD 改成模型 `read` 文件路径协议 | 多租户服务端无本地 skill FS；安全与可重复性更差 |
| D4 | 实现完整 steering/follow-up 产品能力 | 当前 HTTP 一轮请求模型 ROI 低；除非产品做真正的多轮流式插话 |
| D5 | 内建 Crew 式 multi-agent 框架 | 已有 orchestrator+worker；再抽象易空转 |
| D6 | 为像 pi 而拆掉证据平面 / SELECTED | 直接打穿 faithfulness |
| D7 | 让 `LoopHooks` 成为与 `PolicyEnforcer` 并行的策略引擎 | 制造新的双真相（审核指出的 D 级风险） |
| D8 | 未废止 ADR-0008 v0.2 叙事前删除 `LoopMessageQueue` | 与既有 deferred 计划冲突；见 A2 |

---

## 6. 目标架构草图（演进后）

```text
                    ┌─────────────────────────────────────┐
                    │ Product Apps / pipeline / billing     │
                    └─────────────────────────────────────┘
                                      │
          ┌───────────────────────────┼───────────────────────────┐
          ▼                           ▼                           ▼
   Write lane                   Orchestrator                 UnifiedAgent
   (write-core)                 (HostTools loop)             (mode shell)
          │                           │                           │
          │                           │ WorkerSession              │
          │                           ▼                           │
          │                    ┌──────────────┐                    │
          │                    │ ReActLoop    │◄───────────────────┘
          │                    │  - llm       │
          │                    │  - hooks*    │  *可注入；非策略真相
          │                    │  - deps ports│  *trait，非直持 rag-core 类型
          │                    └──────┬───────┘
          │                           │
          │              ┌────────────┼────────────┐
          │              ▼            ▼            ▼
          │         dispatch_tool  codegen     Assembler
          │         +PolicyEnforcer (sandbox)  (SkillMd)
          │              │
          └──────────────┴── 不交叉 ── Write tools 永不进 catalog
```

与 pi 对齐点：`ReActLoop` ≈ agentLoop；`UnifiedAgent`+`WorkerSession` ≈ Agent 有状态壳（产品化）；`HostTools` ≈ 产品层 sub-agent tool。  
与 pi 保留差异：Assembler 服务端披露、证据水合、Write 旁路、Capability + **PolicyEnforcer 策略真相**。

---

## 7. 验证策略

| 波次 | 最低验证 |
|------|----------|
| A | A0 先合；`cargo test -p agent-loop --lib`；`cargo test -p agent-tools --lib`；A3/A4 必须复跑 A0 快照 |
| B | 上 + `cargo test -p app-chat --lib`；B1 附 `rg` 机检结果；B2 附 EXTENDING 策略边界 diff |
| C | 上 + orchestrator 相关测试；波次末 `bash scripts/test-l1.sh` |
| 涉及 RAG 行为 | 按 `docs/e2e-gates.md`；不要求每切片真 LLM，但 C 波涉及 handoff/证据时建议 nightly 子集 |

WSL：`jobs=2`，避免并行全量 `cargo test`。

---

## 8. 建议决策（请产品/架构确认）

1. **采纳 Wave A 为默认下一步**（顺序：A0 characterization → 词汇表 / ADR 标头 / 队列标记 → 钩子注入 → 前缀缓存 rebase 落地 → 双循环图）。  
2. **Wave B 作为 Q 内架构主线**，成功标准写进 EXTENDING：「不 fork run，只挂 hooks/deps」；**PolicyEnforcer 仍是工具策略真相源**。  
3. **Wave C 仅在 B 完成且 L1 稳定后**开；C4a（brain 不迁 ReActLoop）与 C4b（只抽共享碎片）分列。  
4. **书面确认 D 波「不做」列表**（含 D7 hooks 不作策略引擎、D8 不擅自删队列），避免后续会话以「更像 pi」回退产品硬规则。

---

## 9. 附录：关键文件索引

| 主题 | 路径 |
|------|------|
| 循环入口 | `crates/agent-loop/src/react_loop/mod.rs` |
| 检索环 | `.../run_retrieval.rs` |
| 钩子 | `.../hooks.rs` |
| 占位队列 | `.../message_queue.rs` |
| 装配 | `.../assembler.rs`, `.../policy/disclosure_plan.rs` |
| 工具执行 | `.../iteration_tools.rs`, `crates/agent-tools/src/tool_registry.rs` |
| 策略 | `crates/agent-tools/src/capability/policy.rs` |
| Codegen | `.../iteration_codegen.rs` |
| 状态机说明 | `crates/agent-loop/src/react_loop/STATE_MACHINE.md` |
| 扩展纪律 | `crates/agent-loop/EXTENDING.md` |
| 目录 | `crates/agent-tools/src/catalog.rs` |
| Skill-MD | `crates/agent-tools/src/progressive/` |
| 可执行 skill | `crates/agent-tools/src/skills/` |
| 产品入口 | `crates/app-bootstrap/src/product_apps/conversation.rs` |
| UnifiedAgent | `crates/app-chat/src/agents/unified/mod.rs` |
| Orchestrator | `crates/app-chat/src/orchestrator/{brain,host,workers,worker_session}.rs` |
| Mode | `modes/{chat,rag,search,orchestrator,write_refine}.yaml` |
| 前缀缓存 plan | `docs/plans/2026-07-04-llm-prefix-cache-optimization.md`（A4 须 rebase） |
| 近期编排演进 | `docs/engineering/2026-07-28-handover.md` |

---

## 10. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-29 | 初版：代码审计 + pi 对照 + Wave A–D 建议 |
| 2026-07-29 | **审核修订**：事实口径（§1.2 行数含/不含测试与子目录）；§0 双轴 ~1.8 万行硬论据；A0 characterization 闸门；A2 去掉删除选项并对齐 ADR-0008 deferred；A4 先 rebase 旧 plan；A6 旧 ADR superseded 标头；B1 类型解耦机检；B2 与 PolicyEnforcer 职责划界 + D7；C4 拆 C4a/C4b；§4.6 策略单真相；Wave D 增 D7/D8 |
| 2026-07-29 | **Wave A 实现**：A0 role/tool-id 序列 characterization；A1 词汇表（EXTENDING + crate roots）；A2 `LoopMessageQueue` deprecated + ADR-0008 注；A3 `run_with_hooks` + `&dyn LoopHooks`；A4 `compact_high_watermark` 两档折叠（rebase 确认 plan 状态与代码漂移）；A5 双循环图 + 策略边界；A6 ADR-0005/-revised superseded 标头 |
