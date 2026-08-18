# 交接：retrieve/synthesis 分模型 + lead_workers 事件日志 + facets 子检索（2026-08-16/17）

## 一句话状态

retrieve=`qwen3.7-flash`(thinking off) / synthesis=`deepseek-v4-flash`(thinking max) 已上线，速度 ~4×（单题 p50 73s→20s，全量 38min→~11min）。事件日志 + `[retrieval_worklog]` 投影、joint 打回、BriefWire null 修复、calculator 宿主误触发删除均已落地验证。最新一轮（run6）上线 **facets 多侧子检索**（单 Worker 顺序执行、逐 facet 独立预算与独立 pack）：facet 题 PASS 率 88–100% vs 未拆 79%，但总 PASS 127 受 churn 拖累（详见 §10/§11）。**待决策**：Worker 执行模型要不要从「顺序隔离 facet」换成「共享上下文一次铺开」（§11 末）。

## 1. 分模型配置（已上线）

- `AppConfig.retrieve_llm`（`app-core/src/config.rs`），env 前缀 `RETRIEVE_LLM_*`；`.env` 已配 dashscope `qwen3.7-flash`（复用 INGESTION 的 key），`.env.example` 已文档化。**默认空 = 不启用，行为不变**。
- `ReActLoop::with_retrieve_llm`（`agent-loop/.../react_loop/mod.rs`）：retrieve 强制 thinking off；synthesis 不变（thinking max）。
- `UnifiedAgent.retrieve_llm_client`（`app-chat/.../unified/mod.rs`），app-bootstrap 两个组合点接线；**BYOK 请求跳过该 override**（用户 key 全程留在用户端点）。
- dashscope 非 deepseek/google 时 `enable_thinking: false` 走 OpenAI 兼容字段，已确认兼容。

## 2. 基线与验证数据

单题耗时基线来自 PG `llm_usage_events`（`avrag_rs_e2e_smoke`），按 run 时间窗 + `session_id` 分组（首事件→末事件跨度；不含 judge）：

| Run | 配置 | 全量时长 | 单题 avg/p50/p95 | PASS | 关键质量 |
|---|---|---|---|---|---|
| 08-11 | 全 deepseek-v4-flash | 2301s | 74s / 73s / 124s | 126 | correctness .958, recall@k .912 |
| 08-16 一跑 | retrieve=qwen | 566s | 23s / 20s / 52s | 128 | recall@k .861（检索变弱） |
| 08-16 二跑 | +T1+T2+T3 | 649s | — | 132 | faithfulness .972 |
| 08-16 三跑 | +worker calc prompt | 567s | — | 130 | recall@k .883，回升中 |
| 08-17 四跑 | +删除 detect_utility_kind（并发 8） | 646s | — | **133** | **recall@k .9121（回到基线），calculator Error 0** |

工件：`avrag-rs/crates/app/tests/e2e_output/rag_eval_v2/v2_<ts>/`（summary.json / per_query.tsv / qNNN.artifact.json）。**注意：黄金集题号 n 跨 run 不稳定（会 shuffle），对照必须按 query 文本匹配。**

## 3. 错题分层诊断结论（首跑 20 道非 PASS）

- **Lead 任务分解**：qwen 把 `ADR-0004` 当算术表达式；joint 题漏派 rag 通道。
- **Worker 检索执行**：坚持度低，有效 Ok 4–7 次就收工，ADR 精确事实 first_hit_ranks 全面退化。
- **覆盖裁决**：`coverage` 是 host 拍的二元值（`n>0→Partial`，无 complete 态），135/136 全 partial，零信息量；re-brief 全 run 零触发。
- **合成**：deepseek 未换，问题均为上游传导（薄证据→保守拒答）。

## 4. 已落地的三项修复

**T1 prompt 收紧**：`lead-base.md` / `lead-plan.system.md` / `agent-base.md` 都加了「calculator 输入是题干数字完备的算术表达式；标识符不是」边界陈述。**前两个（Lead 层）无效**——误调用根本不在 Lead 层。

