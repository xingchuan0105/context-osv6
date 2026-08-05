# 架构加深计划 1–7（2026-08-05）

| 项 | 内容 |
|---|---|
| 日期 | 2026-08-05 |
| 状态 | **实现中 / 主轨+旁路+独立轨已落地**（2026-08-05 执行窗口）；验证见各包 `cargo test -p … --lib` |
| 来源 | `/tmp/architecture-review-1785905663.html` + grilling 1–13 拍板 |
| 前序 | `docs/plans/2026-08-02-architecture-deepening-plan.md`（C1–C7 **已交付**，本计划 **不重做**） |
| 范围 | 主轨：多轮 model-visible 证据（#1–#3，#7 并入 #2）；旁路：#4 S+L finalizer；独立轨：#5 MM embed、#6 parse→IR |
| 不做 | push/PR/CI；改 demote/预算数字做效果优化；dense→lexical fallback 修 members（#4 排除）；embed 挂 chat `Transport`；本刀删 `DocumentParser`；host 语义「够了禁止 DirectAnswer」 |
| 约束 | T1–T8；**T5 默认行为保持**；prompts-in-md；第三人称观察；Messenger 模型；ADR-0009 bridge 保持薄 adapter；solo trunk；WSL `jobs=2`；结构性改动后 `code-review-graph update`（不提交图缓存） |
| 执行须知 | 行号/文件以动手当日源码为准；**前提不符就停**，回写本计划「状态」行，不即兴改设计 |

---

## 0. 一句话

多轮检索与 S+L 已作用在同一批 model-visible 证据上，但 **形态协议双份、durable 仍是字段袋、LLM 边界无单一视图**。按「Evidence Form → Evidence Pool → Model-visible View」加深主轨；旁路收 S+L 出口；独立轨收 MM 融合与 parse CLI shell。

---

## 1. 产品 / 架构决策（已拍板，执行窗口直接采用）

| # | 决策点 | 结论 |
|---|--------|------|
| D1 | 交付形态 | **先计划、确认后再实现**（grilling A） |
| D2 | 切轨 | **一文档三轨**：主轨 #1→#2→#3（#7 并入 #2）；#4 可空档；#5/#6 附录并行 |
| D3 | #1 形态语法归属 | **新建 crate `evidence-form`**（纯协议，零业务依赖） |
| D4 | #1 职责边界 | **只** expanded/card/stub、adjacent、切片常量、量字数、`text`/`content` 约定；**不**含 12k/16k 预算、history 选轮、alias/reseen、Pool、prompt |
| D5 | #2 Pool 归属 | **`agent-loop` run 态**；bridge 经共享 Arc 写；Intake 并入 Pool 模块 |
| D6 | #2 durable 内容 | `chunk_id → { first_alias, full_text, members, doc 元数据 }` + **claim 板**；**仅 expanded 抽 claim**；不存 CoT、不双写全量 ToolResult |
| D7 | #3 第一刀范围 | **仅 retrieve 相** model-visible 入口；synthesis 48k / message_format 24k **延后** |
| D8 | #4 范围 | **只** `finalize_evidence_package`（hydrate + S+L + JSON）；**不**修 dense fallback 丢 members（记已知缺口） |
| D9 | #5 范围 | pure `fuse_part_vectors` + 协议分路径 + 单一 `MultiModalEmbeddingInput`；**不**接 Transport |
| D10 | #6 范围 | `markdown_cli` shell + Paddle IR 归 ingestion；**不**本刀删 DocumentParser |
| D11 | 行为契约 | **默认行为保持**；唯一「变好」：#2 接线 `seen_chunk_bodies` 后跨轮 rehydrate **从不可达→可达**（补齐接缝，不改 demote 数字/顺序） |
| D12 | 命名 | crate **`evidence-form`**；术语见 §2 |
| D13 | 落盘 | 本文 `docs/plans/2026-08-05-architecture-deepening-1-7-plan.md` |

---

## 2. 领域术语（计划权威；实现时懒写入现行词典）

> 根 `CONTEXT.md` 已标 STALE。实现窗口将术语写入现行入口（优先短条目挂 `docs/README.md` 索引或 `AGENTS.md` 可链附录），**勿**假装旧 CONTEXT 全文仍权威。

