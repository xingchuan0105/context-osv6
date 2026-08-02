# 架构加深优化计划（2026-08-02）

| 项 | 内容 |
|---|---|
| 日期 | 2026-08-02 |
| 状态 | **主体波次全部交付、全部双轴审毕**（W1–W5 + W1返工 + C6小修 + ADR-0009）：7 提交验收，无推倒项；余 P0×3 / P1×4 归并返工清单（§5 末）+ P2 记录项 |
| 来源 | 架构审查报告 `/tmp/architecture-review-1785639147.html` + 深潜报告 `/tmp/architecture-deepdive-1785640412.html`（7 候选，均含现状接口/问题/加深设计/切片/测试存活/ADR 约束） |
| 范围 | `avrag-rs`：rag-core / agent-tools / app-chat / agent-loop / llm / billing |
| 不做 | push/PR/CI；failover 统一（C4 切片 3，留到有真实流式故障复现）；`RuntimeExecuteRequest` 加 `doc_scope` 字段（决策 1） |
| 约束 | T1–T8；T5 行为保持切片、验证门不过不前进；prompts-in-md；第三人称观察式 prompt；solo 本地 trunk；WSL `jobs=2` 不叠加并发全量 cargo test；每波结构性改动后 `code-review-graph update`（不提交 .code-review-graph/） |
| 执行须知 | 报告中的文件/行号基于 2026-08-02 快照；**动手前逐条对照源码再核实**，前提不符就停下报告，不即兴发挥 |

---

## 0. 一句话

七个候选同一类病：**单一概念多处实现、接缝测试通过但生产不可达、死代码靠 re-export 续命**。统一按「单一深模块 / 单一事实源」收束，分五波推进，每波内部是行为保持的小切片，行为变更切片单独隔离并先写测试锁。

## 1. 产品决策（已拍板，执行窗口直接采用）

| # | 决策点 | 结论 |
|---|---|---|
| 1 | HTTP `RuntimeExecuteRequest` 是否加 `doc_scope` 字段 | **不加**。C1 只做切片 1/2/4：HTTP 面 scope 从 auth 工作区推导收窄，会话级 scope 不做 |
| 2 | 画像记忆 cadence 语义 | **全量 v2**：画像推断统一走 dream-v2（LLM delta），general-v1 关键词启发式**下线删除**；24h 一道闸门只挡 v2 |
| 3 | `answer_contract` 私有 extract 不读 `image:` 前缀 | **当 bug 修**：统一后 grammar 认识 `[[image:]]`，answer_contract 分相后不再漏 |
| 4 | 是否接第二家支付 | **要**。支付宝（Alipay）不是备胎，是正式大陆境内收费渠道；C3 四个切片全做，含 AlipayAdapter |

## 2. 波次总览

| 波 | 候选 | 评级 | 关键杠杆 | 行为变更面 |
|---|---|---|---|---|
| W1 | C1 RAG doc-scope 接缝 | Strong · 安全相关 | scoped RAG dispatch 单入口 + 唯一 intersect | 低（全行为保持 + 收窄） |
| W2 | C2 画像记忆 | Strong | profile_update 单模块 + 一道闸门；**general-v1 删除、只留 dream-v2**（决策 2，行为变更） | 中 |
| W3 | C5 引用语法 / C3 Billing | Worth exploring | grammar 单模块 + golden；PaymentProvider 双 adapter | 低-中 |
| W4 | C7 工具元数据 / C4 llm 调用 | Worth exploring | catalog 单源派生；Transport 接缝（不含 failover 统一） | 低 |
| W5 | C6 chat 流水线重组 | Worth exploring | 真私有子模块 + 文档顺序=代码顺序 | 低 |

顺序理由：C1 唯一安全相关先做；C2 与 C6 共享 `service_postprocess.rs`，先收拢 profile 再重组文件；W3 含低成本减法；C4 failover 是行为密集区靠后；C6 最后。

**波次实际状态（2026-08-02 核，与 git 事实对齐）**——并行执行窗口已先行提交，审查窗口此前记录的「W2–W4 未开始」已过期：

| 波 | 候选 | 提交 | 状态 |
|---|---|---|---|
| W1 | C1 | `58d34c74`（交付）→ `a051223f`（返工） | 交付已审 + 返工已复核（§5） |
| W2 | C2 | `eb1c972f` | 已提交**待双轴审** |
| W3 | C5 引用语法 | — | **未做**（本窗口排期） |
| W3 | C3 Billing | `c7458770` | 已提交**待双轴审** |
| W4 | C7 | `2434df10` | 已提交**待双轴审** |
| W4 | C4 | `7d23423a` | 已提交**待双轴审** |
| W5 | C6 | `dc5876d0`（交付）+ 小修（本窗口） | 交付已审 + 小修已落地（§5） |

---

## W1 · C1 RAG doc-scope 接缝（决策 1 已裁切）