**T2 joint 缺通道打回**：`lead_plan.rs` 新增 `PlanParseOutcome::DualChannelMissing`——dual 激活且选择检索时两通道缺一即打回，复用 repair 轮；repair 仍缺 → host fallback；空检索（base_tools/none）不受影响。二跑验证：6 道 dual 题全部双派全 PASS，打回 0 次触发（qwen 自己派对了，机制作兜底）。

**T3 事件日志 + 投影**（参考 deepseek-harness session/event 与 pi 会话树）：
- 新模块 `react_loop/run_log.rs`：append-only run 事件日志（seq+at_ms），10 类事件；`surface()` 显式分类，log-only 是默认。
- 插桩：plan（proposed/repair/fallback）、base_tools、wave 派工/工具调用/PackGate/波次取代/re-brief/handoff。Worker 步骤从 `WorkerOutcome.tool_results` 投影，**未侵入 SaC 内层**。
- `[retrieval_worklog]` 投影取代 `[coverage_aggregate]`（query→objective→key_facts≤8×120字→缺口→波次事件，含 PackSuperseded 留痕）；pack JSON 去 `coverage` 字段；handoff 模板去伪标签。
- `run_events` 已并入 Evaluation signals → 落 `mode_debug.general.lead_workers`（三跑确认 136 题可见）；DebugTrace 也带。
- marker 注册表 `[coverage_aggregate]`→`[retrieval_worklog]`；`prompts/loop/coverage-aggregate.tmpl.md` 已删，新增 `retrieval-worklog.tmpl.md`；prompts 文档引用（lead SKILL、loop README、lead-base×2）已对齐。

测试：`cargo test -p agent-loop --lib` 431 绿；`cargo check -p app --test product_e2e --features product-e2e` 通过。code-review-graph 已更新。

## 5. calculator 误调用的真根因与最终处置（2026-08-17）

**根因**（三跑 run_events 实证）：`detect_utility_kind` 的 calc 命中条件含「任一字符是 `+-*/^×÷`」——任何带连字符的题面（ADR-0004、IPD 编号…）都触发**宿主自己**跑 calculator（plan 后、wave 前，`base_tool_executed{ok:false}`）。Worker 模仿证据：46 题中 21 题 host+worker 同现、仅 3 题 worker 单独。

**最终处置：整个 `detect_utility_kind` 已删除**（连同 `run_utility_kind` / `host_inject_utility_if_needed` / `extract_math_expression` / `extract_weather_location` / `infer_base_tool_kind` 与 `BaseToolKind`；`run_retrieval.rs` 的宿主注入调用点同步移除）。理由：硬编码关键词枚举无法判断多语种自然语言的 query 意图，机制本身无效且反向误导 LLM——模型该从 brief/prompt 理解任务，而不是被宿主伪造的工具结果带偏。

**替代设计**：`SubTask` 新增 `base_tool` / `base_tool_arg` 字段——base_tools brief 由 Lead 显式声明具体工具与参数（weather=地点、calculator=算术表达式、user_context=空），**宿主只执行、不猜测**；留空/未知 → `base_tools_unmapped` error observation。prompt 三处已同步：lead-plan.system.md（schema + 约束行）、lead-base.md、clusters/lead/SKILL.md。

## 6. 删除后的遗留核查结果（已全清）

- 残留引用：无（`app/tests/product_e2e/mock_llm_server.rs` 的同名 `extract_weather_location` 是测试自身的 mock 助手，无关）。
- `normalize_calculator_expression` 仍被 calculator skill 自用（`agent-tools/.../calculator.rs:221`），非死代码。
- 文档无引用该 detector 之处；host_markers / prompts parity 不受影响。
- 编译 + `cargo test -p agent-loop --lib` 432 绿；`app --test product_e2e` 目标编译通过。
- **行为差异（已实证）**：宿主不再兜底「Lead 漏派 base_tools 的纯工具题」。四跑结果：计算题（`128×46+357`、`1+2*3`、`(10+5)*2`、`(1587+2933)×1.13`）全部 PASS（模型自行调用）；但**空 caps 纯 chat 道**的「现在日期时间」「北京天气」回归（REFUSAL_WRONG / UNGROUNDED）——该道模型未自行调 `client.user_context` / `client.weather_query`，其中时间题还虚构了「工具回传没有数据」。这是删除的已知取舍：**要恢复只能靠模型自觉（prompt 侧），不要重新引入宿主关键词猜测**。
- 遗留弱簇：ADR 精确事实仍 6 道坏（决策日期/备选方案/落地废除文件/Slices-Phases/各自日期），方向是 Worker SaC 检索坚持度（qwen 4–6 次 Ok 收工）；`1T/H` 产能题模型只算不检索（eval_bridge_miss），属合成侧 grounding。

