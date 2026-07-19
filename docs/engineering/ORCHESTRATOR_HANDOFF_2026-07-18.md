# Handoff: Agent 编排器（Orchestrator + Subagents + 共享证据库）工作流

**日期:** 2026-07-18（当日多次更新，覆盖至 R8）· **续：** 2026-07-19 Wave 0 提交 + Wave 1a/1b coverage · **状态:** V1 + V2 已落地；**结构化 worker handoff + prompt coverage 已落地**；基准文档 reindex 已含 triplet（entity 254 / relation 195）· **主线分支:** `master`（本地 trunk）

---

## 1. 这条线在做什么

把 `avrag-rs` 后端的 agent 范式从"三个平行 agent 显式切换"重构为 **主编排器（ReAct loop）+ 子代理能力**：

- 产品表面：能力标签只有 **RAG**（工作区检索）/ **Search**（网页检索），可多选；不选 = 纯聊天。
- 内部：Orchestrator（只编排，不检索不写答案）→ RagWorker / SearchWorker（只取证，产出证据）→ Chat exit（唯一用户出口，Option B）。
- 硬约束（设计锁定）：§7.1 物化通道不可被 LLM 取消；§7.2 每个物化通道至少派发一次才能 delegate_chat；最终回答只由 Chat exit 产出；**不允许新增基于规则的去语境化/查询改写代码**。
- 产品规则（2026-07-18 新增）：**空选择 = RAG 不可用**——用户未选文档时前端禁用 RAG 并提示，后端不注入任何全量文档清单/摘要（见 §6.3）。

## 2. 关键文档

| 文档 | 内容 |
|---|---|
| `docs/engineering/ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md` | 原始设计（§7 三层、Option B） |
| `docs/engineering/ORCHESTRATOR_V2_REACT_EVIDENCE_STORE_DESIGN_2026-07-18.md` | **现行架构总纲**：ReAct 编排器 + 共享证据库 + 波次定义 |
| `avrag-rs/prompts/orchestrators/orchestrator-base.md` | 编排器系统 prompt（brief 写作原则） |
| `avrag-rs/prompts/clusters/codegen/SKILL.md` | RAG worker 的检索 SDK（7 个 client 方法，触发条件在此） |

## 3. 代码结构（`avrag-rs/crates/app-chat/src/orchestrator/`）

| 文件 | 职责 |
|---|---|
| `host.rs` | V1 结构化 host；`AgentServiceExecutor` 生产执行器（worker 装配在此：`assemble_mode` + brief 注入） |
| `brain.rs` | **V2 ReAct 编排器**：LLM 工具 = `delegate_rag/search/chat`、`evidence_fetch`、`conversation_history_load`、`user_profile_load`（全部 host 拦截）；并行 wave；护栏（finish-gate/去重/单通道上限 2/预算兜底）；system message 每轮刷新（通道状态 + 源文档 + 用户画像 + 预算） |
| `store.rs` | **共享证据库**：`E{n}` 单调编号；**TOPK 硬闸**（RAG 24 / Search 12）；`EvidenceKind::DocProfile` 定向条目（doc_profile/doc_summary 输出，豁免 TOPK，`targeted_entries()`） |
| `chat_exit.rs` | Chat brief：源文档身份 + 通道结果 + worker 摘要 + **文档定向段**（全文，do NOT cite）+ 可引用清单（排除定向条目）+ 引用策略 |
| `workers.rs` | worker 摘要（≤2000 字）；`finalize_answer_evidence`：`[[E:id]]` → `[[cite:chunk]]`/`[[web:n]]`，**单一全局出现序 citation 计数器**；悬空剥离 + warn；定向条目引用静默剥离 |
| `invariant.rs` / `materialize.rs` / `types.rs` | §7.2 检查；default_brief（零策略透传）；通道物化；ChatHandoff（含 `targeted` 字段） |

