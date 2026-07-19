# Handoff: Agent 编排器（Orchestrator + Subagents + 共享证据库）

**日期:** 2026-07-18 起 · **验收更新:** 2026-07-19 · **主线:** `master`（本地 trunk，未 push）  
**状态:** **首轮验收完成（有条件通过）** · A4 流式 + 残缺标记两缺陷已修，待复验 A4/A5

---

## 0. 验收一页纸（先看这里）

### 0.1 本线目标

产品表面：能力标签 **RAG / Search**（可多选；不选 = 纯聊天）。  
内部：**Orchestrator（ReAct）→ RagWorker / SearchWorker（取证）→ Chat exit（唯一用户出口）**。

### 0.2 验收环境（当前）

| 项 | 值 |
|---|---|
| Flags | `AGENT_ORCHESTRATOR_V1=1` · `AGENT_ORCHESTRATOR_V2=1` · `INGESTION_TRIPLET_ENABLED=1` |
| 前端 | http://127.0.0.1:3000 |
| API | http://127.0.0.1:8080（health 应 200） |
| tmux | `context-os-dev`：minio / office / api / worker / next |
| 基准用户 | `x-user-id: a2f174ff-af95-4b87-be14-0f91340d39bf` |
| 权限 | `x-permissions: read,write,external_network` |
| Workspace | `9e3abf9d-cae9-43d2-882c-d27c05969c66` |
| 基准文档 | `886de4b1-5abc-426f-9206-3a639950ffb7`（《数字化转型IT立项报告…》148 chunks） |
| Search 代理 | Brave 经 `http://172.27.240.1:20000`（WSL 网关变了要更新） |

**验收前请确认 api 为含本线 commit 的 binary**（见 §0.3）。若仍是旧进程：

```bash
tmux kill-window -t context-os-dev:api 2>/dev/null || true
tmux kill-window -t context-os-dev:worker 2>/dev/null || true
tmux new-window -t context-os-dev -n worker "cd '/home/chuan/context-osv6/avrag-rs' && set -a && source .env && set +a && export CARGO_TARGET_DIR='/home/chuan/context-osv6/avrag-rs/target' && export RUST_LOG=\"\${RUST_LOG:-info,avrag_worker=info}\" && exec cargo run -p avrag-worker 2>&1 | tee -a '.dev-logs/worker.log'"
tmux new-window -t context-os-dev -n api "cd '/home/chuan/context-osv6/avrag-rs' && set -a && source .env && set +a && export CARGO_TARGET_DIR='/home/chuan/context-osv6/avrag-rs/target' && exec cargo run -p avrag-api 2>&1 | tee -a '.dev-logs/api.log'"
# 等 cargo 编完后 curl http://127.0.0.1:8080/health → 200
```

### 0.3 本线关键 commit（master 顶）

| Commit | 内容 |
|---|---|
| *(本批待提交)* | **A4 真流式**（chat/prose retrieve `complete_stream`）+ **残缺 `[[E` 不吞后续合法标记** |
| `2dad134` | 首轮验收结论写入 |
| `a2813d8` | 验收手册（本文件） |
| `92baa67` | Chat sink 接线 + search 空转早收敛（A4 未完全生效，见 §0.9） |
| `dc23543` / `64da91d` | Worker 结构化 handoff + coverage prompt |
| `6fc8977` | V2 brain + TOPK + 定向证据 + 引用单计数器 + 空选择规则 |
| `2dc00e3` | Alipay billing（旁线） |

更早：`64ea155` O1 · `7e7855d` V1 store · `0b714d4` 策略入 prompt · `ddb46ce` 前端进度。

### 0.4 自动化基线

| 包 / 套件 | 结果 |
|---|---|
| `cargo test -p app-chat --lib` orchestrator | **48** 绿（含 `broken_open_marker_does_not_swallow_following_valid_eids`） |
| `cargo test -p agent-loop --lib` | **183** 绿 |

关键单测名：