## 7. 注意事项

- **工作树有大量其它会话的未提交改动**（billing/relay/desktop/frontend 等），`git status` 很脏，提交时注意只挑本次相关文件。
- `transport-http/src/routes/relay.rs` 被我修过两处既有编译错误（`service` 先 move 后 borrow，别人的在途工作），如果对方会话也在改，留意冲突。
- WSL：`CARGO_BUILD_JOBS=2`，不要并发跑多个全量 cargo test。
- 全量跑法：`bash scripts/test-full149.sh`（默认无限预算基线、并发 8、熔断 8、总帽 4h）。
- 每个 ADR 题的 worker `max_steps` 上限 5，calculator 烧 2 步是 ADR 簇薄证据的主因——修好宿主误触发后若 ADR 仍弱，下一步看 Worker SaC 的检索坚持度（qwen 4–6 次 Ok 就收工）。

## 8. 四跑 16 道错题逐题分层分析（run4 = v2_20260816-162734）

错题共 **16** 道（PASS 133/149）。分层：

**Plan/Lead 层**
- q24（1T/H 产能，CORRECT_UNGROUNDED）：Lead 派 base_tools-only，绕开 KB；计算对但未锚定文档（judge 口径也掺半——操作数本就来自题干）。
- **隐性大头：plan JSON 51% fallback**（72 repair / 69 fallback / 136 题）。根因是 wire 格式事故：qwen 给非 base_tools brief 写 `"base_tool": null`，`#[serde(default)] String` 不耐显式 null → 整个 plan 判 Invalid → repair 同样 null → 通用 fallback brief。错题中 7 道（q42/54/58/93/106/107/111）都在跑通用 brief。**已修**：BriefWire 三个字段改 `Option<_>`（lead_plan.rs），带回归测试。

**Worker/检索执行层**
- 坚持度不足：q57（3 次调用收工，facts=2→拒答）、q51（5 次，未翻 Alternatives 节）、q130（2 次）。
- **key_facts 硬上限 5 条 ×160 字**（`run_lead_workers.rs:1498`）：q106/q107/q111 双侧跨文档题各 22–33 次调用，检索量够但 pack 只装 5 条 → 另一侧挤不进 → PARTIAL。宿主侧瓶颈，非模型问题。
- infra 抖动：q58/q62/q107/q111 出现 `dense_retrieval` 短暂 `embedding_unavailable`。

**合成层（deepseek，未变）**
- q118（时间，REFUSAL_WRONG）：`user_context` **Ok 却声称「工具回传没有数据」**——合成否认 observation，最扎眼。
- q119（天气，UNGROUNDED）：chat 道未调 `weather_query`，答案回声。
- q27（2019 行业规模，PARTIAL）：1467 在手却引入 1342 对冲不提交（多跑一贯的过度谨慎）。
- q54（ADR-0009 废除文件，SELECTION_MISS）：答案文本正确（code_gen_query.rs）但引用 alias 错位。

**judge/eval 口径噪声**
- q42：答案给了正确日期 2026-06-06，judge missed 却写「模型给出 06-09」——judge 与答案自相矛盾；RETRIEVAL_MISS 是检索层口径（黄金 chunk 未进 top-k），答案对也照判。
- q93（IPD 概念阶段活动数）：全表计数题，chunk 检索天生给不全（57/81 行口径），结构性 PARTIAL。

逐题明细：