接线：`pipeline_steps.rs::run_orchestrator_v1` → `run_orchestrator_turn`（V2 flag + llm → brain）。**同处注入三样**：docscope（给 store + `agent_request.docscope_metadata` 给 worker）、`state.chat_persistence()`（给 brain 的记忆工具）、orchestrator llm（`UnifiedAgentService.with_orchestrator_llm`）。

## 4. 生产验证记录（同一 query："这篇转型报告和最佳实践的差距在哪里？"，doc_scope=《数字化转型IT立项报告》148 chunks）

| 轮次 | 结果 | 关键发现 |
|---|---|---|
| R1–R3 | 文体误读→修正、引用编造→消除、search 空转（Brave DNS 污染，需代理） | 文档身份缺失、证据洪水（148 全量）、口径丢失 |
| R4 | search 首产 12 条；但只覆盖"基础设施选型"一章 | **主因：`doc_profile` 被单数 `doc_id` 调用遭 `deny_unknown_fields` 拒绝；定向输出不入库** |
| R5 | **合格**：总体判断 + 六维度逐章对比；17 引用 + 12 网页 | R1–R3 生效；答案详细程度存在轮间方差（见 §5.1） |
| R6 | 回答变表格版（详细程度差异） | **引用编号撞号实锤**：doc/web 双计数器同号，lookup 错配 → 前端【加载引用片段失败。】 |
| R7 | rag 入库 17 条（↑15）；5092 字；六维度 + 建议 | 记忆工具/画像/worker docscope 上线；本轮 brain 未调记忆工具（正常空转） |
| R8 | 正常（含理解口径） | 空选择产品规则上线后回归正常 |

## 5. 当日诊断结论（分析成果，后续设计以此为据）

### 5.1 答案详细程度方差的四层来源（R5 vs R6 实测）

一轮 turn 内最多 4 个独立 LLM 循环（orchestrator / rag worker / search worker / chat exit），方差逐跳累积：

1. **取证层（最大源）**：worker 自写检索子查询 → 两轮入库 chunk 仅 3/15 重叠 → 回答维度选择被入库证据决定，而非被文档结构决定；
2. **编排探索层**：evidence_fetch 轮数不定（R5=2 轮，R6=1 轮）；
3. **口径层**：`delegate_chat` instruction 每次现写且不落盘——详细程度最直接的开关；
4. **成文层**：chat 出口体裁自由（散文三段式 vs 表格）。

结论：靠压 worker 温度（当前 0.3）只能减漂移，且会收敛到"众数策略"可能系统性漏维度；真正的抓手是 **coverage 显式化**（见 §8 欠账）。

### 5.2 docscope 曾未加载到 worker（已修，根因链）

`build_agent_request` 硬编码 `docscope_metadata: None`；经典 RAG 路径在 `run_rag_mode` 事后补上，orchestrator 路径从未补 → worker 的 `metadata` skill 渲染时 `disclosure_plan.rs::inject_cluster_runtime_context` 拿到 None，`<docscope_metadata>` 块为空，worker 盲跑。修复：`run_orchestrator_v1` 赋值一行。

### 5.3 codegen 面完整，功能未丢失（用户关切澄清）

`modes/rag.yaml`：`tool_pool: []` 故意为空（检索只走 codegen），`skill_catalog.retrieve: [codegen, memory, metadata]`。orchestrator worker 经 `assemble_mode(rag)` 完整继承。bridge 7 方法全在（`iteration_codegen.rs`）：`dense_search/lexical_search/graph_search/chunk_fetch/doc_profile/doc_summary/doc_chunks`。**唯一缺口是数据**：`INGESTION_TRIPLET_ENABLED=0`（dev 环境），triplet 未抽取，`graph_search` 无数据可查——开 flag 重新入库即可。

### 5.4 文档身份机制与"空选择"规则