- `broken_open_marker_does_not_swallow_following_valid_eids`（缺陷 2）
- `citation_ids_are_unique_across_doc_and_web`
- `parses_structured_worker_handoff_json` / `structured_handoff_renders_coverage_and_gaps`
- `search_exhausted_after_two_empty_not_after_one`
- `empty_web_search_ok_is_not_evidence` / `consecutive_empty_search_triggers_early_stop`
- `memory_tools_and_user_profile_reach_brain`
- 前端：`disables RAG with a hint…` / `strips RAG when the selection becomes empty`

### 0.5 验收清单（建议顺序）

| # | 场景 | 操作 | 通过标准 |
|---|---|---|---|
| A1 | 纯聊天 | caps 全关 | 无 worker 进度；快速直答 |
| A2 | 空选择 + RAG | 不选文档，点 RAG | 前端禁用 + 提示 `workspaceChatCapRagNeedsSources`；已开 RAG 被摘掉 |
| A3 | 基准对比（主路径） | 选基准文档 + RAG+Search；问：「这篇转型报告和最佳实践的差距在哪里？」 | 见 §0.6 |
| A4 | 流式 | 同 A3，看 SSE/UI | **compose 阶段 token 陆续出现**，不是全程进度卡死后突然整段字；`done` 后引用芯片可点 |
| A5 | 引用 | A3 答完点引用 | 无【加载引用片段失败】；citation_id 不撞号 |
| A6 | 空 Search 收敛 | 可选：断 Brave 代理再开 Search | 不长时间空转；日志可有 `search_empty_early_stop` 或 brain 拒绝再 `delegate_search` |
| A7 | 空选择后端 | `doc_scope:[]` + `capabilities:["rag"]` | 不注入全工作区文档清单（无「默认全库」行为） |

### 0.6 A3 主路径质量标准（对照 R5/R8）

同一 query + 基准 doc：

1. **理解口径**开头有一句话（报告方案 vs 最佳实践）。
2. **文档身份**正确（立项报告类，非误读体裁）。
3. **多维度**覆盖（不止一章；对照文档结构/定向段）；缺口应明示。
4. **引用** doc `[[cite:…]]` + 有网时 web；数量合理；前端芯片可加载。
5. 日志（`avrag-rs/.dev-logs/api.log`）可见：
   - `orchestrator round`
   - `orchestrator dispatch finished`（rag / search）
   - 可选 `TOPK gate` / `dangling`（dangling 应为 0 或极少）
   - **不应**整段答案只在结束瞬间一次性刷出（流式后 chat 段应有 token）

复测 API 示例：

```bash
curl -sN -X POST 'http://127.0.0.1:8080/api/v1/chat' \
  -H 'content-type: application/json' \
  -H 'x-user-id: a2f174ff-af95-4b87-be14-0f91340d39bf' \
  -H 'x-permissions: read,write,external_network' \
  -d '{
    "query": "这篇转型报告和最佳实践的差距在哪里？",
    "workspace_id": "9e3abf9d-cae9-43d2-882c-d27c05969c66",
    "doc_scope": ["886de4b1-5abc-426f-9206-3a639950ffb7"],
    "capabilities": ["rag", "search"],
    "stream": true
  }' | tee output/acceptance_$(date +%Y%m%d_%H%M%S)_sse.log
```

历史对照：`output/retest_r{5..8}_sse.log`（若仍在）。

### 0.7 首轮验收结论（2026-07-19 · commit `2dad134`）

| # | 结果 | 关键证据 |
|---|---|---|
| 环境 | ✅ 已纠偏 | api/worker 原为旧 binary（10:01 编译 &lt; `92baa67`）；已重编至验收时 tip；自动化 47+12 重跑全绿 |
| A1 | ✅ | caps 全关 → user-chat 直答，SSE 无 orchestrator 痕迹 |
| A2 | ✅ | 前端 vitest 7/7；实机 UI 无该用户登录密码未跑 |
| A3 | 有条件 ✅ | 口径/身份/七维度+缺口/日志 round+dispatch+TOPK 全过；doc 16+web 8 可解析（见缺陷 2） |
| A4 | ❌ → **已修待复验** | 两轮 compose 均为**单 token 整段**（5184B@~39s / 4799B@~18.6s） |
| A5 | ✅ | citation_id 1..24 全局唯一；doc lookup 成功；web 走 sources 面板 |
| A6 | ✅ | 断代理 115s 收敛；`search_empty_early_stop empty_tail=2`；brain 不再派 search |
| A7 | ✅ | 空 scope+RAG → 提示选范围，无全库注入 |