**现状（待核实）**：三条 RAG 执行面 scope 处理分歧——
- ① ReActLoop → `agent-tools/src/tool_registry.rs:125-132,149-170`：match 前 reject 全部 8 个 RAG id（SAC_SUPERSEDED_NATIVE_TOOLS）→ `ToolExecKind::Rag` 分支与 `dispatch_rag_tool` **死代码**；
- ② `agent-tools/src/rag_bridge.rs`：`force_doc_scope` + `intersect_doc_scope` 拷贝 #1（:25），11 个测试全在守死路径；
- ③ HTTP `runtime/execute` → `app-chat/src/rag_execute.rs:17-21` → `rag-core/src/runtime/tools/mod.rs`/`dense.rs`：调用方 `args.doc_scope` **原样透传 Milvus**（doc_scan/doc_grep 是全量内容读取；Milvus doc_filter 仍钉 owner_user_id，故是工作区内越权、非跨租户）；
- ④ `rag-core/src/runtime/bridge.rs:401`：intersect 拷贝 #2（codegen RuntimeBridge 用，`deps.rs:147-172`）。

**设计**：rag-core 新建 `scoped_rag_dispatch`；唯一 `intersect_doc_scope` 实现放 rag-core，bridge.rs 与 dispatch_scoped 共用；HTTP 面经 scoped 入口，scope 从 auth 工作区推导（**不加请求字段**）；agent-tools rag_bridge 改薄委托或删除。

| 切片 | 内容 | 验证门 |
|---|---|---|
| 1（纯移动） | intersect 收拢 rag-core 单点 | rag-core 15+ bridge 测试 + agent-tools 11 个 rag_bridge 测试迁移后保绿 |
| 2（HTTP 接管） | `execute_runtime_tools` 走 dispatch_scoped，scope=auth 工作区 | 新增 HTTP 成功路径 contract 测试断言相交生效（现 4 个 contract 测试全走无 rag_runtime 的 AppState，成功路径零覆盖） |
| ~~3~~ | ~~请求加 doc_scope 字段~~ | **决策 1：不做** |
| 4（减法） | 删 `ToolExecKind::Rag` 死分支 + 无调用者的 `dispatch_rag_tool` | 全仓 grep 无引用；`cargo test -p agent-tools -p rag-core -p app-chat` 绿 |

ADR：ADR-0006 §5（RAG 执行面只认 AgentLoop+ToolCall）支持收拢；T7/T8 scope 从 auth 工作区推导符合。

## W2 · C2 画像记忆（决策 2：全量 v2）

**现状（待核实）**：四条写入路径写同一行 `UserProfileRow`（9 字段，含 `inferred_at` + `inference_version`）——
- general-v1（`chat/service_postprocess.rs:64,119-185`，chat 模式内联，闸门 #1）；
- dream-v2（`chat_private/memory.rs:42-50` `maybe_update_structured_profile`，闸门 #2 重读 `inferred_at`）——**互斥死锁**：chat 模式 general-v1 先跑并戳 `inferred_at=now`，dream 闸门必失败，dream-v2 在 chat 模式从未生效；
- `remember_explicit_agent_preference`（无闸门）；admin `save_user_preferences`（重写整行 + 戳 inferred_at，静默饿死闸门写入者）；
- 纯 merge fns（`chat_private/profile_merge.rs`）有 15 个 JSON fixture 测试，调用代码零测试。

**设计（按决策 2 修订）**：app-chat 内新建 `profile_update` 模块，接口 `async fn maybe_update_profiles(ctx, messages, existing) -> Result<()>`：
- **一道 24h cadence 闸门**，闸内唯一策略 = dream-v2（LLM 经接口注入，单测用假策略）；
- **general-v1 启发式（`memory_helpers.rs` derive_* 段）删除**，不是并存——决策 2「全量 v2」；
- 显式偏好与 admin 保存路径保持不变，但禁止 admin 保存戳 `inferred_at` 饿死 v2 闸门（执行时确认这两条路径与 cadence 的交互并补测试）。

| 切片 | 内容 | 验证门 |
|---|---|---|
| 1（行为锁） | 用已有 `InMemoryProfilePort`（`memory_chat_persistence.rs:464`）写测试锁现状（含「general-v1 写入 → dream 闸门跳过」死锁），或记为已知缺陷 | 测试能复现死锁 |
| 2（收拢） | 抽 profile_update 模块，迁入 dream 段 + profile_merge 规则，pipeline 调用点从两处内联改一处 | 15 个 merge fixture 测试原样迁移保绿 |
| 3（语义落地） | 删 general-v1 路径与 derive_* 启发式；dream-v2 成为唯一策略；一道闸门 | 新增：闸门单测（InMemory fake）、dream 假策略测试（现 dream 层零测试）、显式偏好/admin 与 cadence 交互测试 |

ADR：ADR-0006 #3（生产不要 Memory 适配器）符合——PostgreSQL + InMemory 测试 fake。

## W3 · C5 引用语法（决策 3：image: 当 bug 修）