文档身份 = 入库时 LLM 生成 `SummaryMetadata`（genre 等）存 PG，请求时聚成 `DocScopeMetadata`，代码只做确定性渲染（非硬编码内容）。用户裁定其"未选也读"不合理 → **空选择 = 不给全量**：

- codegen bridge 本来就拒绝空 doc_scope 定向调用（`bridge.rs` 三处 `doc_ids is required when doc_scope is empty`）；
- `response.rs::apply_summary_policy` 的 `summary=all` 兜底已收窄（不再 `list_documents` 全工作区注入）；
- 前端：空选择禁用 RAG + 提示 + 自动摘除已开启的 RAG（`workspace-chat-pane.tsx` / `chat-composer.tsx` / i18n `workspaceChatCapRagNeedsSources`）。
- **连带影响**：shared KB 分享页固定发 `doc_scope: []`，今后只有 hit 文档摘要（`related`）；如分享回答质量下降需单独处理。

## 6. 已完成的修复

### 已提交（commit）

`64ea155` O1 编排器 · `ddb46ce` 前端进度恢复 · `3426fd4`/`5e0d036`/`6a6d435` 文档 · `7e7855d` V1 证据库 · `0b714d4` 策略入 prompt

### 未提交（工作树，orchestrator 批次）

- **V2 brain + F1–F4**（F1 工具失败记 Error≠Empty；F2 TOPK 闸；F3 worker query=brief.goal；F4 instruction 必写口径）
- **R1 `doc_id` 单数容错**：4 个工具入口 + `contracts::lib.rs` 重导出 `normalize_doc_id_alias`
- **R2 store 定向捕获**：`EvidenceKind::DocProfile`，豁免 TOPK，按 doc+text 去重
- **R3 chat brief 定向段** + 可引用清单按 kind 排除 + 定向引用静默剥离
- **引用编号单一计数器**（`workers.rs::rewrite_markers`）：citation_id 全局唯一，`[[web:n]]` 与数组位置对齐（修【加载引用片段失败。】）
- **brain 记忆工具**：`conversation_history_load` / `user_profile_load`（host 拦截，复用 `agent_tools::skills::memory_dispatch`；无存储时不进工具面）
- **brain 用户画像注入**：紧凑三字段（expertise_domains / preferred_answer_style / frequently_asked_topics），刻意不注入 raw `custom_preferences`
- **worker docscope 接线**（§5.2）
- **空选择产品规则**（§5.4，前端 + `response.rs`）
- 附带：`contracts/tests/module_fixtures.rs` 过期 fixture（T8 遗留）；`rag-core/runtime/tests.rs` 的 `ChatRequest` 缺字段
- **切勿混入**：他人 Alipay billing 改动（billing*/app-bootstrap billing/transport-http billing/前端 pricing 等约 20 文件）

### 验证基线（当前全绿）

`cargo test --lib -p app-chat` **134** · `-p avrag-rag-core` **52** · `contracts` 全绿 · 前端 `pnpm vitest run tests/workspace/` **152** + `tsc --noEmit` 净 · `cargo build -p avrag-api` 通过

关键回归测试：`memory_tools_and_user_profile_reach_brain`（brain 录制断言）、`citation_ids_are_unique_across_doc_and_web`（撞号）、`doc_profile_and_summary_become_targeted_entries` / `targeted_entries_dedupe_and_survive_topk`（R2）、`targeted_entry_markers_stripped_silently`（R3）、前端 `disables RAG with a hint…` / `strips RAG when the selection becomes empty`。

## 7. 下一步（按优先级）

1. ~~**提交拆分**~~ **Done 2026-07-19**：`6fc8977` orchestrator V2 批次；`2dc00e3` Alipay billing 批次。
2. ~~**coverage 显式化**~~ **Done 2026-07-19**：
   - **1a** `orchestrator-base.md`：coverage/gaps 驱动 re-dispatch；对比类 query 要求对照文档定向逐章核对。
   - **1b** `WorkerHandoff` / `internal_worker_handoff_v1`：`workers::parse_worker_handoff` + chat brief 渲染 + delegate 结果字段；free-form → `coverage=partial`。
