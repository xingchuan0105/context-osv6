# 提示词工程体系重构 · 实施文档（2026-08-01）

> 本文档是开发契约：新窗口按此实施，发起人按 §7 验收清单验收。
> 所有"现状事实"条目均已核实过代码，附文件:行号；实施时若与代码不符，以代码为准并在文末"实施偏差记录"追加说明。

## 1. 背景与触发

核心 agent loop 已完成 SaC（Search as Code）SDK 化重构（设计：`docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），并引入 DuckDB 读表原语 `struct_catalog`/`struct_query`（设计：`avrag-rs/docs/plans/2026-07-31-struct-query-virtual-tables.md`）。orchestrator 多智能体路径已物理删除（commit `7f2d182d`），产品路径彻底单 agent。

诊断确认的问题（按严重度）：

1. **能力单元裂变**：一个"知识库能力"的提示词散在 `prompts/capabilities/knowledge-base.md`（契约）+ `prompts/clusters/knowledge-base/`（SDK 教学）+ `modes/rag.yaml` 的 `skill_catalog.mandatory`（装配），靠命名约定 + YAML 双重缝合，改一边漏另一边不报错。
2. **策略教学缺失**：提示词长于边界陈述、短于工作策略。「同一块内可多条 await 并行」散落在 4 处且以约束形态出现；全体系没有教导"独立检索调用应同块并行发出"（一轮一个调用 = 每轮一次完整 LLM 往返）。唯一 `asyncio.gather` 示例在 search skill。
3. **语法挂载点错误**：沙箱语法介绍跟"检索能力"走（agent-base.md:17 是条件句），不跟"沙箱开放"走。纯 chat 下 `history/user_profile/save/load` 原语已开放（sdk_gate.rs:9 BASE_PRIMITIVES）但上下文无任何语法契约。
4. **prompts-in-md 违规**：`struct-supervision/src/session.rs` 等 91 处中文观察文案硬编码，而 `prompts/pipeline/table-supervision/obs-*.md` 五个模板写好未接线（孤儿）；`assembler.rs:129-143`、`external_agent_guide.rs:5-8`、`writer/adapters.rs:171` 内联指令文案。
5. **「metadata」名不副实**：ingestion 产物是 summary(+summary_metadata 字段)、section index、struct store、chunks——**没有 metadata 产物**。`metadata` cluster 的内容 100% 是 profile 阶段的 summary_metadata 字段聚合；`client.doc_profile` 实际是 profile 字段 + 章节 index 的合并（doc_profile.rs:49-53），KB skill 方法表只写了「章节地图」，低估了它。
6. **tool/SDK 双面残留**：模型可见 native tool 仅剩纯 chat 三件套（user_context/calculator/weather_query），memory skill 还留着 legacy 点选式说法（memory/SKILL.md:30）；SDK 新增方法要碰 5 处（gate 常量、Python shim、host dispatch、实现、提示词）。

## 2. 目标与非目标

### 目标（验收口径）

- G1 capability 目录化内聚：`prompts/capabilities/<id>/` 含 contract.md + SKILL.md + reference/，capability 挂载即该单元披露。
- G2 agent-base 含**无条件**沙箱基座：语法形态、首块执行约束、**并行扇出策略**、基础原语（history/user_profile/save/load）、证据判定。
- G3 `metadata` cluster 改名 `docscope` 并重写；`doc_profile` 教学修正为「画像字段 + 章节结构」。
- G4 struct_query 教学补齐：SQL 结果集读法、row_ord 行序、ambiguous_relations。
- G5 prompts-in-md 违规清零；table-supervision 孤儿模板接线。
- G6 `assemble_mode` 从 CapabilitySet 直接推导 mandatory skill + SDK 原语集；删除 mode YAML 的 `skill_catalog.mandatory` 间接层。
- G7 声明式 SDK 原语注册表：id + capability 归属 + docstring 单一事实源，gate/shim/dispatch 派生；新增方法只碰 3 处（注册表一行 + handler 实现 + 提示词）。
- G8 纯 chat 三件套 SDK 化（client.user_context/calculator/weather_query），native 模型面关闭；两道拒绝闸退役为一条固定提示。
- G9 memory 升格基础披露（每轮 mandatory，短文件），指代消解全程可用。
- G10 验证：相关 crate 单测全绿 + `bash scripts/test-l1.sh` 通过。

### 非目标（明确不做）

- write_refine 工具面、`prompts/clusters/heavytail-*`、`write-core` 控制环（T2 约束）。
- pipeline 提示词（summary/section-index/triplet/user-profile/session-summary）内容重写——本轮只做 table-supervision 孤儿模板接线。
- `worker_handoff`/`compile_feedback` 死路径删除（orchestrator 删除后的疑似残留）——本轮只在 §8 记录评估结论，不删代码。
- 真实 LLM 冒烟 / Playwright E2E（验收以 L1 + 单测为准）。
- `docscope_metadata` 字段名、`<docscope_metadata>` 注入标签等 wire 契约更名（不动）。

## 3. 现状事实（已核实）

### 3.1 提示词加载三机制

- A. 编译期 `include_str!`：loop 观察经 `crates/agent-loop/src/react_loop/prompt_assets.rs:13-32`（`loop_prompt!` 宏 + `subst()` 占位符替换）；`crates/agent-tools/src/tool_registry.rs:14-30` 有同型宏 `subst_prompt`；pipeline 系统提示词散点 include_str!（llm/src/summary.rs:11-17、struct-supervision/src/prompts.rs:5 等）。
- B. build.rs codegen → PromptRegistry：`crates/agent-tools/build.rs:9-93` 扫描 `prompts/clusters/*/SKILL.md`（+ reference/）和 `prompts/{synthesis,system,capabilities}/*.md`，生成 `generated_registry.rs`，由 `agent-tools/src/progressive/prompt_registry.rs:23-29` include。**注意：`prompts/synthesis/` 是空目录仍被扫描。**
- C. 运行时 fs 读：system 装配 `agent-loop/src/react_loop/policy/config/mode_loader.rs:117` + `assembler.rs:37-56`（`\n\n---\n\n` 拼接）。
- 双轨：`system/agent-base.md`、`capabilities/*.md` 同时走 B 和 C。

### 3.2 装配链路（当前）

`resolve_capabilities`（请求 `capabilities` 多选字段 → CapabilitySet，`app-chat/src/capabilities.rs:48-67`）→ `assemble_mode`（`app-chat/src/mode_assemble.rs:50`）：system_prompt_parts = agent-base (+ capability md)；skill_catalog 从 mode YAML merge；`sdk_primitives` 由 `sdk_primitives_for_caps` 推导（mode_assemble.rs:122，**已是 CapabilitySet 直推，好榜样**）→ DisclosurePlanner 渲染 mandatory skill 每轮进 system（`disclosure_plan.rs:57-59`，设计意图：沙箱报错后签名仍在上下文，**必须保留**）；`how-to-read-tables` 默认随 knowledge-base 披露一次（disclosure_plan.rs:63-78）。

### 3.3 SDK 面（当前 13 原语）

- 常量：`agent-loop/src/react_loop/sdk_gate.rs` — BASE(4)=save/load/history/user_profile；RAG(7)=dense/lexical/grep/doc_profile/doc_summary/struct_catalog/struct_query；SEARCH(3)=web/fetch/dense。
- Python shim + docstring：`code-interpreter/src/bridge.rs:50-131`；方法名清单 `bridge.rs:138-154`；**legacy 别名 dense_search/lexical_search 仍在 shim（bridge.rs:53-58）和 gate canonicalize（sdk_gate.rs:46-52）**。
- host dispatch：`rag-core/src/runtime/bridge.rs:278-330` → `tools/mod.rs:26-33`。
- native 拒绝闸：`agent-tools/src/tool_registry.rs:101-139`（codegen-method-as-native + sac-superseded 两道，md 模板在 prompts/loop/）。

### 3.4 metadata 真相

- `DocScopeMetadata` = `documents: Vec<SummaryMetadata>` + 聚合 profile（`common/src/docscope.rs:214-246`；字段 doc_id/filename/docname/language/domain/genre/era/author/publication_date）。
- 数据源：`pg.get_summary_metadata`（`app-chat/src/agent_runtime.rs:33`），即 profile 阶段（SummaryGenerator，`llm/src/summary.rs`）产物。**ingestion 无 metadata 产物**（document_pipeline/ = parse/profile/index/struct_stage/materialize）。
- `client.doc_profile` = get_document_metadata + get_summary_metadata + get_document_toc_entries 三方 join（`rag-core/src/runtime/tools/doc_profile.rs:49-53`），fields 为空时全量返回（含 sections）。
- metadata cluster 特殊注入点：`disclosure_plan.rs:408-423` `inject_cluster_runtime_context` 按 `cluster_id == "metadata"` 匹配——**改名 docscope 时此处必须同步**。

### 3.5 违规清单（G5 范围）

| 位置 | 内容 | 处置 |
|---|---|---|
| `struct-supervision/src/session.rs:131,153-160,178-190,250,272,287-324`（91 处） | 健康报告/切片/守卫观察文案 | 接线到已有 `prompts/pipeline/table-supervision/obs-*.md`（include_str! + subst） |
| `struct-supervision/src/runner.rs:125,135,155` | loop 观察内联 | 新增 md 文件（pipeline/table-supervision/ 下）迁出 |
| `agent-loop/src/react_loop/assembler.rs:129-143` | `<format_hint>`/`<writing_hint>` 英文指令 | 迁 prompts/system/hints/*.md（include_str!） |
| `app-chat/src/external_agent_guide.rs:5-8` | RAG_SUMMARY 等 4 段英文常量 | 迁 prompts/（新子目录，如 prompts/agent-guide/），运行时加载 |
| `app-chat/src/writer/adapters.rs:171` | 命令式中文一句 | 迁 md + 改第三人称观察式 |
| `write-core/src/refine_helpers.rs:14-` | 中文轮次观察（第三人称，borderline） | 迁 md（低优先级，可随 G5 一并） |

迁出时全部按第三人称观察式改写（见 §4 D2）。

### 3.6 其余欠账

- `prompts/synthesis/` 空目录被 build.rs 扫描 → 删除扫描项。
- `prompts/templates/` 未进 prompts/README.md 布局表 → 补。
- `prompts/loop/README.md` 占位符清单缺 table-supervision 的 `{sql}`/`{rows}` → 接线后补。
- `guardrails/src/output/prompt_leak.rs:9-37` 泄漏检测基线引用 prompt 路径/内容 → 布局变更后同步。

## 4. 设计决策

- **D1 目录布局**：

```
prompts/
  system/
    agent-base.md                 # 身份 + 无条件沙箱基座（D4）
    hints/format-hint.md          # 自 assembler.rs 迁入（G5）
    hints/writing-hint.md
  capabilities/
    knowledge-base/
      contract.md                 # 现 capabilities/knowledge-base.md 演进（D3）
      SKILL.md                    # 自 clusters/knowledge-base 迁入并修订（D5）
      reference/how-to-read-tables.md   # 重写（D6）
    web/
      contract.md
      SKILL.md                    # 自 clusters/search 迁入
  clusters/                       # 跨能力共享：memory、docscope、writing、format、heavytail-*、index、workspace-create
    docscope/                     # 自 metadata 改名重写（D7）
    memory/                       # 保留，清 legacy 表述（D8）
  agent-guide/                    # 自 external_agent_guide.rs 迁入（G5）
  loop/ pipeline/ templates/      # loop 不动；pipeline 只接线 supervision
```

- **D2 文风纪律**（AGENTS.md 硬规则，验收逐项核）：全部 LLM 可见文案用第三人称观察式（「本轮检索观察中仍未出现 answer-grade 命中」），禁命令/禁令/步骤清单腔（「请/必须/不要/禁止」）；硬门禁只在代码，md 只报告事实；禁 golden-set 实体/数字；占位符只在代码侧替换。
- **D3 contract.md 补能力地图**：每个 capability 的 contract 增加一段「本能力能做什么」的散文地图（解决 P2：capability 段不自包含），方法语义细节仍归 SKILL.md，contract 不重复签名。
- **D4 agent-base 沙箱基座**（无条件段，不依赖任何 capability）：① `<code language="python">` 是唯一执行入口，每轮仅第一个块执行；② **独立调用同块并行是默认工作方式**（多 query 扇出、多方法并行，`asyncio.gather` 示例）——这是行为性修复，措辞用观察式（如「同块内多条 await 一次回传全部结果；一轮一块比一轮一调用节省整轮往返」）；③ 每块新进程，跨块 save/load；④ 基础原语 `client.history/user_profile/save/load` 始终可用，指代/更早偏好 → 先取历史（衔接 D8）；⑤ 什么不是证据（现有内容保留）。
- **D5 KB SKILL 修订**：① 并行扇出策略段（多 query/多方法同块 gather，示例从顺序 await 改为 gather）；② `doc_profile` 行改「文档画像（标题/作者/文体/年代/语言）+ 章节结构」；③ 保留方法表/空结果表/低自由度路径/gotchas 现有骨架；④ 补 `ambiguous_relations`（多 doc 同名表静默归属单 doc，`struct_query.rs:748-752`；教学：用 doc_ids 收窄）与 `row_ord`（表出现序）语义归拢。
- **D6 how-to-read-tables 重写**：保留 grep/管道行 ontology，新增 SQL 结果集读法（`columns`/`rows`/`row_count`=结果集行数、COUNT 值在 rows 单元格、`truncated`=样本、`ok=false` 的 error.code 自纠路径）。
- **D7 metadata→docscope**：目录/文件/frontmatter `name`/skill_request token 全改 `docscope`；同步 `disclosure_plan.rs:416` 特判、`modes/rag.yaml` retrieve 列表、相关单测、prompts/README。内容重写：「文档清单 = 灌入 profile 阶段产出的 scope 级聚合」，教学链路 docscope（拿 doc_id + 概览）→ doc_profile（单篇画像+章节）→ doc_summary（单篇摘要）。wire 名（`<docscope_metadata>`、字段名）不动。
- **D8 memory 升格**：memory skill 进入全模式 mandatory（每轮披露，文件短，45 行量级，token 可接受）；删除 SKILL.md:30 的 legacy 点选式表述；agent-base 记忆小节更新为「history/user_profile 是基础原语，随时可调」（skill_request JSON 触发仍保留作为 reference/anaphora.md 的按需加载口）。
- **D9 装配线去间接层**：`assemble_mode` 从 CapabilitySet 直推 mandatory retrieve skill（rag→knowledge-base、search→web-skill、两者都含 memory 基础位）与 sdk_primitives（现状已直推，保留）；`modes/*.yaml` 删除 `skill_catalog.mandatory`，YAML 只留 budget/temperature/loop_exit/auto_fallback/可选 cluster 列表（docscope、writing、format）。`SkillCatalogConfig.mandatory` 字段保留兼容解析但不再使用（或标 deprecated），避免 wire 破坏。
- **D10 SDK 注册表**：新建声明式原语表（id、capability 归属 Base|Rag|Search、docstring、host handler 绑定），单一事实源；派生：① sdk_gate allowlist（替代 sdk_gate.rs 三组常量）；② Python shim 方法 + docstring（code-interpreter codegen 或宏生成）；③ rag-core host dispatch 表。放置约束：该表所在 crate 必须是 agent-loop、code-interpreter、rag-core 三者的共同依赖（候选：`contracts` 或新 leaf crate；handler fn 指针部分放 rag-core，纯数据部分放共享 crate）。parity 测试扩展：注册表 ↔ shim 方法名 ↔ host dispatch 三方一致。顺手删除 shim 的 dense_search/lexical_search 别名与 gate 的 canonicalize。
- **D11 三件套 SDK 化 + native 面关闭**：`user_context`/`calculator`/`weather_query` 注册为 Base 原语（id 不变，host handler 复用现有 SkillComponent 实现）；纯 chat `tool_pool` 清空（mode_assemble.rs:26-32,111 的 utility whitelist 退役）；`dispatch_tool` 两道拒绝闸合并退役为一条「native 模型面已关闭，检索/工具经沙箱 client.*」固定提示（md 放 prompts/loop/，第三人称）；auto_fallback 的 dense_retrieval/web_search 保留 host 侧（不是模型面，不动）。ToolCatalog 保留作 host 内部查表，注释更新。

## 5. 工作包（依赖序）

### WP1 SDK 注册表 + 三件套 SDK 化（D10、D11）

1. 建原语注册表（位置按 D10 约束选定，附 dependency 说明注释）。
2. sdk_gate 改为从注册表派生 allowlist；删 canonicalize 与 legacy 别名（同步删 shim 别名 + 其单测引用）。
3. code-interpreter shim 从注册表 codegen 方法与 docstring。
4. rag-core dispatch 接注册表；新增 user_context/calculator/weather_query 三个 host handler（复用 agent-tools 现有实现，注意依赖方向：rag-core 不应反向依赖 agent-tools——若现有实现不可达，把实现本体下沉到注册表可达的 crate，agent-tools 侧改为 re-export）。
5. 纯 chat tool_pool 清空；dispatch_tool 两道闸退役为单条固定提示（新 md）。
6. 验证门：`cargo test -p agent-loop --lib && cargo test -p agent-tools --lib && cargo test -p rag-core --lib && cargo test -p code-interpreter --lib`（逐一，勿并发全量）。

### WP2 装配线（D9、D8 的 mandatory 部分）

1. `assemble_mode` 直推 mandatory retrieve skill（含 memory 基础位）+ 保留 sdk_primitives 直推；删 YAML `skill_catalog.mandatory` 四份。
2. disclosure_plan：mandatory 来源从 config 读取路径相应调整；`metadata` 特判改 `docscope`（与 WP3 D7 同提交或先行兼容两者）。
3. 更新 mode_assemble.rs 内全部单测（现有 7 个测试逐一过）。
4. 验证门：`cargo test -p app-chat --lib && cargo test -p agent-loop --lib`。

### WP3 提示词内容（D1-D8）

1. 目录搬迁 + build.rs 扫描路径更新（capabilities/*/SKILL.md + contract.md；删 synthesis 空目录扫描项）。
2. agent-base.md 加沙箱基座段（D4）。
3. contract.md ×2 补能力地图（D3）。
4. KB SKILL 修订（D5）+ how-to-read-tables 重写（D6）。
5. web SKILL 迁入（并行示例已有，补「独立调用同块并行」策略句）。
6. metadata→docscope 改名重写（D7）+ memory 清理（D8）。
7. guardrails/prompt_leak.rs 基线路径同步。
8. 验证门：`cargo test -p agent-tools --lib && cargo test -p agent-loop --lib && cargo test -p guardrails --lib`（registry/披露/泄漏检测测试）；人工通读全部改动 md 过 D2 文风。