| 术语 | 定义 |
|------|------|
| **Evidence Form** | 单条检索 hit 的 model-visible **形态协议**：expanded / card / stub + adjacent 约定 + 字段读写（`text`/`content`） |
| **Evidence Pool** | 单次 ReAct **run** 的 durable 记忆：alias 命名空间、全文 body、member 闭包、claim 板 |
| **Model-visible View** | 在 LLM 边界从 durable + messages 组装出的 **本轮** 可见消息（retrieve 相第一刀） |
| **Evidence Intake** | Ok 检索回传 → 写入 Pool + 产出 observation 素材（原审查候选 #7） |
| **Retrieval Bridge** | 沙箱 codegen → 宿主 `RagRuntime` 的 fd 管道 RPC（ADR-0009）；本计划中保持 **薄 adapter** |

架构词（加深时统一用）：module / interface / depth / seam / adapter / leverage / locality / implementation。

---

## 3. 波次总览

### 主轨（串行）

| 波 | 候选 | 评级 | 关键杠杆 | 行为面 |
|----|------|------|----------|--------|
| **W1** | #1 Evidence Form crate | Strong | 双份 280/adjacent 收拢；bridge 与 loop demote 共语法 | 保持（常量与启发式语义对齐现状） |
| **W2** | #2 Evidence Pool + Intake（含 #7） | Strong | run 级 durable 单模块；body Arc 接线；claim 出 codegen 旁路 | 保持 + rehydrate **可达性补齐** |
| **W3** | #3 Model-visible View（retrieve） | Strong | assemble 薄调用；history/working-set/claim 一入口 | 保持；synthesis/24k 不进本波 |

顺序理由：无共享 Form 则 Pool/View 仍会复制 demote；无 Pool 则 View 只能继续扫 messages bag。

### 旁路（可与主轨空档并行）

| 波 | 候选 | 评级 | 说明 |
|----|------|------|------|
| **W4** | #4 S+L finalizer | Worth exploring | 可插在 W1 后任意空档；不挡 W2/W3 |

### 独立轨（附录；可另开窗口）

| 波 | 候选 | 评级 | 说明 |
|----|------|------|------|
| **T5** | #5 MM embed fusion | Strong | 与主轨无代码依赖 |
| **T6** | #6 parse→IR shell | Worth exploring | 与主轨无代码依赖 |

### 候选 → 波次映射

| 审查 # | 名称 | 波次 |
|--------|------|------|
| 1 | Shared chunk visibility grammar | W1 |
| 2 | Run-scoped Evidence Pool | W2 |
| 3 | Single model-visible context view | W3 |
| 4 | Post-retrieve shortlist finalizer | W4 |
| 5 | MM embedding adapters + pure fusion | T5 |
| 6 | Parse→IR CLI shell + Paddle locality | T6 |
| 7 | Observation intake out of codegen | **并入 W2**（不单列波） |

---

## W1 · Evidence Form（`evidence-form` crate）

### 现状（动手前再核实）

- `rag-core/src/runtime/visibility.rs`：`CARD_SNIPPET_CHARS = 280`、`is_adjacent_item`、`apply_visibility_to_chunks`、mark expand/card/stub  
- `agent-loop/.../context_visibility.rs`：`WORKING_SET_CARD_CHARS = 280`、`is_adjacent_chunk`、`demote_chunk_to_card`、working-set 预算逻辑  
- 预算（12k call / 16k working-set）**分属两 seam**，正确；**形态突变**双份，错误  

### 设计

- 新建 workspace member：`avrag-rs/crates/evidence-form`（package 名与目录一致；edition/workspace 继承）  
- **interface（概念，实现窗口再定签名）**：形态原语 + 单一切片常量；无 IO、无 loop/bridge 类型  
- `rag-core` visibility / bridge 调用方改为依赖 Form 原语  
- `agent-loop` demote 改为依赖 Form 原语；**保留**「选哪几条 / 16k 预算」在 context_visibility  

### 切片

| 切片 | 内容 | 验证门 |
|------|------|--------|
| 1 | 建 crate + 从现实现 **抽出** pure 原语；双端仍调用本地 wrapper（可先 re-export） | `cargo test -p evidence-form`；既有 rag-core / agent-loop 相关测绿 |
| 2 | rag-core visibility 改真依赖 Form；删本地重复常量/adjacent/mark | `cargo test -p avrag-rag-core --lib` |
| 3 | agent-loop demote 改真依赖 Form；删 twin 280/adjacent/demote 体 | `cargo test -p agent-loop --lib` |
| 4 | grep 确认无第二份 `CARD_SNIPPET`/`WORKING_SET_CARD` 字面协议 | 全仓 grep；`code-review-graph update` |

