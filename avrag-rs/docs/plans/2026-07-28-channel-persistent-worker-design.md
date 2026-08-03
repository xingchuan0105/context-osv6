# 通道级持久 Worker 设计（Channel-scoped Persistent Worker）

> **SUPERSEDED** — 本文描述的 orchestrator / worker / brief / handoff 多 agent 架构已被取代：2026-07-30 起产品路径改为单 agent（SaC 设计，见根 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），orchestrator 代码已于 2026-08-01 物理删除（commit `7f2d182d`）。本文仅作历史记录。（横幅添加于 2026-08-02 文档体系梳理）

| 项目 | 内容 |
|---|---|
| 状态 | **设计待评审** |
| 日期 | 2026-07-28 |
| 关联 | q088 双 worker 冲突诊断；薄编排范式；证据平面（检索日志+水合）设计 |
| 范围 | orchestrator 派发语义与 worker 生命周期；不改检索/评测/提示词体系 |

---

## 0. 一句话

每个通道（rag/search）在一个对话轮内**至多一个 worker 实例**：orchestrator 可以多派工（多个 brief），但同一通道的所有 brief **顺序投递给同一个 worker**——它带着全部前序上下文继续工作。双胞胎冲突类问题（q088）结构性消亡；"重派"从"换新人重踩坑"变成"带记忆的追问"。

## 1. 问题定义

- **现状**：每个 brief 新建 worker（host.rs run_channel → 新 AgentRequest + 新 ReActLoop）。同类任务多 brief 时产生多个互不知情的 worker，结论可能互相矛盾（q088：worker 0 错答 7 项、worker 1 算对 59 但没传出，Answer 选了错的）。
- **不确定性乘数**：单 worker 波动率 p，双 worker 冲突率 ≈2p，且 Answer 的"选详细者"偏好恰好会选中错的那份。
- **重派无效**：新 worker 冷启动，重犯上一个 worker 已踩的坑（q088 worker 0 重复"搜文件名 0 命中"）。

## 2. 最佳实践依据