| 题 | 标签 | 层 | 诊断 |
|---|---|---|---|
| q24 产能计算 | CORRECT_UNGROUNDED | Lead 分解 | base_tools-only 绕开 KB |
| q27 行业规模 | PARTIAL | 合成 | 1467→1342 对冲 |
| q42 ADR-0004 日期 | RETRIEVAL_MISS | plan fallback+judge 噪声 | 答案实际正确 |
| q51 备选方案 | RETRIEVAL_MISS | Worker 坚持度 | 5 次未翻 Alternatives |
| q54 废除文件 | SELECTION_MISS | 合成引用 | 答案对、alias 错 |
| q57 废弃文件 | RETRIEVAL_MISS | Worker 坚持度 | 3 次收工 |
| q58 各解决什么 | PARTIAL | fallback+dense 抖动 | ADR-0004 侧缺 |
| q61 各自日期 | SELECTION_MISS | 检索选择 | ADR-0004 日期未入选 |
| q62 Slice/Phase | PARTIAL | 检索 | Phase 侧缺（dense 抖动） |
| q93 概念活动数 | PARTIAL | 结构性 | 全表计数超 chunk 能力 |
| q106 PAC×分级 | PARTIAL | key_facts 上限 | 白药侧挤不进 |
| q107 4R×4A | PARTIAL | key_facts 上限 | 4A 三架构缺 |
| q111 活动×对象 | PARTIAL | key_facts 上限 | 370 缺 |
| q118 时间 | REFUSAL_WRONG | 合成 | Ok 却谎称无数据 |
| q119 天气 | UNGROUNDED | chat 自觉 | 未调 weather_query |
| q130 文档类型 | PARTIAL | Worker 坚持度 | doc_summary 无文件类型 |

层面分布：Lead/plan 1+7（wire）/ Worker 4+3（上限）/ 合成 3 / judge 噪声 2。

## 9. 双侧题单侧证据缺失的根因诊断（2026-08-17）

**结论：不是 embedding/topK 问题，是 Worker 没发另一侧的 query。**

排查路径（排除法）：

1. ~~跨库向量坏~~：`rag_text_chunks` 里有 **130 个 doc_id ≈ 同一批文档被重复灌库 ~10 代**，历史代次（非活动工作区）确有大量向量与文本不符（自匹配距离 ~1.0）——是数据卫生问题，但不在评测 scope 内，是假线索。
2. 活动工作区（`6e75b15c…`，10 篇 477 chunk）embedding 完好：随机 8 chunk 自匹配距离全 0.000；白药分级黄金 chunk（`5d4cc532`）对 Worker 实际 query「云南白药 IT项目分级 A级 B级 C级」dense **rank 1**。
3. **run4 实证**：该题（PAC×白药分级）Worker 22 次检索调用**全部 PAC 侧**，「云南白药」零出现——fallback 通用 brief（「从知识库检索：整题」）无拆分提示，qwen Worker 顺着显眼侧钻到底，侧 B 从未被检索。召回评测自然 `matched_golden=[0]`。
4. **run5 反证**：同题 Worker 双侧交替 query，黄金 chunk 双侧命中（first_hit_ranks [0,14]，侧 B 经 graph/合并通道 rank 14 进包）→ PASS。

**残余结构风险**：dense pool 24 → vgrag final 12 → 工具回传 5 条；侧 B 若经合并通道排在后段（rank 14），有被截的概率。

**附带**：
- BriefWire `Option<_>` 修复生效：run5 plan fallback 50%→19%、repair 52%→22%（剩余 19% 另因，待查）；calculator Error 连续两跑为 0。
- 但 run5 总 PASS 129 < run4 133（修 7 / 新增 11 / 仍坏 9），churn 大，单跑对比噪声大；新增错题集中在 IPD 活动号类（q79/82/84）与 cross_document（q110/111/112）。趋势需再跑确认。

## 10. run6（facets 版，v2_20260816-180850，671s）结果与回归三类细查

**总分**：PASS 127（run4 133 / 单跑 churn ±5）；fallback 50%→19%→**13%**，repair 16%。**facet 采用 44/136 题；按 facet 数 PASS 率：未拆 79%、2 侧 88%、3–4 侧 100%**——拆开就有效，整体回落来自非 facet 桶。

**回归三类细查**：