### ADR

- 不与 ADR-0009 冲突（bridge 仍 adapter）  
- 不引入 host 语义停答  

---

## W2 · Evidence Pool + Intake（含原 #7）

### 现状（动手前再核实）

- `IterationState`：`tool_results`、`seen_chunk_aliases`、`seen_retrieval_aliases`、`evidence_notes` 松散字段  
- `RuntimeBridge::seen_chunk_bodies` 私有；`with_seen_chunk_bodies` **存在**，loop **未**注入  
- claim 仅在 `iteration_codegen::retrieval_callouts` 副作用更新  
- 产品检索 native 面已 `SAC_SUPERSEDED`；intake 主路径 = codegen/bridge Ok  

### 设计

- `agent-loop` 内 deep module（建议路径 `react_loop/evidence_pool/` 或等价）：拥有 D6 内容  
- 构造 run 时创建 Pool；注入 bridge：`with_seen_chunk_aliases` **+** `with_seen_chunk_bodies`  
- **Evidence Intake**：从 Ok bridge/tool data → register runs / bodies / members → accumulate claim（expanded only）→ 供 summary/observation 读出；`retrieval_callouts` **变薄**（只格式化 + 调 Intake，不直接 `accumulate`）  
- SELECTED/cite：优先 Pool 查 body；必要时短期保留 tool_results 回放 fallback，稳定后删（可 W2 切片 4 或挂账）  

### 切片

| 切片 | 内容 | 验证门 |
|------|------|--------|
| 1 | 行为锁：现 claim 板 / reseen / alias 行为用单测钉死 | agent-loop 现有 + 新增 fixture 绿 |
| 2 | Pool 类型落地；State 字段迁入或委托 Pool；**接线 body Arc** | 单测：二次命中 member 闭包 reseen；跨 call body 可读 |
| 3 | Intake 从 `retrieval_callouts` 抽出；codegen 只调 Intake | 无「摘要函数改 durable」副作用；测绿 |
| 4 | （可选）cite/SELECTED 读 Pool；删脆弱回放 | grep + 相关测 |
| 5 | `code-review-graph update` | — |

### 行为注（D11）

- demote 顺序、16k、280、history keep-K **不变**  
- 变化仅：history 清掉后 **按 alias 取全文** 从「依赖 messages 碰巧还在」变为「Pool 保证可达」  

### ADR

- 对齐多轮设计 §2 durable vs model-visible  
- ADR-0002 Messenger：Pool 不禁止再检索  
- ADR-0009：bridge 不膨胀为 policy 引擎  

---

## W3 · Model-visible View（retrieve 相）

### 现状

- `iteration/assemble.rs`：history transform → budget → query card → claim 注入 → hooks  
- `synthesis.rs`：`trim_tool_results_for_synthesis` 另一套  
- `message_format.rs`：24k 盲截  

### 设计（D7）

- deep module：`build_retrieve_model_visible(...)`（名实现窗口定）  
  - 输入：durable Pool + messages + keep_recent + char budget + claim 板  
  - 输出：本轮送 LLM 的 messages（含 claim 注入约定）  
  - 内部调 **Evidence Form** 做 demote  
- assemble 只调 View + 既有 ContextAssembler system/tools  
- **本波不做**：synthesis phase 合并、24k 语义改写  

### 切片

| 切片 | 内容 | 验证门 |
|------|------|--------|
| 1 | 锁 assemble 现序与 transform 输出 golden | agent-loop 测绿 |
| 2 | 抽 View 模块；assemble 改委托 | 行为等价测绿 |
| 3 | 文档：STATE_MACHINE / multi-round 设计指针 → View 入口 | 文档=代码 |
| 4 | 挂账条目写入本计划 §6：synthesis 视图、24k 账户 | — |

### ADR

- 对齐 pi transformContext / 多轮设计 §6  
- 不与 ADR-0007 ContextAssembler 抢职责：Assembler 管 skill/disclosure system；View 管 **user/tool 历史形态**  

---