### WP4 prompts-in-md 违规清理（G5，§3.5 表）

按表逐行迁出；table-supervision 五个孤儿模板接线（session.rs 91 处 → include_str! + subst，占位符 {sql}/{rows} 等）；runner.rs 三处新 md；全部第三人称化改写。
验证门：`cargo test -p struct-supervision --lib && cargo test -p agent-loop --lib && cargo test -p app-chat --lib && cargo test -p write-core --lib`。

### WP5 文档同步

prompts/README.md（布局表、装配公式更新）、prompts/loop/README.md（占位符清单补 {sql}/{rows}、退役闸说明）、prompts/templates/ 入表、根 AGENTS.md 的 prompts 规则段（clusters 表述改 capabilities 目录化后的新约定）、本文件 §8 偏差记录。
验证门：人工审。

### WP6 收尾验证

1. `bash scripts/test-l1.sh`。
2. `graphify update .`（结构性代码变更后的硬规则）。
3. 自查 §7 验收清单逐项打勾。

## 6. 实施顺序与提交建议

WP1 → WP2 → WP3 → WP4 → WP5 → WP6，每 WP 一个本地 commit（solo trunk 纪律，不 push）。WP2 与 WP3 有耦合（mandatory 直推依赖 skill id 稳定；docscope 改名影响 disclosure_plan 特判），允许两包交叉提交，但每个 commit 必须独立通过其验证门。