1. **churn 簇（ADR/IPD 精确事实）——实为三个不同子机制**：
   - q49（DEPRECATE 组件）：合成形状事故——终答是英文思考过程泄漏（"The user asks… Let me look at the evidence pack…"），叠加检索薄。双层失败。
   - q55（RETAIN 组件）：检到 ADR-0004 及「2.3 Retain/Deprecate List」节标题，但节正文 chunk（含 EvidenceGate）未进 pack → 拒答。召回差一格。
   - q82/q84（IPD 活动号 PAC-90/100）：黄金行在语料中但写作 "PAC- 100"（带空格，精确串 %PAC-100% 为 0 命中）。q82 的 Worker grep 了「退市|停产|生命终止」，语料命中 9 行（全在 IPD 文档、含黄金行、远低于 50 条截断帽），但 eval 提取 retrieved_count=8、pack 事实全是验证阶段行——**grep 命中的黄金 chunk 在 bridge 捕获→pack 装配→eval 提取链路某处丢失，断点未定**（tool_results_count=9 证明结果进了 state.tool_results）。下一步：单题 debug 跑定位断点。

2. **base_tools 误路由（波特五力供应商）**：Lead 把「波特五力模型」当通用知识题 → base_tools brief（实际执行成 user_context，Ok）→ 不启 Worker、packs=0 → 合成诚实称未检到 → RETRIEVAL_MISS。「base_tools=自包含工具题」的边界是语义的，结构闸难挡；属模型理解力问题，prompt 边界陈述已在。

3. **残余 fallback（白药阶段等）**：离线重现该题 qwen 输出完全合法（能过 parse）——run6 的失败是采样方差或当时输出另有缺陷。**已给 `PlanRepairRequested` 事件加 `raw_preview`（log-only，原文前 300 字）**，下一跑直接看失败原文。

## 11. facets 多侧子检索（2026-08-17 实现，run6 验证）

**背景**：「每通道至多 1 brief」的 v1 闸门使多实体/双侧题的拆解责任被推给 Worker 自由文本；run4 实证 Worker 22 次调用全压单侧（§9）。

**设计（用户拍板）**：不要并发 Worker；一个 Worker 接多个子检索，**每个子检索独立预算、独立筛选**，合成时各子检索筛出的证据合并交给 LLM。

**实现**（全部在 agent-loop，436 测试绿）：

- schema：`SubTask.facets: Vec<Facet>`（`brief.rs`；`MAX_FACETS=4`）；`effective_facets()` 空 facets 退化为单子检索（facet id = sub_task id），显式 facets 作用域为 `{brief_id}/{facet_id}`。
- 解析（`lead_plan.rs`）：`FacetWire`（null 耐受、空 objective 丢弃、id 去重 first-wins、超限截断）；**facets 仅 rag brief 生效**，web 侧由 `queries[]` 扇出承载。
- 执行：`run_rag_worker_short_sac` = 外层 facet 循环 + 内层 `run_rag_facet_sac`——**逐 facet 独立消息上下文、独立 max_steps 预算**（宿主驱动循环，跳侧在结构上不可能）；alias 跨 facet 连续。`run_rag_worker_host`（re-brief host leaf）同样 facet 化。
- 装配/归并/补检：`merge_or_push_pack` 键 channel→**sub_task_id**（同通道多 facet pack 共存）；`packs_needing_rebrief` 返回空槽 sub_task_id；`host_rebrief_briefs` 只重建含空 facet 的 brief（id 保持，原位替换）。`RebriefWave` 事件字段 channels→targets。
- prompt 三处：lead-plan.system.md（schema+约束行）、lead-base.md（brief 字段）、clusters/lead/SKILL.md（schema 示例）。
- 观测：`PlanRepairRequested` 加 `raw_preview`（log-only，原文前 300 字）——下跑直接看 plan 失败原文。

**run6 结果**（v2_20260816-180850，671s）：PASS 127；facet 44/136 题，拆开就有效（2 侧 88%、3–4 侧 100%）；回归 11 道的三类细查见 §10。

**待决策（用户提出的替代执行模型）**：当前是「顺序隔离 facet」。用户设想「最大化 SaC 并发：Worker 一轮代码块把所有子检索的 client.* 调用一次写完执行」。已分析的取舍：
1. 顺序隔离（现状）：保证强、归属清晰；每 facet ≥1 次 LLM 往返，token +19%。
2. 纯共享上下文：最快、可交叉参考；但 per-facet 预算/筛选软化（工具结果归属无结构依据），「都做完」退回模型自觉（qwen 局部收敛有前科）。
3. 折中：共享上下文 + 宿主按 facet 推进（预算/pack 仍按 facet 切，模型可一轮覆盖多 facet；归属按推进时间窗）。实现多一层窗口逻辑。
   建议：先再跑一轮顺序版确认 127 是噪声还是回落，再上 2 或 3 做对比。