SSE 证据：`output/acceptance_{a1,a3,a3b_ts,a6,a7}*.log`。

### 0.8 首轮缺陷与修复（2026-07-19 同日）

| # | 缺陷 | 根因 | 修复 |
|---|---|---|---|
| 1 | A4 流式未生效 | sink 接线正确，但 chat exit 走 `direct_content` → `complete_with_tools` 非流式 → 单 `MessageDelta` 整段 | `call_retrieve_llm`：`stream` 且（无 tools / chat / prose_only）走 `complete_stream` 并边生成边 emit；已 stream 则 `finish_direct_answer_run` 不再整段重发；若未 stream 则强制 synthesis prose 流 |
| 2 | finalize 吞标记 | 残缺 `[[E15]目录]` 使 `find("]]")` 贪婪吃掉后续 `[[E3]]` | `rewrite_markers`：若 `[[` 先于 `]]` 再出现，只吐出 `[[` 并重扫 |

**复验 A4：** 重编 api 后重跑 A3，compose 阶段应出现**多个** `event: token` 且时间分散（非整段一次到达）。  
**复验 A5 侧车：** 答案中若模型再写残缺 `[[E…]`，后续合法 cite 仍应在 final answer 与芯片中。

边界观察（不阻塞）：裸 API 用 **web** `citation_id` 调 lookup → `document_not_found`（UI 走 sources，不可达）。

### 0.9 已知可接受现象（非阻塞）

| 现象 | 说明 |
|---|---|
| 流式中短暂出现 `[[E:n]]` | finalize 在整段结束后改写；`done` 载荷为产品标记；前端用 final answer 覆盖 |
| 答案详细程度仍有轮间方差 | coverage handoff 降低但不消灭方差 |
| 本次 reindex 曾跳过 LLM summary | 日志 `quota exhausted`；图谱 entity 254 / relation 195 |
| Search 依赖代理 | 无代理时 search 空/失败属环境 |
| worker 温度 0.3 未调 | 延期 |
| chat stream 本轮不调 user_context 工具 | 为真流式让步；synthesize brief 已自包含 |

### 0.10 仍欠你本地处理

1. **DeepSeek 控制台 revoke 旧 TRIPLET 专用 key**（§0.8 配置侧已换绑）。
2. **复验 A4/A5**：按 §0.2 重启 api 至含缺陷修复的 commit 后再跑主路径。

---

## 1. 这条线在做什么

把 `avrag-rs` 后端的 agent 范式从"三个平行 agent 显式切换"重构为 **主编排器（ReAct loop）+ 子代理能力**：

- 产品表面：能力标签只有 **RAG**（工作区检索）/ **Search**（网页检索），可多选；不选 = 纯聊天。
- 内部：Orchestrator（只编排，不检索不写答案）→ RagWorker / SearchWorker（只取证，产出证据）→ Chat exit（唯一用户出口，Option B）。
- 硬约束（设计锁定）：§7.1 物化通道不可被 LLM 取消；§7.2 每个物化通道至少派发一次才能 delegate_chat；最终回答只由 Chat exit 产出；**不允许新增基于规则的去语境化/查询改写代码**。
- 产品规则：**空选择 = RAG 不可用**——未选文档时前端禁用 RAG 并提示，后端不注入全量文档清单/摘要。

## 2. 关键文档

| 文档 | 内容 |
|---|---|
| `docs/engineering/ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md` | 原始设计（§7 三层、Option B） |
| `docs/engineering/ORCHESTRATOR_V2_REACT_EVIDENCE_STORE_DESIGN_2026-07-18.md` | **现行架构总纲** |
| `avrag-rs/prompts/orchestrators/orchestrator-base.md` | 编排器 prompt（含 coverage / 对比类逐章核对） |
| `avrag-rs/prompts/orchestrators/capability-search.md` | Search 能力说明（含空结果早停） |
| `avrag-rs/prompts/clusters/codegen/SKILL.md` | RAG worker 检索 SDK |

## 3. 代码结构（`avrag-rs/crates/app-chat/src/orchestrator/`）

