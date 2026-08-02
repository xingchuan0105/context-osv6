# 全量 149 黄金集 · LLM 行为分析报告（2026-08-02）

> 范围：提示词工程体系重构（WP1–WP6，commit 707004a7…ddbfb592）后的首次全量真实 LLM 黄金集回归。
> 命令：`E2E_MODE=nightly RAG_EVAL_V2=1 RAG_EVAL_V2_ONLY=1 cargo test -p app --test product_e2e realistic_corpus_full_eval --features product-e2e -- --ignored --test-threads=1 --nocapture`（5482.83s，exit 0）。
> 数据源：`crates/app/tests/e2e_output/rag_eval_v2/v2_20260801-211307/per_query.tsv` + `e2e_output/realistic_corpus_full_eval/qNNN.json`（mode_debug.activity_counts / tool_trace / answer）。

---

## 1. 运行与总体成绩

| 指标 | 值 |
|---|---|
| 样本 | 149 题（21 subset） |
| Recall@15 | 88.57%（v2 judge recall 90.92%，n=145） |
| v2 correctness（均值） | **0.903** |
| v2 faithfulness（均值） | **0.929** |
| v2 relevancy（均值） | **0.955** |
| label 分布 | PASS=128，RETRIEVAL_MISS=7，SELECTION_MISS=5，JUDGE_ERROR=5，INCORRECT=2，INFRA_ERROR=1，REFUSAL_WRONG=1 |
| 硬闸失败 | 20 条（多闸复计：eval_bridge_miss 10、G-17 utility gate 7、expect_citations 3、empty_answer 1） |

对比重构前基线：07-24 pre-SaC 全量跑 HTTP_500=76（检索路径大面积 500）；本跑 **0 HTTP_500**，检索链路（SaC 沙箱 codegen → bridge → host dispatch）整体健康。

---

## 2. 失败归因：以 LLM 行为为核心

21 个非 PASS 按模型行为归为 5 类（JUDGE/INFRA 为外因，其余全部是模型行为）：

| 类 | 计数 | 题号 | 一句话 |
|---|---|---|---|
| A. 零检索直答 / 意图叙述泄漏 | 4 | 65,92,119,138 | 模型不写代码，终答是参数化记忆或自己的规划叙事 |
| B. 代码块即终答 + 方法名幻觉 | 3 | 16,25,26 | 模型写好了并行代码，却把代码块本身当终答 |
| C. 表类题检索过载 + 预算耗尽 | 6 | 62,78,79,86,88,106 | 检索 30–238 次/题，烧满预算，仍选错 |
| D. 纯 chat 工具三件套不用 | 5(闸) | 124,125,147,148,149 | 三件套移入 SDK 后无提示词教导，模型完全不用 |
| E. judge/infra 外因 | 6 | 4,40,53,83,87,144 | judge JSON 解析失败 / 空答 |

### A. 零检索直答 / 意图叙述泄漏（4 题，全 RETRIEVAL_MISS）

**子型 A1 —— 参数化记忆直答（q065、q119）**：RAG 已挂载（caps=['rag']），模型却完全不检索，用先验知识 + hedging 作答。
- q065「Salesforce 全球 CRM 份额」：答「20%~24%，本轮未挂载联网检索，无法给出可核实数值」——**没试知识库**就自认无据。
- q119「网络效应 70% 与创投研究是否一致」：全程 `client.web` 都没用（0 次），直接凭记忆给出「不构成一致结论」的论断式答复。

**子型 A2 —— 意图叙述泄漏为终答（q092、q138）**：模型终答就是自己的**规划叙事**，无代码、无检索，被循环当 DirectAnswer 收下。
- q092「云南白药 IT 规划冒烟通过率」：终答 =「The user asks about… I need to search the knowledge base… Let me do a parallel fan-out」（英文规划草稿原文泄漏，454 字）。
- q138「滴灌通 DRC 全称」：终答 =「我先从工作区文档范围内摸清与『滴灌通 / DRC』相关的材料，用语义检索、字面检索和行级查找并行取回证据。」（53 字，纯意图陈述）。