**开放问题清单（优先级序）**：
1. q82 型 grep 证据链断点（golden chunk 命中但丢了；tool_results_count=9 vs retrieved_count=8）——单题 debug 跑定位。
2. q49 型合成形状泄漏（英文思考过程进用户主气泡）——answer 出站闸应拦。
3. base_tools 语义误路由（波特五力 → user_context）——模型理解力，暂无结构闸。
4. 残余 plan fallback 13%——等 raw_preview 数据。
5. chat 空 caps 道 utility 题（时间/天气）模型自觉性（§6）。
6. 数据卫生：e2e PG 库 130 个 doc_id 重复灌库 ~10 代、历史代次向量与文本脱节——建议清库重建（`E2E_FORCE_INGEST=1`）。

## 12. 模型分工切换（2026-08-17）+ Worker 坚持度/q125 诊断

**切换已落地**：`run_lead_workers.rs` plan 调用点由 `llm_for_retrieve`（qwen non-thinking）改为 `synthesis_llm`（deepseek thinking max）；`with_retrieve_llm` 语义收窄为「仅 Worker SaC / retrieval 循环」（mod.rs/unified 注释已同步）。436 测试绿。预期：plan fallback 13%→<5%、拆解/路由质量提升；时延代价全量 ~11min→~16–20min。

**Worker 坚持度/工具面细查（G2 工件）**：
- **q14（目标市场）**：wave 0 Worker `gaps=rag_sac_error`（SaC 内层失败，非空结果）；新版 facet re-brief 正确触发（targets=["t1"]），但 host 补检叶**固定 lexical 单发**——BM25 对自然语言整句 0 命中，补检=复读死路。**re-brief 无换工具保证**是该簇的结构缺陷（下刀方向：补检携带前波工具/命中事实，或 host 叶 dense/lexical 轮换）。
- **q52（ADR-0009 备选方案）**：3 次调用收工，grep 未尝试 pattern 变体——Worker 坚持度老问题。
- **q60（Slice/Phase）**：facet 机制被用上了但只有 1 个 facet（t1/f1），且 objective 只覆盖 ADR-0004 侧——Lead 拆解质量问题（deepseek plan 切换后应改善）。同题出现 `Multimodal embedding failed` 瞬时错误——**embedding 服务抖动已是第 3 次在不同跑里出现，建议单独排查（MM embedding 为何被 text dense 调用触发、为何间歇失败）**。
- **q125（北京天气，chat 道）**：排除「无工具」假设——chat.yaml 明确挂沙箱 `client.weather_query`（D11）。实际是：模型（deepseek thinking max）首轮输出公告式 prose「我来查一下北京的实时天气。」，`allow_content_early_stop: true` 把公告当终答接受（`exit_reason: direct_content`，0 工具调用）。**「宣布即止」是 early-stop 接受公告句**，与「宿主不兜底」的取舍叠加——该题三种失败形态（Ok 却否认 / 沙箱错 / 宣布即止）都是模型动作选择不稳，prompt 侧可让「查询意图句不构成终答」成为事实陈述，但本质还是模型行为。

## 13. run7（v2_20260817-054848，1130s）：四改动齐验，PASS 139 新高

**改动**：deepseek plan 切换 + 天气先验解药（agent-base.md）+ key_facts 删除 + re-brief 换工具。

**总分**：**PASS 139/149（历次最高）**，recall@k **0.9293**（最高），faithfulness 0.982，correctness 0.9591。时长 1130s（deepseek plan 使全量 ~10min→~19min）；mean_total_tokens 58.7k（plan thinking 代价）。