| 文件 | 职责 |
|---|---|
| `host.rs` | V1 结构化 host；`AgentServiceExecutor`：worker 私有 CollectingSink；**chat exit 写入外层 sink（流式）** |
| `brain.rs` | V2 ReAct：delegate / evidence_fetch / memory 工具；并行 wave；护栏；**search 两次空后禁止再派** |
| `store.rs` | 共享证据库；TOPK（RAG 24 / Search 12）；`DocProfile` 定向条目豁免 TOPK |
| `chat_exit.rs` | Chat brief：源文档 + channel outcomes（含 **coverage/gaps/key_facts**）+ 文档定向 + 可引用清单 |
| `workers.rs` | `parse_worker_handoff` / `finalize_answer_evidence`；引用全局单计数器 |
| `types.rs` | `WorkerHandoff` / `ChannelNote::with_handoff` / `ChatHandoff` |
| `invariant.rs` / `materialize.rs` | §7.2；default_brief；通道物化 |

接线：`pipeline_steps.rs::run_orchestrator_v1` → V2 brain 或 V1 host。注入：docscope、chat_persistence、orchestrator llm。

**Search 早收敛（`agent-loop`）**：`exit_policy::tool_result_has_web_hits`；连续 2 次空 → `search_empty_early_stop` / `BreakToSynthesis`。

## 4. 生产验证记录（历史 R1–R8）

同一 query：「这篇转型报告和最佳实践的差距在哪里？」· doc_scope=基准文档 148 chunks

| 轮次 | 结果 | 关键发现 |
|---|---|---|
| R1–R3 | 文体误读→修、引用编造→消、search 空转（Brave 代理） | 身份缺失、证据洪水、口径丢失 |
| R4 | search 12 条；仅覆盖一章 | `doc_profile` 单数 `doc_id` 被拒 |
| R5 | **合格** 六维度 + 17 cite + 12 web | 详细程度有方差 |
| R6 | 表格版 | 引用撞号 → 前端加载失败 |
| R7 | rag 17 条；记忆/画像/docscope | 记忆工具可空转 |
| R8 | 正常 | 空选择规则回归 |
| R9/R10（2026-07-19 验收） | 质量合格，2 缺陷 | 七维度 + 16 doc cite + 8 web cite 全可解析；A4 流式未生效（direct_content 单 blob）；`[[E15]目录]` 残缺标记致 finalize 吞合法标记（见文末验收备注） |

**2026-07-19 功能已合入；请用 §0 做正式验收（建议记为 R9+）。**

## 5. 诊断结论（设计依据，摘要）

### 5.1 答案详细程度方差

四层 LLM 方差累积；真抓手是 **coverage 显式化**（1a+1b 已做，方差可降不可消）。

### 5.2 docscope 曾未进 worker（已修）

`run_orchestrator_v1` 写入 `agent_request.docscope_metadata`。

### 5.3 codegen 面完整

7 方法均在。**Triplet：** 基准文档 reindex 后 `entity_count=254` / `relation_count=195` / `graph_passage_count=195`（先前为 0）；`graph_search` 有数据可查。

### 5.4 空选择 = 不给全量

前端禁用 RAG；`apply_summary_policy` 不再全库 list；bridge 拒空 doc_scope 定向调用。

## 6. 已完成项（按波次）

### 6.1 已提交 commit（编排相关）

| Commit | 摘要 |
|---|---|
| `64ea155` … `0b714d4` | O1 / V1 store / 策略入 prompt / 设计文档 / 前端进度 |
| `6fc8977` | V2 brain、TOPK、定向证据、引用单计数器、记忆工具、空选择、docscope |
| `dc23543` | `WorkerHandoff` + coverage prompt |
| `92baa67` | Chat 流式出口 + search 空转早收敛 |

旁线：`2dc00e3` Alipay（与编排验收无关）。

### 6.2 功能对照（验收映射）