**现状（待核实）**：citation/block 线格式 6+ 份手写扫描器——`agent-loop/src/react_loop/answer_contract.rs:357` 私有 `extract_cite_chunk_ids`（**不读 `image:`**，已漂移）、`agent-loop/src/cite_extract.rs`、`app-chat/src/prompts/citations.rs`（逐字节死拷贝、零消费者）、`rag-core response_utils.rs:70`（第三份拷贝 + 全仓唯一 extractor 测试）；`[block n]` 线格式 `iteration_codegen.rs:369` 生产、`exit_policy.rs:82-117`/`codegen_bridge.rs:24`/`handoff.rs:110-111` 解析，无规范文档。answer_contract.rs 1629 行单体混提示词文本+JSON 规范化+校验+救回，另有 4 个死 pub 项。

**设计**：citation grammar 单模块放 rag-core（`extract_markers(text) -> Vec<Marker>` + `format_block(idx, kind, body)`，单一实现 + golden 测试）；各消费方全部改委托；answer_contract 分相私有子模块（parse→validate→sanitize→lift→envelope→salvage）；**`image:` 支持补齐（决策 3）**；线格式规范写入 `prompts/capabilities/knowledge-base/SKILL.md`（prompts 硬规则：LLM 可见格式须有出处）。

| 切片 | 内容 | 验证门 |
|---|---|---|
| 1（golden 锁） | 为各 extractor 现状行为写 golden 测试（含 image: 差异） | 漂移成为显式失败用例 |
| 2（收拢） | grammar 模块落地；citations/selected/response_utils/exit_policy/codegen_bridge 改委托；**统一后认识 `[[image:]]`** | codegen_bridge 5 测试、handoff 15+ 测试、response_utils 测试保绿并指向共享模块；image: 行为变更用 golden 体现 |
| 3（减法） | 删 app-chat citations.rs 死拷贝 + answer_contract 4 个死 pub 项 | grep 无引用 |
| 4（分相） | answer_contract 拆私有子模块（纯移动，pub 面不变） | 既有测试绿 |

## W3 · C3 Billing provider（决策 4：支付宝为正式渠道，四切片全做）

**现状（待核实）**：`billing/src/service.rs:151-268` checkout 创建 + `:292-428` webhook 第一分发内联 Creem 专有逻辑；`app-bootstrap/.../core_webhooks/process.rs`（342 行）第二分发器 `serde_json::Value` 手工取字段 + 静默默认值；`contracts/src/billing.rs` 未用死词汇表；`core.rs`/`webhook_parse.rs`/`tier.rs` 三个死文件。真实 bug 面：Alipay 签名 sign 排除 sign、验签排除 sign+sign_type 的不对称、CJK percent-decoding、cents 比较。

**设计**：billing crate 内 `payment_provider` 模块：`trait PaymentProvider { fn id(); async fn create_checkout(&self, req) -> CheckoutSession; async fn parse_event(&self, raw) -> ProviderEvent }`；`CheckoutSession`/`ProviderEvent` 进 contracts（替换或删除死词汇表）；webhook 按 store 中已注册 provider_id 路由 adapter，第二分发器删除；类型化解析、失败显式报错；**CreemAdapter + AlipayAdapter 双落地**。

| 切片 | 内容 | 验证门 |
|---|---|---|
| 1（减法） | 删 core.rs/webhook_parse.rs/tier.rs + contracts/billing.rs 死词汇 | grep + `cargo test -p billing` 无破坏 |
| 2（接缝） | 抽 PaymentProvider trait + CreemAdapter，service.rs 改经 trait（行为保持） | 现有 BillingStorePort 测试保绿 |
| 3（合并分发） | webhook 第二分发并入 adapter 路由，删 process.rs 第二分发器；解析类型化 + 显式错误 | 新增：Creem webhook 真实样本 golden 测试；静默默认值消除后的显式错误路径测试 |
| 4（Alipay） | AlipayAdapter 正式落地：checkout（qr_code）+ webhook 验签（sign/sign_type 不对称、CJK percent-decoding、cents 比较逐项测试） | 新增：Alipay 样本 golden 测试 + 验签不对称单测 |

ADR：ADR-0001 计费主体 user_id（B2C）——adapter 层保持 provider 无关；不触碰 Rolling 用量（ADR-0006 #4）。

## W4 · C7 工具元数据单源

**现状（待核实）**：id→权限/风险事实硬编码三处须互相一致——`agent-tools/src/catalog.rs`（`skill_policy_defaults:149-160` + `infer_risk:162-172` + `rag_tool_metadata:174-201`，运行时权威）、`capability/policy.rs:176-237`（`standard_rules` 内嵌 id 清单）、`capability/registry.rs:288-293`（`infer_skill_risk_level` 只喂 prompt SkillMetadata）。已有漂移：rag_tool_metadata 全 Medium vs infer_risk 默认 Low。`PolicyEnforcer::new(standard_rules())` 每次 dispatch 重建（`tool_registry.rs:182-186`）。`ContextRiskLevel::tool_allowed`（policy.rs:299-305）零调用者。

| 切片 | 内容 | 验证门 |
|---|---|---|
| 1（一致性锁） | 跨表一致性测试：catalog/policy id 清单/registry risk 对每个工具 id 必须一致 | 当前实现应有失败用例（暴露 Medium/Low 分歧） |
| 2（单源） | catalog 表扩为完整元数据（permissions+risk+strategies+phase）；standard_rules 改为派生 | 派生与现硬编码等价的回归测试 |
| 3（缓存） | PolicyEnforcer 规则集缓存（OnceLock / 构造一次注入） | 热路径不再每次重建 |
| 4（减法） | registry risk 改读 catalog（或删，若 SkillMetadata 投影可派生——执行窗口决策）；删 `tool_allowed` | grep 无引用 |