**逐项判据**：
- **deepseek plan：决定性成功**。plan fallback 13%→**0%**、repair 0/136；faceted 73/136（拆解意愿大增）。
- **re-brief 换工具**：全 run 仅 1 次触发（证据覆盖改善），工具组合 dense+lexical+grep 正确轮换。
- **key_facts 删除**：无回归，pack 纯 evidence。
- **天气先验解药：部分生效**。模型**开始调用** `weather_query`（之前从不调用）——但随后称「工具回传为空」：mock 实际返回 22.5°C/clear sky（工件中无此数据），模型声称空。tool payload 未入工件，断点（bridge 调用失败 vs 模型误读）未定——**下一步：chat 道 tool payload 进工件/trace**。时间题新形态：模型**编造**「参数形式问题执行失败」（实际零调用）——第 4 种失败形态。

**余 10 道非 PASS**：q46/q58（ADR 实体-属性混淆：黄金 chunk rank 7 在手，deepseek 合成仍引 ADR-0009 的 06-09 作 ADR-0004 日期——合成选错，非检索）；q118/q119（chat utility 新形态）；q25/q26/q37/q117/q122/q135（judge 口径/合成对冲/全表计数/cross-doc 老面孔）。

**模型分工定型建议**：deepseek plan 的收益（fallback 0%、recall@k 新高）远大于时延代价（仍比 08-11 基线 38min 快一半）。若要回收速度，下一步选项是 plan 用 deepseek **non-thinking**（plan 是 JSON 任务，thinking 收益未必大）做一轮对比。

## 14. run8（v2_20260817-062941，1301s）：DeepSeek 服务商切到 Wafer，PASS 141 新高

**切换**：`AGENT_LLM_*` → `https://pass.wafer.ai/v1` + `DeepSeek-V4-Flash-0731-Fast`（reasoning/ZDR 均支持，输入 28¢/M）；`WAFER_ZDR=required`；llm crate 新增 wafer thinking 分支（顶层 `reasoning_effort: max|none`）与 `apply_wafer_zdr_header`（三个 header 构建点）。RETRIEVE_LLM（qwen）不动。

**总分**：**PASS 141/149（新高）**，recall@k **0.9385**（最高），correctness 0.9685，faithfulness 0.9811；plan fallback 3/136（低位稳定）；时长 1301s（与 run7 的 1130s 同量级，Wafer 略慢）。usage.model 实证 = `DeepSeek-V4-Flash-0731-Fast`（Wafer 已在产品链路生效，ZDR 头未见副作用）。

**亮点**：**天气题转 PASS**（先验解药生效，模型正常调用 weather_query 并作答）。时间题仍 INCORRECT（chat 道 utility 唯一遗留）。

**余 8 道非 PASS**：q44/q58/q63（ADR 日期/各自问题/废弃文件——合成实体-属性混淆族，检索已无罪）；q23/q25（judge 口径老面孔）；q88/q120（全表计数族）；q117（时间题）。

**模型/服务商配置终态**：plan+synthesis = Wafer DeepSeek-V4-Flash-0731-Fast（reasoning_effort=max）、Worker = dashscope qwen3.7-flash（thinking off）、judge = ollama deepseek-v4-flash:0731-cloud。

## 15. VPS E2E 迁移 + qwen_web 原生搜索（2026-08-17/18，合并 149：PASS 133）

**动机**：Wafer/Makora DeepSeek 在境外，本机走 VPN 延迟高；full-149 搬到 VPS（43.161.220.253，2C7G）跑，复用现有 PG/Milvus/Redis 容器。

**VPS E2E 依赖清单**（踩坑逐个暴露，最终闭环）：
- 预编译产物：`product_e2e` 测试二进制 + `avrag-worker`（harness 找不到会**静默 cargo build**，在 2 核 VPS 上等于死等 → 看门狗杀）。本地构建容器 `e2e-vps-builder:v1`（debian:12-slim + rust 1.97.1；apt 走阿里云 HTTP 镜像直连绕代理 502；slim 无 ca-certificates 不能走 HTTPS 源）。改代码后增量编 ~21s + rsync ~1min。
- 解析器：anydoc-extract / markitdown（venv `/opt/avrag-e2e/anydoc-venv`）、lit（`cargo install liteparse --no-default-features`，剥 tesseract 免 cmake；扫描件走 paddle OCR 远程）+ 官方 pdfium 构建（`~/.cache/pdfium-rs/chromium_7897/.../libpdfium.so` → VPS `/usr/local/lib` + ldconfig；pypdfium2 自带的 .so 缺 `FPDFText_GetCharCode` 符号，不兼容）。
- 环境：PG 隔离库 `avrag_rs_e2e_smoke`；VPS `.env` 须注释 `HTTP_PROXY/HTTPS_PROXY`（本机 WSL 网关代理在 VPS 不存在，Brave 探测曾因此失败）；`ulimit -n 65536`（默认 1024，并发 8 时 fd 耗尽连环 500）；`ANYDOC_BIN/MARKITDOWN_BIN` 指向 VPS venv（rsync 覆盖 .env 后要补）。
-  runner：`/opt/avrag-e2e/run-binary.sh`（直接跑二进制，cwd=`crates/app`，watchdog 900s/4h）。

