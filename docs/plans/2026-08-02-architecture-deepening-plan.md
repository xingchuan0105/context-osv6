# 架构加深优化计划（2026-08-02）

| 项 | 内容 |
|---|---|
| 日期 | 2026-08-02 |
| 状态 | **待执行**（决策已全部拍板，见 §1） |
| 来源 | 架构审查报告 `/tmp/architecture-review-1785639147.html` + 深潜报告 `/tmp/architecture-deepdive-1785640412.html`（7 候选，均含现状接口/问题/加深设计/切片/测试存活/ADR 约束） |
| 范围 | `avrag-rs`：rag-core / agent-tools / app-chat / agent-loop / llm / billing |
| 不做 | push/PR/CI；failover 统一（C4 切片 3，留到有真实流式故障复现）；`RuntimeExecuteRequest` 加 `doc_scope` 字段（决策 1） |
| 约束 | T1–T8；T5 行为保持切片、验证门不过不前进；prompts-in-md；第三人称观察式 prompt；solo 本地 trunk；WSL `jobs=2` 不叠加并发全量 cargo test；每波结构性改动后 `graphify update .`（不提交 graphify-out/） |
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
- 每波结束：`graphify update .`（WSL，不提交 graphify-out/）。

## 4. 执行窗口交接说明

1. 先读 `AGENTS.md` 与本计划 §1 决策表；报告快照的行号可能已漂移，**先核实再动手**。
2. 严格按切片顺序，验证门不过不前进；行为变更切片（W2 切片 3、W3-C5 的 image: 补齐、W3-C3 的显式错误）必须先把现状锁成测试。
3. 验证默认：`cargo test -p <pkg> --lib`（相关包）；每波末 `bash scripts/test-l1.sh`；WSL `jobs=2`，不叠加并发全量 cargo test。
4. 本地 trunk 提交，不 push 不开 PR；commit 信息遵循仓库惯例。
5. 遇计划与代码事实冲突：停下，把冲突写回本计划「状态」行并上报，不即兴改设计。