ADR：ADR-0006 §5a + T4 不塌层——catalog 仍是唯一表。

## W4 · C4 llm 调用（切片 3 failover 统一**暂缓**）

**现状（待核实）**：`llm/src/client/mod.rs:593-720` 内联流式 failover 循环 vs `routing/mod.rs:337-358` `try_each` 结构分歧；Route reqwest 硬接线（pool 测试要真实 axum socket）；`usage_to_event_usage` ×4 份；死面：Framing（单变体）、RoutePatch、Provider struct、LlmProvider trait。

| 切片 | 内容 | 验证门 |
|---|---|---|
| 1（收拢） | 4 份 usage_to_event_usage 合并为 1 | 现有 provider 测试绿 |
| 2（接缝） | 抽 `trait Transport`（reqwest impl + 测试 fake），Route 持 Transport | pool 测试可离网跑 |
| ~~3~~ | ~~failover 统一~~ | **暂缓**：留到有真实流式故障复现；届时先明确「流已产出字节即不再尝试下一 key」的交付边界语义 |
| 4（减法） | 删 Framing/RoutePatch/Provider/LlmProvider trait（grep 确认无调用者后） | grep + `cargo test -p llm` 绿 |

## W5 · C6 chat 流水线重组（最后，等 W2 完成）

**现状（待核实）**：`chat/service.rs:331-332` `include!("service_modes.rs")` + `include!("service_postprocess.rs")` 无可见性边界；`mod.rs:4-5` 文档称 terminal 事件在最后，实际 `run_pipeline` 顺序 audit(:166-189)→output_guard(:191-201)→terminal 事件(:203)→persist(:205-216)→usage→notifications——**文档撒谎**且 audit 未文档化；10 个测试全走 dispatch_*，脊柱/preflight/guard/persist/usage/notifications/SSE 零覆盖；死代码：`prompts/{search_eval,strategy_eval,types,internal}.rs`、`memory_helpers.rs:143-270` 12 个死 builders（lib.rs:49-52 re-export 续命）；`chat_private/` 漏 3 个 pub。

| 切片 | 内容 | 验证门 |
|---|---|---|
| 1（行为锁） | 端到端管道测试锁脊柱顺序（含 SSE 事件序） | 能写出与文档矛盾的失败用例 |
| 2（文档修正） | 修 mod.rs 文档顺序 + 补 audit 阶段（纯文档，先于重构） | 文档=代码 |
| 3（去 include!） | service_modes/service_postprocess 改真私有子模块，pub 面收缩（只留 execute_chat_pipeline/execute_chat_stream + lane helpers） | 10 个 dispatch_* 测试保绿；ADR-0007 单入口不变 |
| 4（减法） | 删 eval 机件 + 死 builders + re-export | grep + `cargo test -p app-chat` 绿 |

决策点（执行窗口定）：chat_private 3 个漏 pub 收进 chat 还是显式留 pub——看调用者（agent_runtime / app-bootstrap）归属。

---

## 3. 跨候选治理收尾（顺手做）

- **补录 ADR-0009**（Retrieval Bridge）：CONTEXT.md 引用但 `docs/adr/` 无此文件——补录或修正引用（W1 时一并）；
- **ADR 编号/文件名对齐**：文件名 0005 内容标题为 ADR 0004（W4-C4 时）；`policy.rs:4` 引用 §5a 与 `0006-unified-agent-loop-revised.md` 文件名（W4-C7 时）；
- **prompts 硬规则**：C5 的 `[block n]` 线格式（LLM 可见）规范写入 `prompts/capabilities/knowledge-base/SKILL.md`；任何新增 LLM 可见文本一律落 `avrag-rs/prompts/**/*.md`，第三人称观察式，不内联 Rust；
- 每波结束：`code-review-graph update`（WSL，不提交 .code-review-graph/）。

## 4. 执行窗口交接说明

1. 先读 `AGENTS.md` 与本计划 §1 决策表；报告快照的行号可能已漂移，**先核实再动手**。
2. 严格按切片顺序，验证门不过不前进；行为变更切片（W2 切片 3、W3-C5 的 image: 补齐、W3-C3 的显式错误）必须先把现状锁成测试。
3. 验证默认：`cargo test -p <pkg> --lib`（相关包）；每波末 `bash scripts/test-l1.sh`；WSL `jobs=2`，不叠加并发全量 cargo test。
4. 本地 trunk 提交，不 push 不开 PR；commit 信息遵循仓库惯例。
5. 遇计划与代码事实冲突：停下，把冲突写回本计划「状态」行并上报，不即兴改设计。

---

## 5. 审查记录（review 窗口回填）

### W1 · C1 — 已交付已审（commit 58d34c74，2026-08-02）