## W4 · Post-retrieve S+L finalizer（旁路）

### 现状

- `dense.rs` / `lexical.rs` 各一份 hydrate + `adjacent_merge_shortlist_longlist(..., 1, 8)`  
- dense fallback 从 lexical `ToolResult` JSON 回拼 → **丢 cursor/members**（**本波不修**，§6 缺口）  

### 设计（D8）

- tools 层单一 `finalize_evidence_package(shortlist, longlist, …)`：hydrate if store → adjacent_merge → scored JSON  
- dense：VGRAG/adaptive **之后** 调 finalizer  
- lexical：BM25/adaptive **之后** 调同一 finalizer  
- `(radius, pull_budget)` 单点常量或配置读  

### 切片

| 切片 | 内容 | 验证门 |
|------|------|--------|
| 1 | 抽 finalizer；dense/lexical 改调用 | `cargo test -p avrag-rag-core --lib`；S+L 单测 |
| 2 | 删双份 inline；env `RETRIEVAL_ADJACENT_MERGE=0` 仍一处短路 | grep |
| 3 | 文档指针 adjacent-merge 设计 §4.2 | — |

---

## T5 · MM embedding（独立轨）

### 现状

- `llm/src/embedding.rs`：`api_style` 缠 DashScope server fusion 与 OpenAiVl 客户端 L2-mean  
- `MultiModalEmbeddingInput` 在 `llm` 与 `rag-core-ports` 双份  
- chat 已有 `Transport`；embed 自持 reqwest（本轨 **不** 强接 Transport，D9）  

### 设计（D9）

1. pure `fuse_part_vectors`（L2 → mean → L2）+ 单测（含现 SF caption 回归）  
2. 私有协议路径：DashScopeMm / OpenAiVl / text；`EmbeddingClient` 只路由  
3. **单一** `MultiModalEmbeddingInput`：`rag-core-ports` 为源；llm 删孪生并 map 删除  
4. 可选紧随：rerank VL **同构** 小切片（同 PR 或紧随 commit）  

### 切片

| 切片 | 内容 | 验证门 |
|------|------|--------|
| 1 | pure fuse + 现有 openai_vl 融合测迁移/保留 | `cargo test -p avrag-llm --lib` embedding 相关 |
| 2 | 协议路径拆分；Client 变薄 | 同上 |
| 3 | 类型单源；ports 适配去 field-by-field 双结构 | rag-core / worker 编译 + 相关测 |
| 4 | （可选）rerank 同构 | rerank 测 |

---

## T6 · Parse→IR（独立轨）

### 现状

- anydoc / markitdown / liteparse 三套 temp+spawn+timeout  
- Paddle client 在 ingestion，IR 组装在 worker `pdf/paddle`  
- `DocumentParser`/`ParserFactory` 仍服务 url_fetch Html 等（**本刀不删**，D10）  

### 设计（D10）

1. ingestion `markdown_cli` deep module：temp、timed spawn、stdout/file  
2. 三后端只留 dialect + 后处理  
3. Paddle IR 组装迁至 ingestion，与 `PaddleOcrClient` 同箱；对外 `parse_*_document_ir`  
4. worker `parse_route` 纯 plan 调度  

### 切片

| 切片 | 内容 | 验证门 |
|------|------|--------|
| 1 | `markdown_cli` + 一后端迁移（建议 markitdown 或 anydoc 先） | ingestion 测 + worker 解析相关 |
| 2 | 其余 markdown CLI 迁完 | 同上 |
| 3 | Paddle IR 迁 ingestion；worker 删重复组装 | worker + ingestion 测 |
| 4 | 注释/命名：struct stage「markdown 侧信道」与后端无关 | 文档 |

---

## 4. 跨波治理

| 项 | 动作 |
|----|------|
| 术语 | 实现首波后把 §2 写入现行词典入口（短表即可） |
| ADR | **不** 仅为 Form crate 新开 ADR（可逆、不惊人）；若 Pool 与 ADR-0009 写手冲突再补录 |
| prompts | 新 host marker 必须先注册 `host_markers.rs`；文案只进 `avrag-rs/prompts/**/*.md` |
| 图 | 每波结构性改动后 `code-review-graph update` |
| 验证默认 | `cargo test -p <pkg> --lib`；波末或请求时 `bash scripts/test-l1.sh`；不叠全量 cargo；真 LLM full149 **不** 挡中段 |
| 时间成本 | 任何 `cargo`/脚本前按 AGENTS.md **估时并征得用户同意** |