- [LangChain 架构指南](https://www.langchain.com/blog/choosing-the-right-multi-agent-architecture)：能用简单模式就不上多 agent；多 agent 复杂度必须被证明值得。
- [生产五模式](https://www.digitalapplied.com/blog/multi-agent-orchestration-5-patterns-that-work)：fan-out 仅用于真正独立的并行子任务。
- [AgentFactory](https://arxiv.org/html/2603.18000v1)：持久 subagent + 执行历史 + 连续任务派发，是演进方向。
- 结论对齐用户方案：**supervisor + 通道级持久 worker + brief 队列**。

## 3. 设计

### 3.1 生命周期

```
轮开始
  └─ brain 首次 delegate_rag(brief#1)
       └─ 创建 rag WorkerSession（新 ReActLoop 上下文）
            ├─ brief#1 → 运行至交接（summary + SELECTED）→ 挂起
  └─ brain delegate_rag(brief#2)（同通道再派）
       └─ 同一 WorkerSession：brief#2 作为新 user 消息注入
            ├─ 带着 brief#1 的全部消息/工具结果继续
            └─ brief#2 交接 → 挂起
  └─ brain finish_answer → 全部 session 终结，证据库定稿
```

search 通道同理（web 任务走 search worker，不进 rag worker）。

### 3.2 语义规则

| 项 | 规则 |
|---|---|
| **一通道一实例** | 同一轮内同通道 brief 全部进入同一 WorkerSession；**管道上不存在同通道第二个实例** |
| **brief 预算** | 每个 brief 独立迭代预算（沿用 mode yaml `max_iterations=4`）；另设通道级总量上限（初值 10 轮/轮/通道）。**cap 在 brief 中途触发时**：当前 brief 走 C5 式强制交接 → **session 封印**（不再接受新 brief，brain 收到"通道预算耗尽"信号） |
| **上下文** | 消息历史、tool_results、别名计数器跨 brief 保留（别名在轮内全局唯一）。**防膨胀**：跨 brief 时压缩旧消息——最近一个 brief 保留完整消息，更早 brief 只保留交接摘要 + 圈选清单 |
| **故障隔离** | worker 运行硬失败（传输层错误/异常）→ session 标记 failed；**下一个 brief 创建新 session** |
| **E105 作用域** | 零检索查无判定按 **brief 作用域**（`has_tool_results_this_brief`，按 brief 的增量 tool calls）——防止 brief#1 的检索记录为 brief#2 的零检索查无背书（自审 BUG-1） |
| **证据** | 同一 evidence store；brief#2 可直接引用 brief#1 的圈选（多跳红利）；store 条目带 `brief_seq` 标记出处 |
| **交接** | 每个 brief 产出自己的 summary + SELECTED（证据平面规则不变；水合按该 brief 截止点的别名映射）；ChannelNote 按 brief 记录 |
| **重派** | 语义变为"同一 worker 的追问 brief"；delegate 工具文案同步改为"向通道 worker 投递任务/追问"（消除 brain 的"每 brief 新工人"心智） |
| **并行性** | 跨通道并行不变（rag 与 search 可并行）；同通道 brief 串行 |
| **终结** | finish_answer 时所有 session 关闭；V1 路径（无 brain）每通道恰好一个 brief，行为与今天完全一致 |

### 3.3 与现状的替换关系

- `delegate_rag`/`delegate_search` 语义从"spawn worker"改为"向通道 session 投递 brief"（对 brain 的工具签名不变）。
- `mode_debug.workers[]` 从"每实例一条"改为"每通道一条，含 `briefs[]` 段"——**破坏性变更**：结构带版本标记（`mode_debug_version: 2`），eval 消费方（e2e-analyzer、harness、分析脚本）在 W2 同步适配；旧产物按 v1 读取。工具结果聚合方式不变。
- 冲突裁决类逻辑不再需要（同通道不再产生两份交接）。

## 4. 代码接缝

| 位置 | 变更 |
|---|---|
| `orchestrator/`（新 `worker_session.rs`） | `WorkerSession { channel, messages, tool_results, iterations_used, brief_seq, alias_counter }`；`run_brief(brief) -> BriefOutcome` |
| `host.rs` / `brain.rs` dispatch 路径 | `HashMap<Channel, WorkerSession>` 挂在编排上下文；dispatch → `session.run_brief`；重派同路 |
| `workers.rs` | 交接解析按 brief 输出；handoff 带 `brief_seq` |
| `chat_exit.rs` / mode_debug | workers 结构改"每通道一条 + briefs[]" |
| `store.rs` | 条目加 `brief_seq`（serde 加法） |

## 5. 切片

| 切片 | 内容 | 验证 |
|---|---|---|
| W1 | `WorkerSession` 抽象 + dispatch 改单例投递 + V1 路径适配 | 现有 app-chat 测试全绿（单 brief 行为不变）；新测试：同通道两 brief 共用上下文（第二个 brief 的消息历史含第一个的工具结果） |
| W2 | brief 预算（每 brief 预算 + 通道总量上限）+ brief_seq 标记 + mode_debug 结构调整 | 预算边界测试；eval 消费方（e2e-analyzer/harness）兼容 |
| W3 | 验收跑（q088 型同题重派场景 + q105 跨文档 + 全量基线） | 无双胞胎冲突；重派命中率；judge 标签不回退 |

## 6. 风险

| 风险 | 缓解 |
|---|---|
| 跨 brief 上下文污染（无关任务互相干扰） | 现有观察预算/截断 + 证据平面分层；观察轮看串味率 |
| 通道级总量失控 | 每通道 10 轮硬上限 + finish 时强制交接 |
| mode_debug 结构变更波及 eval 工具 | W2 同步改消费方；旧产物读取保持兼容（serde 加法） |
| brain 不擅长写"追问 brief" | 教学一行即可（orchestrator-base：同通道再派=给同一 worker 的追问，写明与上次的不同） |

## 7. 非目标

- 不改 brain 的垂直策略（薄编排是更大议题，另案）。
- 不做跨轮持久 worker（session 仅限单轮对话内）。
- 不改 per-brief 内的任何既有机制（adaptive-k、编译器、E105、证据平面）。