双轴审查结论：Standards 无硬违规（prompts-in-md / T1 / T7/T8 均合规）；Spec 核心目标达成（intersect 全仓单点、HTTP 面走 scoped 入口、contract 测试真实锁成功路径、`dispatch_rag_tool` 零引用）。**返工 3 项**：

1. **必修（安全语义）**：`rag_execute.rs workspace_doc_scope()` 上游 `app-documents list_documents` 在 store 缺失/出错时返回空 vec，而 `intersect_doc_scope` 把空 scope 当「不强制」——存储故障 fail-open 关闭 scope 强制。需区分「工作区确实无文档」与「读取出错」，出错 fail-closed。同时修「调用方显式给 doc_scope 但相交为空 → 回退到全量 Completed 集合」的交互：显式 scope 相交为空应为空集。
2. **补完切片 4**：`ToolExecKind::Rag` 变体（catalog.rs:80,98）+ match arm（tool_registry.rs:148，被改作 reject 用途）仍在——彻底删除（reject 前置已覆盖）；并删本次失去唯一生产调用者的 `runtime.rs execute_tools` → `tools/mod.rs dispatch_all` 无 scope 入口（reviewer 补充许可，正是 C1 初衷）。
3. **可选**：`OrchestratorContext::set_rag_runtime` 收到 test-support feature-gate 后。

判断级记录（不强制）：`test_set_rag_runtime` 手工重同步 `chat.orchestrator` 的 clone-and-resync 形态，持有者增多时脆弱。

### W5 · C6 — 已交付已审（commit dc5876d0，2026-08-02）

双轴审查结论：Standards 无硬违规（删除的 prompts eval 文件属清除既有 prompts-in-md 违规；include!→mod 无可见性泄漏；ADR-0007 pub 面未动）；Spec 切片 2/3/4 全达成（文档=代码逐行一致、删除物 grep 零调用者、`estimate_token_count` 保留、生产零行为变更）。**小修 1 项 + 记录 1 项**：

1. **小修**：脊柱测试（pipeline_tests.rs:677-740）与 `chat/mod.rs` 文档虚标——测试只能锁 `audit → persist` 与 Start-first/Done-present，**锁不了「Done 在 persist 之前」**（事件流 await 后才比对）。二选一：测试真锁 Done-before-persist，或把测试名/文档声明降级为「audit→persist 已锁；Done 先于 persist 由代码检视保证」。倾向后者。
2. **记录**：chat_private 3 个漏 pub（`build_rag_session_context`/`get_user_usage_limit`/`memory_session_visible`）决策点被静默搁置——暂记为「显式留 pub（现状默认）」，W2 动 chat_private 时一并定夺。
3. 记录在案不返工：删 13 个 builders 超出计划字面清单 12 个（多删者均验证零调用者）；单 squashed commit 使「文档先于重构」顺序不可验证；`RecordingChatPersistence` 100 行纯委托 signal `ChatPersistencePort` 是胖接口（未来收窄 port 的候选）；测试内 `std::sync::Mutex` in async 宜换 `tokio::sync::Mutex`。

### 本窗口（2026-08-02 执行窗口）补记

**W1 返工复核（commit `a051223f`）— 审查 3 项全部落地，免返工**：
1. **必修（安全语义）** 已落地：`workspace_doc_scope()`（app-chat/rag_execute.rs:44-51）改为 `Result`——仅当 auth 无 workspace_id 返回空 vec；store 缺失/出错经 `require_document_store`（app-documents/document_context.rs:15-21）→ `Err` 传播（fail-closed）。`intersect_doc_scope`（rag-core/scoped_rag_dispatch.rs:28-41，全仓唯一）空 scope→原样放行（上游不强制）；caller 非空但相交空→返回空集；`dispatch_scoped`（:86-103）对显式越界 caller 短路返回 `empty_scope_result`（:108-125），不回退全量 Completed。bridge.rs:111-116 共享同一实现。
2. **切片 4 删除** 已落地：`ToolExecKind::Rag` 变体（catalog.rs 仅剩 Skill）、`dispatch_rag_tool`、`runtime.rs execute_tools` / `tools::dispatch_all` 全仓 grep 零引用；`runtime/execute.rs` 已 cfg(test) 门控。
3. **可选项** 判定「现状已满足，跳过」：`set_rag_runtime` 方法无 gate，但 app-bootstrap 的 `test_set_rag_runtime` 包装已在 `test-support` feature 下（lib.rs:566-567）。

**W5/C6 小修已落地（本窗口）**：脊柱测试名 `pipeline_spine_locks_terminal_events_before_persist_and_audit_stage` → `pipeline_spine_locks_audit_before_persist`（降级声明：只锁 audit→persist + Done 存在；Done-before-persist 由代码检视保证）；`chat/mod.rs` 文档锚点同步降级并注明测试不锁 Done 位置。`cargo test -p app-chat --lib` 75 passed / 0 failed。