> 行为特征：模型把「下一步计划」当「回答」输出了。这与 D2 文风（第三人称观察式、多报告环境事实）可能存在诱导关联——模型学会了「陈述将做什么/环境是什么」的腔调，但在部分题上把叙述本身当成了产出，未进入执行态。128/149 正常作答，说明是少数题的转弯失手，不是系统性。

### B. 代码块即终答 + 方法名幻觉（3 题，全 RETRIEVAL_MISS）

q016/q025/q026 模型**写出了正确的并行检索代码**（`asyncio.gather(client.dense, client.grep, …)`），但：
- 终答 = 代码块原文（`<code language="python">…`），从未转成散文 → 触发 `synthesis_code_answer_repair`（5 次）但未救回；
- `sandbox_error=1`：执行时出错，tool_results=[]；
- q016 用了**不存在的 SDK 方法 `client.docsummary`**（注册表正确名是 `doc_summary`）→ 方法名幻觉直接炸首块。

> 行为特征：模型能正确套用沙箱基座并行示例，但**没学会「代码→观察→散文终答」的回合结构**——把代码块当作可交付的最终输出。

### C. 表类题检索过载 + 预算耗尽（6 题：5 SELECTION_MISS + 1 REFUSAL_WRONG）

华为 IPD 流程表类题（q078/079/086/088）与跨文档（q106）、ADR 对比（q062）：
- **检索工作量爆炸**：SELECTION_MISS 平均 act_retr=119.8 次/题（PASS 平均 37.8，**3.2 倍**）；q088 单题 `act:retrieve_doc=207`（doc_profile/doc_grep 反复轰），q106=120、q086=121。
- **全部 budget_exhausted=1**（跑满 12 轮预算），其中 4/6 已 retrieve 到 recall=1.0，但 **cite/选择 miss**——证据在库、答案结构对，最终没选中 gold 主张。
- 工具画像：doc_grep×10、dense/lexical/graph×4、struct_catalog×1（q088）——**反复全量扫，不做 doc_ids 收窄**（SKILL 已教学「用 doc_ids 收窄后再查」，模型未执行）。

> 行为特征：表类多主张题上模型倾向「大范围多次检索」而非「先 catalog 再精确窄化」，扇出叠加轮次上限 → 预算烧尽，且选择精度不足。

### D. 纯 chat 工具三件套不用（2 INCORRECT + 5 条 G-17 闸，直接命中）

**根因：D11 把 user_context/calculator/weather_query 移入 SDK 后，没有任何提示词教导 `client.calculator / client.user_context / client.weather_query`。**
- 核查：`prompts/` 全树无 `client.calculator/user_context/weather_query`；agent-base.md 基础原语只列 `history/user_profile/save/load`；纯 chat 无能力 SKILL 披露三件套；chat.yaml `tool_pool: [user_context]` 已被 assemble_mode 清空（D11）。→ 模型**不知道这些原语存在**。
- q124「现在日期和时间（精确到分）」：模型**幻觉了具体时间**「2026年8月2日 06:27」（INCORRECT）——本可 `client.user_context()` 取真实值。
- q125「北京天气」：模型在沙箱里硬试 HTTP 抓取两轮（一次被安全策略拦、一次无有效字段），**不知道有 `client.weather_query`**，最终拒答（INCORRECT）。
- q147/148/149 计算题：答案正确（模型心算），但 G-17 gate 要求 tool_results 含 calculator → 闸 FAIL。**计算题模型永远心算、永远不碰计算器原语**。

### E. judge / infra 外因（6 题，非 LLM 行为）

- **JUDGE_ERROR×5**（q004/040/083/087/144）：judge 输出 JSON 带尾逗号（`trailing comma at line 25`），schema 校验失败重试仍败。这些题检索全部正常（act_retr 12–56），答案内容好——**判分器格式 bug，不是模型失败**。
- **INFRA_ERROR×1**（q053）：ADR-0009 方法题，模型基于多轮记忆（act:memory=4）答出 1209 字内容，但本会话无检索层 tool_result → harness 判 INFRA。

---

## 3. 检索过程评估（轮次 / 效率 / 错误）