**qwen_web provider**（web 路去 Brave 化，`SEARCH_PROVIDER=qwen_web`）：dashscope Responses API `tools=[web_search]`，非流式；sources 仅 URL（title 从 URL 派生），snippet 空 → CRW auto-scrape 兜底（VPS 已起 `crw` 容器 :3100）。两个关键修复：
1. **`tool_choice: "required"`**：不加时 qwen 对抽象/对比类 query「判断无需搜索」直接答 → web_empty（8 题）。加后 27–30 源/次。
2. **CRW 必须在线**：否则 web 证据只有裸链接，Lead 判定「无实质正文」不引用 → expect_citations web=0（3 题）。起容器后 3/3 PASS。
- 配置装配：`SEARCH_QWEN_*` 缺省回落 `RETRIEVE_LLM_*`（零新增密钥）；E2E harness 探测改打 dashscope `/models`（任何 HTTP 响应即达），凭证认 `SEARCH_QWEN_API_KEY/RETRIEVE_LLM_API_KEY/DASHSCOPE_API_KEY`。
- **注意**：生产 `avrag-worker` 镜像里没有 lit/pdfium——**生产灌 PDF 走 liteparse 必然失败**（静默 dead-letter），E2E 暴露的真实产品缺口。

**dense_retrieval `query` 别名**：qwen non-thinking 约 19% 概率把 `queries`（数组）写成 `query`（单数），host `deny_unknown_fields` 拒收 → 零证据连爆熔断（17 题同根因）。修复：`contracts DenseRetrievalArgs` 加 `query: Option<String>`（skip_serializing）+ dense.rs 归并。修后 17 题 → 15 PASS + 2 PARTIAL。

**服务商**：AGENT_LLM 从 Wafer 切 **Makora**（`inference.makora.com/v1`，`deepseek-ai/DeepSeek-V4-Flash`，reasoning_effort=max 实测可用）。

**合并 149 终局**（5 轮拼接，后跑覆盖先跑）：**PASS 133** / PARTIAL 9 / REFUSAL_WRONG 3 / RETRIEVAL_MISS 2 / UNGROUNDED+CORRECT_UNGROUNDED 各 1。速度：97 题 ~8min、41 题 6.5min（本地全量 19–21min），VPS 无 VPN 提速 ~2×。
**余 16 题分层**：检索执行 2（q097 pdf L1 流程列举、q100 cross-doc）；web 残留 3（q136 UNGROUNDED、q137/q145 纯 web 拒答——Lead 持证据仍拒，合成裁决问题）；合成 PARTIAL 8（cross_document 4、cross_adr 2、ipd_table/thesis_numeric/joint 各 1）；utility 3（q030 calculator-only、q125 天气、q117 时间）。

**生产部署（2026-08-17，rev 38759b01+dirty）**：`deploy-backend.sh`（SKIP_BUILD=1，release 由本地 16 核容器 8m36s 编出）。生产栈 = Makora DeepSeek（Lead/合成）+ qwen3.7-flash（Worker）+ qwen_web（web 路）+ CRW（127.0.0.1:3100，unless-stopped）。runtime 镜像新增 lit+pdfium（`deploy/docker/avrag-runtime.Dockerfile` + 脚本 staging，LIT_BIN/PDFIUM_LIB 环境变量可覆盖本机路径）——生产 PDF 灌库缺口随本次修复。生产 env 改动前备份 `/etc/avrag-rs/avrag.env.bak-20260817`。