**C2-pub 定夺（C6 记录项结案）**：
- `get_user_usage_limit`（chat_private/quota.rs:7）——真实跨 crate 调用（app-bootstrap → transport-http），**显式留 pub**。
- `build_rag_session_context`（chat_private/memory.rs:7）——同 crate agent_runtime + 跨 crate 测试，**显式留 pub**。
- `memory_session_visible`（chat_private/visibility.rs:11）——全仓零调用者（死 pub），**已删除**（自然减法，编译通过）。
- 附带发现（W2 审查时请留意）：worker 日合并任务 `agent_memory_jobs.rs:185,228` 每跑必戳 `inferred_at=now`，是当前唯一会重置 dream 24h 闸门的第三方写入者。

**治理项核实（§3）**：
- ADR-0009 补录**已完成**：`docs/adr/0009-retrieval-bridge.md`（沙箱 codegen → 宿主 RAG 的 fd 管道 RPC，对齐 CONTEXT.md 引用）。
- ADR 编号/文件名错位（0004-desktop 标题写 ADR 0003、0005-llm 标题写 ADR 0004）：**核实为历史冲突**——e2e 语料（07-05）问「ADR-0004」指 LLM Provider/Agent Loop（即 0005 标题），DESKTOP 审计（07-14）说「ADR-0004」指桌面混合（即 0004 文件）。改标题会破坏语料/正文引用，**需刻意重排决定，本窗口不动**。
- policy.rs §5a 引用：**已成立**——实际引用在 agent-tools/capability/api.rs:4「ADR-0006 §5a」，对应 `0006-product-architecture-decisions-post-tn.md` 真实 `### 5a` 节；计划旧注（0006-unified-agent-loop-revised.md 文件名）已随 W4-C7 规整，无需处理。

**待双轴审 commit 范围**（已提交未审，含并行窗口历史）：W1 返工 `a051223f`；W2/C2 `eb1c972f`；W3/C3 `c7458770`；W4/C7 `2434df10`；W4/C4 `7d23423a`；另 C5 整波 + C6 小修由本窗口提交后并入。

**W3-C5 引用语法 — 切片 1–3 已交付（本窗口），切片 4 延后**：
- **切片 1+2（golden + 收拢）**：rag-core 新建 `runtime/markers.rs` 统一 grammar——`extract_markers`（`[[cite:]]`/`[[image:]]`/`[[web:n]]`/裸 `[[n]]` 分类）+ `extract_chunk_ids`（cite+image，决策 3：image: 不再漏）+ `extract_web_indices`（复刻两趟序：web 先、裸后）+ `format_block`（`[block n]` 成功行唯一生产者）+ 内联 golden 测试。消费方全部改委托：`response_utils.rs`/`cite_extract.rs`/`answer_contract.rs` 的 `extract_cite_chunk_ids`/`extract_web_marker_indices`/`iteration_codegen.rs` 生产者。验证门：rag-core 116 + agent-loop 291 + app-chat 75 全绿。
- **切片 3（减法）**：删 `app-chat/prompts/citations.rs` 死拷贝 + 整个 `prompts`/`rag_prompts` 死 re-export 模块（全仓零消费者，grep 复核）；删 `answer_contract` 真死 pub 项 `known_chunk_ids`、`collect_synthesis_validation_errors`（零调用者）。其余「仅内部自用却 pub」项留待切片 4 分相时随私有子模块自然收敛。
- **线格式规范**：`[[cite:]]`/`[[image:]]`/`[[web:n]]`/`[block n]` 已写入 `prompts/capabilities/knowledge-base/SKILL.md`（prompts 硬规则）。
- **切片 4（分相）已交付（本窗口）**：L1 文件大小门（>1000 行硬限）要求 `answer_contract.rs` 必须分解——拆为目录模块 `answer_contract/`：`mod.rs`（740 行，facade + lift/validate/salvage + pub API re-export）、`parse.rs`（430 行，structs + 解析/归一/升级/标记提取）、`final_answer_rules.rs`（~193 行，终答格式规则簇 + DRAFT_REFUSAL_CUES）、`tests.rs`（391 行，测试移出）。跨模块共享项升 `pub(crate)`，外部 pub 面由 mod.rs 精确 `pub use` 保持原样。验证门：agent-loop 291 全绿；`check_file_size_limits.sh` hard_failures=0（从 allowlist 移除已分解的 answer_contract.rs 条目）。
- **顺手硬化（既有 flaky 竞态）**：host_markers.rs 两测试（`every_md_tag_candidate_is_registered` 扫描 `prompts/loop/*.md` vs `parity_fails_on_unregistered_md_tag` 写临时 probe）并行竞争致偶发失败——加共享 Mutex 串行化，6/6 稳定。

### 双轴审查记录（review 窗口，2026-08-02 第二批：W1返工 / W2 / C3 / C7 / C4 / C5 / C6小修 / docs）

**总判**：7 个提交全部「验收+小修」或「验收」，无推倒重做项。返工按优先级归并到本节末清单。

**W1 返工 `a051223f` — 验收**（review 窗口逐行复核）：fail-closed 三件套（`workspace_doc_scope` 改 Result 传播存储错误；空相交返回空集不回退；`dispatch_scoped` 对显式越界 caller 短路空结果且防「空 doc_scope=org-wide」）全部落地并有测试；`ToolExecKind::Rag` 变体+arm、`execute_tools`/`dispatch_all` grep 零引用。可选项 3（`set_rag_runtime` feature-gate）未做、判定可接受（`test_set_rag_runtime` 已在 test-support 下）。