### 3.1 方法使用分布（149 题 tool_results 全量）

| 方法 | 次数 | 备注 |
|---|---|---|
| doc_grep | **507** | 绝对主导（grep 式字面检索） |
| lexical_retrieval | 220 | |
| dense_retrieval | 217 | |
| graph_retrieval | 80 | |
| struct_catalog | 34 | 表结构查看 |
| doc_profile | 34 | |
| web_search | 32 | 仅 search/dual 能力 |
| doc_summary | 12 | |
| conversation_history_load / user_profile_load | 3 / 2 | memory 原生工具披露回归已消（修复生效） |
| invoke | 2 | 残留工具名 |
| web_fetch | 1 | |

**三个关键读数：**
1. **struct_query = 0（从未使用）**：WP1 注册表一等公民的 SQL 读表原语，149 题一次没被调用；struct_catalog 用了 34 次（看表结构）但从不往下发 SQL。教学-采用缺口。
2. **doc_grep 一家独大（507）**：字面检索成为默认武器，dense/lexical 均等靠后——与「低自由度路径」教学（行计数→grep total_hits）一致，模型偏好确定性检索。
3. **memory 原生工具几乎归零**：D8 修复后 round0 不再暴露 conversation_history_load/user_profile_load（合计 5 次，均为历史遗留），client.history 基础原语走桥接记录，回归已消。

### 3.2 轮次与预算

- **budget_exhausted 合计 44/149 ≈ 30%**：近三分之一题目跑满预算上限（rag 12 轮）。**PASS 中也有 36 次**——大量答对的题也是「烧到顶」才收尾，效率有提升空间。
- 检索工作量（act_retr/题）：PASS 37.8、SELECTION_MISS **119.8**、JUDGE_ERROR 23.2、REFUSAL_WRONG 33。
- tool_results/题：PASS 8.0（≈8–9 轮平均，含并行块）。

### 3.3 并行扇出

- 修复后 asyncio.gather 示例生效：tool_results 常单块多条（dense+lexical+grep 一次回传），一轮一块多处检索成为主流形态。
- **副作用**：并行扇出 + 轮次上限 = 单题检索量上限显著抬升，表类题（C 类）把扇出当「扫射」，几十上百次仍失焦——扇出策略缺「窄化后再查」的执行纪律。

### 3.4 错误面

- sandbox_error：8 次（5 次在 PASS 内自纠，3 次直接导致 RETRIEVAL_MISS = q016/025/026）。
- synthesis_code_answer_repair：5 次（代码块作终答，B 类同源）。
- format/nudge 类活动：合计低，无系统性格式错误。

---

## 4. 结论与建议

**总体结论**：重构后的 SaC 检索路径健康——recall 90.9%、0 HTTP_500、memory 原生工具回归清零、并行扇出全面生效。失败面（21/149）主要集中在 **4 类 LLM 行为问题 + 1 处教学缺口 + 1 个 judge bug**。

**建议（供决策，未实施）**

- **S1（必修，D11 自洽）**：给三件套补提示词教导——agent-base.md 基础原语列表扩为 `history/user_profile/save/load/calculator/user_context/weather_query`（或新增纯 chat 能力披露页），否则 G-17 5 题持续闸挂、q124 会持续幻觉时间。
- **S2（观察 → 若频发则修）**：终答形态 guard——模型把「意图叙述 / 代码块」当终答时，宿主用观察式 nudge（prompts/loop/）提示「本轮消息是计划或代码，尚未产出证据支撑的散文回答」。
- **S3（观察）**：struct_query 0 使用——在 KB SKILL「默认低自由度路径」提高 struct_query 优先级（表内计数/过滤已教，需让模型从 struct_catalog 之后主动下钻 SQL）。
- **S4（观察）**：表类题检索过载——SKILL 已教 doc_ids 收窄，模型未执行；可加观察式句「同一表反复全量扫描时，先 struct_catalog 取 doc_id 收窄再查」。
- **S5（harness 侧）**：JUDGE_ERROR 5 次为 judge 输出尾逗号格式 bug，修 judge 提示词的 JSON 约束即可，非 LLM。