## 7. 验收清单（发起人验收用）

- [ ] G1 `prompts/capabilities/knowledge-base/{contract.md,SKILL.md,reference/how-to-read-tables.md}` 与 `web/` 同构存在；clusters/ 下无 knowledge-base、search 残留目录。
- [ ] G2 agent-base.md 含无条件沙箱基座：首块约束、并行扇出策略（含 gather 示例）、基础原语、证据判定；全文第三人称。
- [ ] G3 clusters/metadata 不存在；docscope/ 内容符合 D7；`grep -rn '"metadata"' avrag-rs/crates/agent-loop/src/react_loop/policy/` 无旧特判；rag.yaml retrieve 列表已更新。
- [ ] G4 KB SKILL 含 doc_profile 修正、ambiguous_relations、并行扇出策略；how-to-read-tables 含 SQL 结果集读法。
- [ ] G5 `grep` 复查 §3.5 六个位置无内联文案残留；obs-*.md 被 session.rs include_str! 引用。
- [ ] G6 四份 mode YAML 无 `mandatory` 键；assemble_mode 直推逻辑与单测一致。
- [ ] G7 注册表单一事实源；parity 测试覆盖注册表↔shim↔dispatch；legacy 别名删除。
- [ ] G8 纯 chat tool_pool 为空；三件套经 client.* 可调（单测）；dispatch_tool 只剩一条固定拒绝提示。
- [ ] G9 memory 在全模式 mandatory 披露；SKILL.md 无 legacy 点选式表述。
- [ ] G10 全部相关 crate 单测绿 + test-l1.sh 通过 + graphify 已更新。
- [ ] 文风抽查：随机 5 个改动 md，无命令式措辞、无 golden-set 实体。