**C6 小修 `9ba6e735` — 验收**：测试名/文档声明降级与审查要求逐字对应；`memory_session_visible` 死 pub 删除，另两 pub 显式留（C2-pub 定夺结案）。

**docs `743bd77f` — 验收**：ADR-0009 内容与技术事实相符（HostBridge fd3/fd4、RuntimeBridge、唯一 intersect 复用）；ADR 0004/0005 编号错位核实为历史冲突，**挂账待刻意重排**（e2e 语料与 DESKTOP 审计对「ADR-0004」指代不同，改标题会破坏引用）。

**W2·C2 `eb1c972f` — 验收+小修（含 1 项必修）**：
- Spec：切片 2/3 落地（profile_update 单模块、general-v1 grep 零引用、闸门单测真实断言、admin 保存不再戳 inferred_at 且有测试锁）。**必修**：worker 日合并 `agent_memory_jobs.rs:185,228` 每跑必戳 `inferred_at=now`——dream 24h 闸门在生产被持续重置，dream-v2 实际永不触发，「dream-v2 唯一写入者」名不副实，决策 2 语义未真正达成。切片 1 行为锁属部分交付（无死锁复现测试，记为已知缺陷）。
- Standards：无硬违规。最重：`is_direct_chat_mode` 双份私有拷贝（profile_update/mod.rs:25 + service_postprocess.rs:273）应合一；`maybe_update_profiles` 返回 Result 但永 Ok（无效失败面）；存量 `user-profile-extraction.system.md` 第二人称指令体（非本提交引入，记入 prompt 债）。

**W3·C3 `c7458770` — 验收+小修**：
- Spec：四切片实质达成（真死文件删除判断正确——计划快照误标 core.rs/tier.rs 为死文件，实际非死，执行方判断对；process.rs 改写为 ProviderEvent 消费层而非删除，意图达成；Alipay 验签不对称/CJK percent-decoding/cents 比较均有真实测试）。问题：commit message 测试计数 17 vs 实际 14；两个未声明的小行为变更（错误码归并）。
- Standards：surgical 越界 1 处——`contracts/tests/module_fixtures.rs` 整文件删除连带砍掉 2 个与 billing 无关的 fixture 测试（workspace_list / admin_health），**应恢复这两个测试**；`adapter_for` 的 `panic!` 在金钱 webhook 热路径留未来可触发的崩溃点，**应改显式错误**（最重）；handle_webhook 双 arm 逐行重复可合并。

**W4·C7 `2434df10` — 验收+小修**：
- Spec：派生等价回归测试扎实（5 auth 态 × 全工具对拍）。问题：一致性锁测试 risk 半边同义反复（单源化后永不可失败，「锁」名不副实）；registry.rs:294 三个不可达死 match 臂；`standard_rules_cached` 死导出；rag 工具 risk 投影 Low→Medium 行为变化未在 commit 标注（属修复漂移，应补注+测试锁投影值）。
- Standards：最重——派生循环 `_ => continue` 静默跳过 User/Advanced/Admin 权限类型，未来工具挂这些权限将无声失去闸门，且等价测试同盲区；建议穷尽匹配或显式注释执法范围。双 OnceLock 重复构建规则集（轻微）。

**P1-2 已落地（本窗口）**：
- `policy.rs` 派生循环 `_ => continue` 改**穷尽匹配**：User/Advanced/Admin 显式列出并注释（RBAC 档位由 auth 层执法，非工具级能力闸门；新增 Permission 变体必须在此显式放置）。
- `registry.rs` `infer_skill_risk_level` 删除三个不可达死 match 臂（code_interpreter/web_search/web_fetch 已注册，经 `tool_meta` 解析）。
- 删除 `standard_rules_cached` 死导出 + `STANDARD_RULES` OnceLock + mod.rs/lib.rs re-export。
- **rag risk Low→Medium 补注**：`2434df10` 的 rag 工具 risk 投影由 `infer_risk` 默认 Low 改为 catalog 单源 Medium（修复既有漂移），属有意的行为对齐；此补注即 commit 缺失标注的补录。`cargo test -p agent-tools` 145 绿。

**W4·C4 `7d23423a` — 验收+小修**：
- Spec：切片 1/2/4 全达标（usage 转换全仓唯一、pool 测试离网 109 绿、死面 grep 零残留）；切片 3 暂缓被严格遵守，failover/try_each 语义未动。
- Standards（与 Spec 轴同指最重）：`route/client.rs:225` 流式分支 `panic!` 处理 TransportBody 契约违例——同函数非流式分支返回 `LlmError::protocol`，不对称；任何自定义 Transport 实现者的一个 bug 即 abort 进程，**应改 Err 返回**。`complete_stream_openai` 与 `Route::complete_stream` 平行重复（Feature Envy 残留，可留后续）。