---

## 5. 执行窗口交接

1. 用户 **明确确认本计划** 后再开 W1。  
2. 读 `AGENTS.md` + 本计划 §1；动手前核实源码前提。  
3. 严格切片顺序；验证门不过不前进。  
4. 本地 trunk commit；不 push。  
5. 计划与代码冲突 → 停，回写状态，上报。  

---

## 6. 已知缺口与挂账（本计划明确不做）

| ID | 项 | 去向 |
|----|----|------|
| G1 | dense→lexical fallback 丢 `cursor`/`member_chunk_ids` | 效果/正确性后续；非 W4 |
| G2 | synthesis 并入 Model-visible View（phase） | W3 之后 |
| G3 | `TOOL_MESSAGE_MAX_CHARS` 24k 与 working-set/synthesis 预算账户统一 | 效果波次 |
| G4 | embed/rerank 挂 `Transport` 离网 | T5 之后可选 |
| G5 | 删除 `DocumentParser`/`ParserFactory`，url_fetch 改 IR | T6 之后可选 |
| G6 | cite 完全去掉 tool_results 回放 | W2 切片 4 可选 |
| G7 | 上一波 C4 failover 统一 | 仍等真实流式故障（旧计划） |

---

## 7. 决策冻结记录（grilling）

| 问 | 主题 | 用户选择 |
|----|------|----------|
| 1 | 交付形态 | A：计划 → 确认 → 代码 |
| 2 | 切轨 | A：一文档三轨 |
| 3 | Form 归属（曾问 rag-core） | 用户改 **B：独立 crate**（后定名 evidence-form） |
| 4 | Form 职责 | A：纯形态协议 |
| 5 | Pool 归属 | A：agent-loop |
| 6 | Pool 内容 | A：body+claim；expanded only |
| 7 | View 范围 | A：retrieve 第一刀 |
| 8 | #4 范围 | A：只 finalizer |
| 9 | #5 范围 | A：fuse+协议+单类型 |
| 10 | #6 范围 | A：cli shell+Paddle；不删 DocumentParser |
| 11 | 行为契约 | A：保持 + rehydrate 可达补齐 |
| 12 | 命名 | A：`evidence-form` + §2 术语 |
| 13 | 落盘 / 收束 | A 路径 + G1 冻结 |

---

## 8. 确认清单

- [x] 同意 §1 全部决策 D1–D13  
- [x] 同意主轨顺序 W1→W2→W3，W4/T5/T6 可并行  
- [x] 同意 §6 挂账不做进本计划范围  
- [x] 用户确认：`确认计划，开W1-W4，T5+T6`（2026-08-05）  

### 8.1 实现回填（2026-08-05 执行窗口）

| 波 | 落地要点 | 验证 |
|----|----------|------|
| W1 | crate `evidence-form`；rag-core/agent-loop 改依赖 Form | `cargo test -p evidence-form` + visibility 测 |
| W2 | `evidence_pool` + `seen_chunk_bodies` 注入 bridge；Intake 经 `intake_claim_notes` | agent-loop lib |
| W3 | `model_visible::build_retrieve_history_view`；assemble 委托 | agent-loop lib |
| W4 | `merge::finalize_evidence_package`；dense/lexical 共用 | avrag-rag-core lib |
| T5 | `fuse_part_vectors`；`MultiModalEmbeddingInput` 单源 ports | avrag-llm lib |
| T6 | `markdown_cli`；anydoc/markitdown 共用 shell；Paddle IR 迁 ingestion | ingestion + worker paddle 测 |

**顺手测修：** bridge reseen 断言对齐多轮 P0；codegen L-eval 测改 Atomic override + 串行锁（无 process env 竞态）。

**Review 返工（I1–I9）：** EvidencePool 入 `IterationState`；Intake 出 callouts；history stub 走 form；View 组合 budget/card/claims；paddle metadata 纯函数；`run_cli_status`；mark 去双重 set；L-eval 测无 env。

**未做（§6 挂账仍有效）：** G1 fallback members、G2 synthesis View、G3 24k 账户、G4 Transport、G5 删 DocumentParser。Issue 10（crate 命名 `evidence-form`）有意保留。