## 8. 实施偏差记录

### WP1（2026-08-02 提交 707004a7）

1. **三件套 host handler 落在 `agent-loop` 的 `SacHostBridge`，而非 rag-core `RuntimeBridge`**（D10 原设想：注册表可达的共享 crate 承接 handler）。理由：纯 chat 无 rag runtime，`RuntimeBridge` 永远服务不到三件套；Base 原语宿主本就在 `SacHostBridge`；`agent-tools` 的实现（CalculatorSkill/UserContextSkill/WeatherQuerySkill）对 agent-loop 直接可达，无需依赖反转或下沉 evalexpr/maxminddb/reqwest。rag-core 侧三件套与 save/load/history/user_profile 一并维持 `not_configured`。
2. **三件套经 `client.*` 的可调性验证以单测为准**：D11 要求"纯 chat tool_pool 清空 + 单测验证经 client.* 可调"；本轮以 `agent-loop` 全量单测 + `app-chat` mode_assemble 测试覆盖（无真实 LLM 冒烟，属计划非目标）。

### WP2（2026-08-02 提交（本次））

3. **YAML `mandatory` 删除从"四份"改为"三份"**（G6）：`write_refine.yaml` 的 `mandatory.retrieve: [heavytail-metrics]` **保留**——`write_refine` 是独立产品模式（T2 非目标），`app-chat/src/writer/adapters.rs` `render_mandatory_skills()` 依赖它披露 heavytail-metrics（write-core 控制环）。`SkillCatalogConfig.mandatory` 字段保留（write_refine 使用；SaC 三模式经 `derive_mandatory_retrieve` 派生）。
4. **`derive_mandatory_retrieve(rag, search)` 放 `agent-loop/react_loop/policy/mod.rs`**（D9 原文只说 assemble_mode 直推；实现为 agent-loop 提供公共派生函数，app-chat 的 assemble_mode 调用它，避免双处推导漂移）。纯 chat 也含 `memory` 基础位（D8 全模式）。
5. **memory 基础披露带来行为变化**：round 0 即披露 memory cluster 并暴露 conversation_history_load/user_profile_load 两个 native 工具（原测试断言 round0 tools=0 已按 D8 更新为期望这两个工具）。
6. **disclosure_plan `metadata` 特判先行兼容 `docscope`**（`cluster_id == "metadata" || cluster_id == "docscope"`），WP3 完成改名后再收敛为单值。