3. ~~**triplet**~~ **Done 2026-07-19**：`INGESTION_TRIPLET_ENABLED=1`；文档 `886de4b1-…` reindex 完成，`entity_count=254` / `relation_count=195` / `graph_passage_count=195`（先前为 0）。
4. ~~**安全 key 轮换（配置侧）**~~ **Done 2026-07-19**：`TRIPLET_LLM_API_KEY` 已改为与 `INGESTION_LLM_API_KEY` 同值（配置不再使用曾暴露的独立 secret）。**仍需你在 DeepSeek 控制台 revoke 旧 TRIPLET 专用 key**（平台侧失效，本地无法代办）。
5. worker 温度（0.3 → 0~0.1）：可选的战术手段，单独做、复测两轮对比方差；未做。

## 8. 环境与运行

- 起栈：`bash scripts/ci-start-milvus.sh && bash scripts/product-dev-up.sh`（tmux `context-os-dev`：minio/office/api/worker/next；前端 3000 / api 8080）
- api 重启姿势：`tmux kill-window -t context-os-dev:api && tmux new-window -t context-os-dev -n api "cd '/home/chuan/context-osv6/avrag-rs' && set -a && source .env && set +a && export CARGO_TARGET_DIR='/home/chuan/context-osv6/avrag-rs/target' && exec cargo run -p avrag-api 2>&1 | tee -a '.dev-logs/api.log'"`
- Flags（`avrag-rs/.env`）：`AGENT_ORCHESTRATOR_V1=1`、`AGENT_ORCHESTRATOR_V2=1`
- **代理（search 必备）**：Brave 需经 `http://172.27.240.1:20000`（WSL 网关 IP，网络重置后需更新）；国内 LLM 端点走 `NO_PROXY` 直连
- 复测调用：`POST /api/v1/chat`，headers `x-user-id: a2f174ff-af95-4b87-be14-0f91340d39bf` + `x-permissions: read,write,external_network`，body 带 `capabilities:["rag","search"]`、`doc_scope:["886de4b1-5abc-426f-9206-3a639950ffb7"]`、workspace `9e3abf9d-cae9-43d2-882c-d27c05969c66`；历史 SSE 在 `output/retest_r{5..8}_sse.log`
- 日志：`avrag-rs/.dev-logs/api.log`（`orchestrator round` / `dispatch finished` / `TOPK gate` / `dangling`）；worker 内部迭代不进 INFO 日志
- 图谱：结构性改动后 `graphify update .`

## 9. 欠账（明确延期）

- ~~Worker 结构化交接契约~~（2026-07-19 已实现；可继续加 eval 看模型是否稳定吐 JSON）
- Chat 出口流式化（当前收集后整段重放，分钟级任务有"冻结感"）
- search worker 空转预算行为（连续空结果应更早收敛）
- 编排器/worker 温度决策（§7.5）
- V3：引用校验 agent、对比类查询 eval 集、chat 成 loop（可选）
- **DeepSeek 平台 revoke 旧 TRIPLET key**（见 §7.4）

## 10. 不要踩的坑（本线实测）

- 策略文本住 prompt 文件，代码只留确定性约束（§7、store、标记重写、端口接线）
- 不要绕开 TOPK 闸；不要把全量检索结果直接入库
- 空选择语义已改：不要再发明"全工作区默认注入"的任何形式（§5.4）
- citation_id 必须全局唯一（doc/web 同空间编号），否则 lookup 错配复发
- 不要给 `write` 通道留 ToolCatalog 入口（T2）；workspace/org 术语红线见 `docs/agent/product-apps.md`
- 提交拆分：orchestrator 批次 vs 他人 billing 改动 vs 文档