**W3·C5 `3a711d48` — 验收+小修**：
- Spec：golden 6/6、三份 extract 归一、决策 3（image:）有显式测试、SKILL.md 线格式与代码实际格式一致、分相 pub 面逐项无丢失。缺口：① 切片 2 验证门具名项 `exit_policy.rs:112` 仍手写 `[block n]` 解析（markers 只有产出侧 `format_block`，缺 parse 侧 API）——线格式仍有第二实现；② 「删 4 个死 pub 项」只删 2 个，`strip_json_fences`/`template_artifact_matched`/`executable_code_matched`/`host_shell_matched`/`known_chunk_ids_with_messages` 仍 pub 且零外部调用者。host_markers Mutex 顺手修复诊断正确、修法最小，认可。
- Standards：无硬违规（SKILL.md 全第三人称陈述，合规）。存量债记录：answer_contract 内 `synthesis_contract_block`/`feedback_hint` 中文化是 LLM-facing 散文硬编码于 Rust（字节级存量搬迁，本波不追责，记入 prompt 债）。

#### 归并返工清单（执行窗口领走）

- **P0-1（W2，语义必修）**：worker `agent_memory_jobs` upsert 不戳 `inferred_at`（保留既有值），让 dream 24h 闸门真实生效；补「worker 写入不重置 cadence」测试。
- **P0-2（C3）**：`service.rs adapter_for` 的 `panic!` 改返回显式 `ProviderError`。
- **P0-3（C4）**：`route/client.rs` 流式分支 `panic!` 改 `LlmError::protocol`，与非流式分支对齐。
- **P1-1（C3）**：恢复 `module_fixtures.rs` 中被连带删除的 workspace_list / admin_health 两个 fixture 往返测试。
- **P1-2（C7）**：派生循环 `_ => continue` 改穷尽匹配（或显式注释执法范围仅 ExternalNetwork/CodeExecution）；删 registry 死 match 臂与 `standard_rules_cached` 死导出；commit/文档补注 rag risk Low→Medium 变化。
- **P1-3（C5）**：markers.rs 补 parse 侧 API（parse_block 行），exit_policy.rs:112 手写解析改委托；删 answer_contract 剩余死 pub re-export（5 项）。
- **P1-4（W2）**：`is_direct_chat_mode` 双份合一；`maybe_update_profiles` 的 Result 签名收窄或让 list_messages 错误真实传播（二选一）。
- **P2（记录，不阻塞）**：ADR 0004/0005 编号刻意重排（需动语料/审计引用，单独决策）；prompt 债两笔（`user-profile-extraction.system.md` 第二人称 voice；answer_contract 内 `synthesis_contract_block`/`feedback_hint` 内联散文迁 prompts/）；C3 commit message 测试计数 17→14 木已成舟，以此条为准。

**P0×3 + P1×4 已全部落地（本窗口，2026-08-02）**：
- **P0-1**：`agent_memory_jobs` 两处 upsert 提为共享 `PROFILE_UPSERT_SQL` const，`on conflict` 去掉 `inferred_at = excluded.inferred_at`（保留既有值）；补 SQL 回归守卫（PG-free）+ 真 PG 集成测试（种子 25h 旧 inferred_at → 跑 upsert → 断言不变，含 RLS/FK 种用户）。`cargo test -p avrag-worker` 33 绿。
- **P0-2**：`ProviderError` 加 `Unsupported` 变体；`adapter_for` 返回 `Result<&dyn PaymentProvider, ProviderError>`，4 调用点（checkout×2/webhook×2）显式处理。billing 26 绿。
- **P0-3**：llm `route/client.rs` 流式分支 TransportBody 契约违例由 `panic!` 改 `Err(LlmError::protocol(...))`（经 try_stream `?` 路径），与非流式分支对齐。llm 109 绿。
- **P1-1**：恢复 `contracts/tests/module_fixtures.rs`（workspace_list + admin_health 往返测试；billing_plans 随 C3 合同重构正确退役）。contracts 15 绿。
- **P1-2**：`policy.rs` 派生循环穷尽匹配（User/Advanced/Admin 显式列出 + RBAC 注释）；`infer_skill_risk_level` 删 3 不可达死臂；删 `standard_rules_cached` 死导出 + STANDARD_RULES OnceLock + re-export；rag risk Low→Medium 补注见 W4·C7 记录。agent-tools 145 绿。
- **P1-3**：`markers.rs` 补 parse 侧 `parse_block`/`Block`（+ round-trip/error/truncated golden）；`exit_policy.rs:112` 手写解析改委托；answer_contract 删 5 个死 pub re-export（strip_json_fences/template_artifact_matched/executable_code_matched/host_shell_matched/known_chunk_ids_with_messages 降级）。rag-core 120 + agent-loop 291 绿。
- **P1-4**：`is_direct_chat_mode` 合一（profile_update 唯一源，service_postprocess 删副本）；`maybe_update_profiles` Result→bool（尽力而为，list_messages 错误显式吞并注释），调用点去 `?`。app-chat 75 绿。
- 验证：`test-l1`（worker 33 / billing 26 / llm 109 / contracts 15 / agent-tools 145 / rag-core 120 / agent-loop 291 / app-chat 75）**L1 OK**；`cargo check --workspace` 0 error。