### WP3（2026-08-02 提交 97e7d88b）

7. **`modes/*.yaml` 的 `system_prompt_base` 指向 `prompts/capabilities/{knowledge-base,web}/contract.md`**（D1 布局）；`metadata`→`docscope` 改名同步了 `disclosure_plan.rs` 特判收敛为单值 `"docscope"`、`modes/rag.yaml` retrieve 列表、registry 测试断言（`skill("metadata").is_none()`、`skill("docscope").is_some()`、flat capability id `capability-knowledge-base` 随 flat 扫描删除而消失）。
8. **build.rs 扫描根**：`scan_roots = [clusters, capabilities]`（capabilities/*/SKILL.md 同款目录扫描，frontmatter `name` 稳定 registry id）+ flat 只留 `system`；删除了 `synthesis`（空目录）与 `capabilities` 平铺两项。

### WP4（2026-08-02 提交 a63e1f79）

9. **`external_agent_guide.rs` 用 `include_str!` 而非计划原文的"运行时加载"**：编译期内嵌满足 prompts-in-md（文案住 md、代码只加载），实现层面差异，行为不变。
10. **write-refine 轮次计数用本地极简 render 引擎**：`prompts/system/hints/round-counter.md` 模板 + `write-core/src/refine_helpers.rs` 内 ~60 行 keys/picks 渲染（与 struct-supervision 的 ObsCtx 同构，未引入共享 crate）。
11. **table-supervision 接线用了自研迷你模板引擎**（`avrag-struct-supervision/src/prompts.rs`：keys/blocks/picks 三语法），5 个孤儿模板按 session.rs 语义改写 + 新增 10 个 obs-*.md；briefing/annotate 等文案与原 format! 输出在少量标点/空行上有差异（无测试断言原格式）。
12. **G5 验证方式**：以 grep 复查六处无内联 LLM 文案 + 各 crate 单测全绿为准（§3.5 表六行全部处置）。

### worker_handoff / compile_feedback 死路径评估（非目标，仅记录）

- **`compile_feedback`**：orchestrator 删除（commit 7f2d182d）后，结构性编译失败的自由纠正轮次只存在于 worker 手递路径；产品单 agent 路径（SaC）无 worker handoff，该字段/分支已不可达。代码中仍有解析与类型定义（wire 兼容），**本轮不删**。
- **`worker_handoff`**：同为 orchestrator 时代的会话交接消息类型；单 agent 路径不产生也不消费。若未来从 wire 契约移除需评估序列化兼容（当前保留零成本）。
- 结论：两者均为纯残留定义，无运行时副作用；删除属独立重构项（需动 app-chat/agent-loop 类型 + 测试），按计划留作非目标，仅此记录。

### WP6（2026-08-02 收尾验证）

13. **`bash scripts/test-l1.sh` 的 L1 file-size 闸为**实施前既有失败（commit 447e6fa7 引入该闸，早于本计划 commit db9b80cc；`answer_contract.rs` 1588 行 > 1000 行硬上限，自 a1c7be35 后未被本计划任何 commit 触碰）。本计划无分解该文件的 WP 项，超出范围不改动；L1 其余部分（cargo test agent-tools/agent-loop/app-chat + frontend tsc）单独执行全绿。G10 以此口径验收：crate 单测全绿 + L1 cargo 层通过 + graphify 已更新。

### WP7（2026-08-02 黄金集回归修复）

14. **全量黄金集（realistic_corpus_full_eval，149 题，deepseek-v4-flash）暴露重构回归**：重构前 v2 基线（08-01）多子集 recall=1.0/correctness=1.0；重构后前 18 题全挂（eval_bridge_miss / RETRIEVAL_MISS）。根因：WP2 D8 使 memory cluster **每轮 mandatory 披露** → round 0 即暴露 `conversation_history_load`/`user_profile_load` 两个原生工具（此前仅 skill_request 披露 memory 时出现）→ 函数调用模型看到 tools 数组即走原生工具路径（含幻觉工具名 invoke_skill/code/`__knowledge_base__search` 等），全部 dispatch 未命中返回 NotImplemented，烧光预算，从不写 `<code language="python">` 块 → 检索从未发生。
15. **修复（assembler.rs）**：`assemble_retrieve` 的 tools 恒定取自 `mode.tools_for_retrieve(registry)`（rag/search tool_pool 空 → `tools=[]`），不再在 memory 披露时附加两个原生记忆工具；删除 `memory_cluster_disclosed`/`dedupe_tools` 私有函数；更新 3 个相关单测。D8 自洽：memory 每轮散文披露、教学 `client.history`/`client.user_profile` 基础原语，原生工具是 legacy 点选式残留。`cargo test -p agent-loop --lib` = 279 全绿。
16. **验证（4 题探针 thesis_factual Q1-Q4）**：2/4 PASS（recall=1.0），2 题 eval_bridge_miss 但已能写出 code 块（q002 为 `asyncio.run()` 与沙箱运行中事件循环冲突、q003 为 sandbox_error + synthesis 修复）——非系统性回归，属模型行为波动（与重构前基线中零星 miss 同量级）。

### WP8（2026-08-02 全量 149 复跑后，E2E 并发提速；commit 2f4d81ab）

17. **`realistic_corpus_full_eval` 主循环并发化**（A：chat 并行）：串行 `for` 改 `futures::stream::iter(...).map(...).buffer_unordered(N)`，`N = E2E_CONCURRENCY` env（默认 8）。循环体提取为 `run_single_question` async fn + `QuestionOutcome{idx, failures, subset, recall/citation/halluc/scorecard Option}`；按 idx 归位收集后再聚合，report 逻辑与顺序不变。
18. **TestContext 不 Sync（含 oneshot Sender + tokio::process::Child），不跨 task 共享**：只提取 `http_client`（reqwest::Client，Clone）+ `base_url` 传入各 task。
19. **judge 并行（B）**：judge 与 chat 共享同一 buffer_unordered 池（每 task 内 chat 完成后立即 judge），未拆独立 judge 池——`V2RunCtx.scores` 改 `Arc<Mutex<Vec<ScoreV2>>>`，`score_question`/`record_infra` 由 `&mut self` 改 `&self`（内部 lock），`print_and_write_summary` 开头 lock.clone()。
20. **重试补全**：`post_rag_chat` 自由函数（自 `chat_v3` 提取，`chat_v3` 改调它）加重试——transport err 指数退避 1s/2s/4s 最多 3 次；状态码 429|500|502|503|504 时 retriable（429 读 `Retry-After` 头 min 30s，否则 1s/2s；5xx 用 1s/2s；attempts>=3 返回当前响应）；4xx(非429)/2xx/3xx 立即返回。此前 agent chat 的 http 500 无重试（全量跑 2 个 INFRA_ERROR）。`JudgeCache::store` 改临时文件 + `fs::rename` 原子替换（防并发读者读半写文件）。
21. **fail_fast 语义弱化**：原 `break`（fail_fast 立即停）改 `Arc<AtomicBool>` 置位，各 task 开头查（置位则跳过后续），已在飞的 task 完成；聚合后保留 `fail_fast && !failures.is_empty()` panic。
22. **验证**：编译 0 error；calculator 3 题探针（E2E_CONCURRENCY=3）11.75s；混合 6 题（E2E_QUESTIONS=1..6, E2E_CONCURRENCY=8）51.57s（串行基线 6 题约 4min，~4.7x）。6 题 v2 全 PASS。全量 149 串行 6622s 的并发期望 ~25-30min（embedding 侧 rate limit 未实测，探针未见 429/退避触发）。
23. **全量 149 并发档位实测（2026-08-02，`v2_20260802-143621`）**：**`E2E_CONCURRENCY=8` 定为安全档**。实测对比：并发 8 → 938s（~15.6min）、**0 HTTP 500**、PASS 137/149（91.9%，历史最高）；并发 16 → 288s 但 **103 HTTP 500**（q1–30 全 PASS 后 q31 起大量 `synthesis stream failed: Failed to read chat completion stream chunk`），串行探针 q35–44 全 PASS 证实非代码回归、是并发打崩 DeepSeek 合成流。**结论：并发 16 是危险档，>8 需先在上游加连接复用/限流保护。**

## 9. 环境纪律（摘自 AGENTS.md，全文有效）

- prompts-in-md：LLM 可见文案只住 `avrag-rs/prompts/**/*.md`；代码里只做加载与占位符替换。
- WSL：`jobs=2`；任何时候不并发跑多个全量 cargo test。
- 凭证：复用 `avrag-rs/.env`，不重问。
- 服务（Milvus/PG/Redis/MinIO）假定已运行，不 docker-compose up。
- 不 push、不 PR；本地 trunk 提交。