| 能力 | 状态 | 看哪里 |
|---|---|---|
| V2 ReAct 编排 | Done | `brain.rs`，flag V2 |
| 共享证据库 + TOPK | Done | `store.rs` |
| 定向 DocProfile | Done | store + chat_exit 文档定向段 |
| 引用全局编号 | Done | `workers::rewrite_markers` |
| 空选择禁 RAG | Done | 前端 workspace + response summary |
| Coverage / handoff JSON | Done | `WorkerHandoff`，chat brief `coverage:` / `gaps:` |
| Chat 出口流式 | Done（首轮验收 A4 未过，已二次修） | sink 透传 + **chat/prose `complete_stream`**（`iteration/assemble.rs`） |
| Search 空转早收敛 | Done | agent-loop exit_policy + brain `search_channel_exhausted` |
| 残缺 `[[E` 扫描 | Done（验收缺陷 2） | `rewrite_markers` 遇嵌套 `[[` 只吐 `[[` 重扫 |
| Triplet 基准文档 | Done | parse_run backend_summary entity/relation 非 0 |

## 7. 环境与运行

- 起栈：`bash scripts/ci-start-milvus.sh && bash scripts/product-dev-up.sh`
- Flags：见 §0.2
- 日志：`avrag-rs/.dev-logs/api.log`（`orchestrator round` / `dispatch finished` / `TOPK gate` / `search retrieve early-stop` / `dangling`）
- Worker 内部迭代默认不进 INFO
- 图谱：结构性改动后 `graphify update .`（勿提交 `graphify-out/`）

## 8. 欠账（验收不阻塞）

| 项 | 状态 |
|---|---|
| 编排器/worker 温度正式决策 | 未做（可选战术） |
| Chat 成 loop / 引用校验 agent / 对比 eval 集 | V3 延期 |
| Worker handoff 模型 JSON 稳定性 eval | 可后续加 |
| DeepSeek **平台** revoke 旧 TRIPLET key | **需你操作** |
| shared KB 分享页 `doc_scope:[]` 质量 | 产品后续 |

## 9. 不要踩的坑

- 策略文本住 prompt；代码只留确定性约束（store / 标记 / 闸门 / 接线）
- 勿绕开 TOPK；勿全量入库
- 勿恢复「全工作区默认注入」
- citation_id 必须全局唯一
- 勿给 `write` 留 ToolCatalog 入口（T2）；workspace/org 红线见 `docs/agent/product-apps.md`
- 流式验收务必确认 api 进程已重编（§0.2）

## 10. 代码导航（验收排障）

| 现象 | 先查 |
|---|---|
| 仍整段蹦字 | api 是否含本批修复；`request.stream`；chat 是否走 `call_retrieve_llm_stream` / synthesis prose stream；勿停在旧 `direct_content` 单 delta |
| 合法 cite 被吞 | `rewrite_markers` 是否含「`[[` 先于 `]]` 则重扫」分支 |
| 引用 404 | `rewrite_markers` 是否单计数器；是否残留双计数器分支 |
| Search 空转很久 | 代理；日志有无 `search_empty_early_stop`；brain 是否仍多次 `delegate_search` |
| 维度只盖一章 | worker handoff `gaps` / chat brief 是否含 coverage；orchestrator instruction 是否要求逐章 |
| graph_search 无数据 | `INGESTION_TRIPLET_ENABLED`；该 doc 最新 parse_run 的 entity/relation 计数 |

---

**首轮验收结论（2026-07-19 · `2dad134`，api 曾重编至 `a2813d8`）：**

- [x] A1–A7：A1 ✅ · A2 ✅（vitest 7/7）· A3 有条件 ✅ · **A4 ❌** · A5 ✅ · A6 ✅ · A7 ✅
- [x] A3：**有条件合格**（§0.6 1/2/3/5 过；引用可解析但见缺陷 2）
- [x] 缺陷 1/2 根因见上表 §0.8；SSE：`output/acceptance_{a1,a3,a3b_ts,a6,a7}*.log`
- [x] 观察：web citation_id lookup → document_not_found（UI 不可达）

**缺陷修复后（待你复验 A4/A5）：**

- [ ] A4：compose 阶段多个 `event: token`、时间分散（非整段单事件）
- [ ] 残缺 `[[E…]` 不再吞后续合法 `[[cite:` / `[[web:`
- [ ] 复验人 / 日期：________
- **复验前**：§0.2 重编 api 至含本批修复的 tip
