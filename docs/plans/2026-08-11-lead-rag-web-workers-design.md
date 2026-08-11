# Lead Agent + RAG/Web Workers 设计评估与迁移决断

| 字段 | 内容 |
|------|------|
| **日期** | 2026-08-11 |
| **状态** | **W0–W4 + 审查收尾** — Lead LLM plan；RAG 短程 SaC；HostWeb 直出已删；每通道 1 Brief；无 host dense 再接线；实机冒烟需新 binary |
| **动机** | 把检索 agent 范式迁回 **显式激活的 capability subagent**；Lead 负责指代消解 / 拆解 / 合成与 **覆盖度裁决**；Workers 只检索与证据压缩 |
| **范围** | 产品 agent-lane：凡 `capabilities` 含 `rag` 和/或 `search` 均走 Lead+Workers；chat / write_refine 不在本设计 |
| **非目标** | 完整 multi-agent 产品框架 UX；独立 verify 环（本路径）；golden 对照式 host 裁决；osv7 实现细节（契约可迁移） |
| **核查** | 2026-08-11 四路代码/文档核对：主干成立；4 处现状失准已修；6 个实现缺口写入 §13 |

---

## 0. 一句话

**前端显式打开的每个检索 capability 对应一个短程 Worker；Lead 是唯一持有全局目标、覆盖度裁决与用户终答权的角色；Workers 只回结构化证据，禁止代答；无独立 verify 环（产品路径上 verify 已关，本设计追认并物理收尸）；最终回答必须 grounded 在证据上，证据不足时承认缺口而不是硬编。**

这不是回到 7 月 orchestrator 全套复杂度，而是 **吸取 SaC 与通道混用教训后，把「通道隔离 + 证据契约」做成一等公民**；裁决权归 Lead，不叠第四个 LLM。

---

## 1. 历史对照：我们为何离开 / 为何再回来

### 1.1 时间线（与本方案相关）

| 时间 | 决策 | 文档 / 代码 | 对今天的含义 |
|------|------|-------------|--------------|
| 2026-07-15 | capabilities 多选；空 = chat | `CAPABILITIES_MULTISELECT_…`；`capabilities.rs` | **产品面保留** |
| 2026-07-16 | Orchestrator + Rag/Search Worker + Chat | `ORCHESTRATOR_SUBAGENT_CHAT_…`（**SUPERSEDED**） | 角色祖先；终答权分离正确 |
| 2026-07-30 | SaC 单 agent：砍 orchestrator/worker | `plans/2026-07-30-sac-…` A2 | 减分层税；**通道隔离失败**遗留 |
| 2026-08-07 | retrieve→synthesis→verify 三环上线 | `…-verify-loop-design.md`；loop 代码完整 | 纵向职责拆分；仍同脑混用 KB+web |
| 2026-08-10 | Harness 不进用户主气泡 | `…harness-llm-user-channel…` | **用户信道法则仍强制** |
| 2026-08-11 | search-only **host_web** 瘦身 | `…host-web-thin-loop.md`；`run_retrieval` early return | Web 叶子可宿主化；**直出用户气泡**待 D3 撤销 |
| 2026-08-11 | **产品关 verify** | `rag.yaml` / `search.yaml`：`verify: false`（注释 product cost） | 三环代码**休眠**；D6 = **追认 live + 物理删休眠路径**，不是「关掉一个仍在烧 LLM 的活闸」 |
| 2026-08-11 | 本设计 + 代码核查 v1.1 | 本文 | Lead+Workers 定稿 |

### 1.2 单 agent 留下的真实痛点（证据级）

1. **通道偏斜**：同一 ReAct 脑偏好易用通道而跳过 workspace 检索，再声称「用户未提供文档」（7 月 dual 手测）。  
2. **协议与预算混绑**：KB 与 web 抢同一 `max_iterations` 与同一系统提示。  
3. **减轮次与 grounding 冲突**：软基线压延迟时更易用先验补全；历史上 verify 曾抓过「薄证据 + 先验」类 fail，但 **今日产品路径 verify 已关**，不宜再把「verify fail 统计」当作现行主论据（见 §8 D6）。  
4. **search-only 已 exit SaC retrieve**：`run_retrieval` 在 ReAct 前 early return，`host_web_user_answer` 把 DeepSeek `synthesized_answer` **直灌用户气泡**——属实；本设计 D3 改掉「直灌」，保留「host 叶子产证据」。

### 1.3 与提案的契合度总判

| 提案要素 | 判定 |
|----------|------|
| Lead + 两 Worker；显式 capability | **采纳** |
| Workers 禁止终答 | **强制** |
| Task Brief / EvidencePack | **采纳（agent 间）** |
| 禁止预训练知识 | **L0–L3**；拒 L4 实体硬扫 |
| 多门禁 | **结构门 host；语义门 Lead** |
| JSON 终答契约 | **内部 telemetry；出站 prose + 引用** |
| 独立 verify | **无**（追认 product-off + 收尸，见 D6） |

### 1.4 代码现状勘误（核查 2026-08-11）

下列曾被写「过满」的说法，以代码为准：

| # | 曾写 | 实际 | 对迁移的影响 |
|---|------|------|----------------|
| **K1** | 三环 verifyverify 为待删活机制 | `verify: false` 已产品关；代码休眠 | D6 论据 = **追认 + 删休眠**，非对抗 live 三环 |
| **K2** | `auto_fallback` dense 为可迁「现网零件」 | `run_fallback` / YAML `auto_fallback` / `execute_search_no_scrape` **已删**；RAG SaC 失败/空 **不** 再 host dense 补救 | **锁定：不接线**；空/失败 → host 从已有 ToolResults 装 pack（可 empty/insufficient） |
| **K3** | 「CRW 仍在 SearchExecutor 后丰富」一笔带过 | enrich 在 `executor` 内；**唯一入口 `execute_search`**（`WEB_AUTO_SCRAPE*` 可关） | Web Worker pack 路径 **CRW on（默认）**（§6.2 / §13.3） |
| **K4** | 「Web 保留 host 叶子」暗示 dual 已如此 | dual **结构性**不能 `host_web`（`is_search_host_web_path` 要求无 dense 等）；dual web 今日 = 沙箱 `client.web`/`fetch`（可带 CRW） | dual Web→host 叶子是 **变更**，不是保留；`client.fetch` 去留见 §6.2 |

**仍核实为真：** dual 单脑 SDK 并集（`sdk_gate` + `dual_is_union` 测）；capabilities 多选；结构门 evidence/required_action；`host_markers` parity；SELECTED/KEEP/alias；`[[web:n]]`；沙箱 `dense/lexical/grep/save/load`（无 top_k）；§11 文档路径与 SUPERSEDED 标注。

---

## 2. 目标架构（v1 定稿形态）

### 2.1 角色

```text
用户输入 + 会话历史 + capabilities[] + Lead 规划上下文（§13.2）
                    │
                    ▼
        ┌───────────────────────┐
        │      Lead Agent       │
        │  指代消解 · 拆解 · Brief │
        │  调度 · 覆盖度 · 合成   │
        │  （唯一用户终答权）     │
        │  BASE 题可短路径（§2.4） │
        └───────────┬───────────┘
           仅当 cap 已激活
         ┌──────────┴──────────┐
         ▼                     ▼
   RAG Worker            Web Worker
   (KB 短程 SaC)         (host 多 query 叶子)
         │                     │
         └──────────┬──────────┘
                    ▼
            EvidencePack[]
                    │
                    ▼
         Lead 裁决 + 合成 → 用户 prose
```

| 角色 | 持有 | 禁止 |
|------|------|------|
| **Lead** | 完整对话历史、全局目标、packs、合成与覆盖度裁决；**可选** BASE 工具通道（§2.4） | dense/web **检索**原语（不破坏通道隔离） |
| **RAG Worker** | Brief、doc_scope、KB SDK + BASE 子集中与沙箱会话相关的原语 | web、用户终答 |
| **Web Worker** | Brief、host search 叶子（+ 可选 fetch 策略） | dense/grep、用户终答 |

### 2.2 与 7-16 的差异（有意简化）

| 7-16 | 本方案 v1 | 理由 |
|------|-----------|------|
| Orchestrator + Chat Agent | **Lead 兼调度与合成** | 少一层 LLM |
| 默认多跳 re-dispatch | 默认单波；**最多 1× re-brief** | 延迟 |
| EvidencePack / handoff | **`evidence_pack_v1` 新契约**；旧 `internal_worker_handoff_v1` 收尸或仅写线 | 无兼容税 |

### 2.3 产品能力 → Worker 物化

| `capabilities` | 物化 | 路径 |
|----------------|------|------|
| `[]` | 无检索 Worker | **现 chat 路径**（BASE only） |
| `["rag"]` | RAG Worker | Lead + Worker + Lead 合成 |
| `["search"]` | Web Worker（host 叶子） | Lead + pack + Lead 合成（禁 DeepSeek 直出） |
| `["rag","search"]` | 两 Worker **默认可并行** | 完整路径；主价值场景 |

**LLM 不得取消用户已选 capability。** 开了 RAG 且 doc_scope 非空、题面依赖文档时，「未派 RAG」= Lead 覆盖度 gap。

### 2.4 BASE 原语归属（核查缺口 #1 — 锁定）

`SdkCapability::BASE`（`sdk_gate`：**无论 caps 永远并入**）：含 `history` / `user_profile` / `user_context` / `calculator` / `weather_query` / `save`/`load` 等（以 `contracts::sdk_primitives` 真源为准）。

| 原语族 | 归属 | 理由 |
|--------|------|------|
| **weather_query / calculator** | **Lead 可调用**（短程 tool / 现有沙箱 BASE 子集，**不**经 RAG/Web Worker 伪装） | 题面为工具题时不应塞进 `preferred_source: rag\|web`；Ok weather 旁路语义归 Lead 直接交付或合成 |
| **user_context / history / user_profile** | **Lead 规划与合成可见**；Worker **默认不**注入完整 history | Lead 持全局；Worker 只收 Brief 摘要 |
| **save / load** | **仅 RAG Worker 沙箱**（短程工作集）；Lead/Web 不用 filesystem 传 pack | pack 走结构化回传，不走 A7 文件 handoff |
| **纯 BASE 题**（caps 含 rag/search 但 Lead 判定无需检索） | Lead **可不派 Worker**，直接工具 + 人话答 | 覆盖度：无 pack 且声明 expect_no_retrieval 类 → 允许；若 cap 要求 grounding 仍须诚实 |

**`preferred_source` 枚举扩展（Brief）：** `rag | web | base_tools | none`。  
Host 启动门：`base_tools` / `none` **不启**检索 Worker。

---

## 3. 契约（内部；不进用户主气泡）

### 3.1 Task Brief（Lead → Worker）

```json
{
  "schema_version": "task_brief_v1",
  "original_query": "指代消解后的完整问题",
  "conversation_context_summary": "≤ N 字前序摘要",
  "sub_task": {
    "id": "t1",
    "objective": "自包含子目标",
    "boundaries": "只做检索与证据压缩；禁止回答完整用户问题",
    "preferred_source": "rag | web",
    "queries": ["可选：Web 通道 host 扇出用，1–N 条已消解 query"],
    "max_steps": 4,
    "success_criteria": "完成判据（可观测）"
  },
  "output_schema": "evidence_pack_v1",
  "grounding_rule": "key_facts 与 evidence 必须可追溯到本轮检索 observation / host hits"
}
```

**Host 启动门：** `original_query` 非空、`objective` 非空、`preferred_source` ⊆ 激活通道、`max_steps` ∈ [1,5]；web 时 `queries` 缺省则用 `[original_query]`。

### 3.2 EvidencePack（Worker → Lead）— 去掉不可验证自报字段

```json
{
  "schema_version": "evidence_pack_v1",
  "sub_task_id": "t1",
  "channel": "rag | web",
  "key_facts": ["来自检索压缩的事实"],
  "evidence": [
    {
      "content": "原文或压缩摘录",
      "source": "doc_id | url",
      "score": 0.0,
      "provenance": "chunk_id / page / paragraph",
      "alias": "#3 | web:1"
    }
  ],
  "coverage": "sufficient | partial | insufficient",
  "gaps": "缺失说明",
  "tool_ok_count": 0
}
```

**删掉 `used_only_retrieved_content`：** 模型可谎报，信号为零。grounding 靠 host 可计算字段 + Lead 义务。

**`citations[]` 不与 `evidence[]` 并列：** 引用以 `evidence[].alias` / `source` 为真源；Lead 合成时再生成用户侧 `[[web:n]]` / `（#n）`。

**Host 证据门（结构）：**

| 检查 | 失败动作 |
|------|----------|
| JSON 可解析且 `channel` 匹配 | `coverage=insufficient`，`gaps=malformed_pack` |
| `tool_ok_count` **由 host 从 ToolResults 重算**，不信任模型填写 | 覆盖模型值 |
| `tool_ok_count ≥ 1` 才允许 `coverage=sufficient` | 否则降为 partial/insufficient |
| `evidence[]` 每条非空 `source` | 剔除无源；清空 → insufficient |
| `key_facts` 空且标 sufficient | 降为 partial |
| host 装配 partial（收束失败） | 只含真实 Ok hits |

### 3.3 Lead 出站

| 内部 | 用户可见 |
|------|----------|
| telemetry：`coverage_assessment`, `grounded`, `limitations`, `rebrief_used` | **自然语言 prose** |
| 引用协议 | 文档 `（#n）`/`SELECTED`；网页 `[[web:n]]` |
| 证据不足 | Lead 人话缺口；**禁止** host 脚注 |

---

## 4. 状态机与门禁

### 4.1 主状态机

```text
ResolveCaps
  → [caps empty] ChatPath
  → LeadPlan（注入 §13.2 规划上下文）
  → PlanGate
  → [仅 base_tools / none Briefs] → 可跳过 Dispatch
  → DispatchWorkers（并行 | 串行）+ Progress Delegate*
  → WorkerRun
  → PackGate（host 重算 tool_ok_count）
  → 聚合 gaps → Lead 可见观察
       ├─ host 结构：已产 pack 且空/insufficient 且 rebrief_used < 1 → ReBrief → Dispatch
       └─ LeadAdjudicateAndSynthesize（无单独 re-brief 决策 LLM）
  → Deliver prose
```

### 4.2 门禁表

| 门禁 | 硬度 |
|------|------|
| 规划门 / Worker 启动门 / 步数门 / Pack 结构门 / rebrief≤1 | **Host** |
| 覆盖度是否 hard-closeout、主张是否可支撑 | **Lead**（prompt）；host 只注入观察 |
| 出站 DSML / 协议泄漏 | **Host 格式闸** |

### 4.3 与三环（休眠代码）的关系

| 三环组件 | 处置 |
|----------|------|
| Retrieve | **Workers** |
| Synthesis | **Lead** |
| Verify 模块 / skill 调用链 | **本路径不调用**；W0–W3 间 **删除或隔离** 休眠入口，避免再被 YAML 误开 |
| `forbid_retrieve_direct_answer` 等 | Worker 无用户交付权，语义由状态机保证 |

**D6 论证（修订）：** 不是「战胜仍在运行的三环」，而是 **产品已 `verify: false`（K1）→ 本设计追认 → 物理收尸休眠代码 + 裁决归 Lead**。通道隔离治根因（痛点 1–2）；不依赖复活 verify 治标。

### 4.4 预算（最坏路径 — 修订）

| 池 | 次数 |
|----|------|
| Lead plan | 1 |
| Lead plan repair | 0–1 |
| RAG Worker | ≤ `max_steps` LLM（每 Brief） |
| Web Worker | **0 LLM**（host 叶子）；若启用短程 fetch 路径另计 |
| Re-brief 波 | ≤1 ×（**host 结构触发**；仅已产 pack 且空/insufficient 通道；无 Lead 裁决 LLM） |
| Lead 最终合成 | 1 |

**Lead 最坏 LLM 次数 ≈ 3：** plan + plan repair + synth（re-brief **不**另计 Lead 决策轮）。  
**延迟（dual，无 re-brief 常见路径）：** plan + max(RAG wall, Web wall) + synth。  
**Web wall：** `ceil(N_queries / concurrency) × search_latency`（+ 可选 CRW，见 §6.2）。

为合成 **预留** token/轮，Worker 不得把全局预算吃光。

---

## 5. 「禁止使用模型内置知识」——可执行分层

| 层 | 机制 |
|----|------|
| L0 prompt | Lead/Worker 文案 + grounding_rule |
| L1 host | PackGate；`tool_ok_count` host 重算；无源剔除 |
| L2 引用 | alias / `[[web:n]]` 与 pool 对齐 |
| L3 Lead | 合成时覆盖度与缺口人话 |
| L4 实体硬扫 | **不做** |

允许：行文组织、同义压缩、明确「未覆盖」说明。  
禁止：空命中编造 key_facts、用预训练补关键数字/实体、insufficient 时装完整答。

---

## 6. 检索与 Worker 决断

### 6.1 RAG Worker — SaC 下沉

| 零件 | 去向 | 备注 |
|------|------|------|
| `dense`/`lexical`/`grep`/`save`/`load` | **仅** RAG Worker | |
| KB prompts | RAG Worker | |
| docscope | Worker 启动注入；Lead 规划见摘要（§13.2） | |
| soft baseline | Worker `max_steps` | |
| **auto_fallback dense** | **已删且不接线（K2）** | YAML / `run_fallback` / SaC 失败后的 host dense 补救均 **不存在**；Worker SaC 内自行 `client.dense` 等；失败/零 Ok → host **仅**从已有 ToolResults 装 pack |
| query_card / plan-query | 上收 Lead 或取消重复 | |
| SELECTED/KEEP/alias | pack.evidence | |

形状：Brief → short SaC ≤ max_steps → **宿主始终**从 ToolResults 装配 `evidence_pack_v1`（**无**模型 pack 收束轮；`worker-sandbox` 已写明）。

### 6.2 Web Worker — host 叶子（含 dual **变更**）

| 路径 | 今日 | 目标 |
|------|------|------|
| search-only | host **单** query + **no_scrape** + **直出用户气泡** | Lead plan → host **多 query 并行** → pack → **Lead 合成** |
| dual web | 沙箱 `client.web`/`fetch`（可 CRW） | **改为 host 叶子**（与 search 同引擎）；**不是**保留 |

#### 6.2.1 CRW 策略（K3 — 锁定）

| 场景 | CRW |
|------|-----|
| **Web Worker 产 pack（本设计默认）** | **`WEB_AUTO_SCRAPE` 遵循 SearchExecutor 默认 enrich**（与 bridge `client.web` 一致：厚 snippet）——**开** |
| 旧 search-only 直出 / no_scrape 路径 | **已删**（`execute_search_no_scrape` 不存在） |

理由：Lead 合成需要可读正文；CRW 成本进 Web wall，换 grounded 质量。实现：Web Worker 唯一走 **`execute_search`**（内含 enrich；由 `WEB_AUTO_SCRAPE*` 控制）。

#### 6.2.2 多 query 扇出（核查缺口 #4 — 交付物）

Web Worker host 叶子对 Brief.`queries[]` 1–N（建议 ≤4–5）做 **`join_all` / 并发** SearchProvider，合并 results 去重 URL 后装 pack。

- **波次归属：** W0 类型 + 纯函数合并；**W1 实现并行 host search**（dual 硬依赖）；W3 search-only 复用。  
- 非「免费保留」。

#### 6.2.3 `client.fetch` 去留

| 选项 | 锁定 |
|------|------|
| 默认 Web Worker | **无沙箱**；CRW 已写 snippet → **不需要**模型 fetch |
| 例外 | re-brief 且 gaps 要求特定 URL 深读：Brief 可带 `urls_to_fetch[]`，host 调 CRW/fetch **一次**，仍 0 模型 web ReAct |
| 产品 SaC `client.fetch` | dual 旧路径删除后，**仅**若某日重开「多步 web ReAct」才挂回；v1 **不挂** Web Worker 沙箱 |

### 6.3 Lead — 规划 / 合成 / 裁决

| 阶段 | 实践 |
|------|------|
| 指代消解 | 会话历史 → `original_query` |
| 规划上下文 | §13.2 强制注入 |
| 复杂度 / Brief 数 | **每激活通道至多 1 检索 Brief**（rag≤1、web≤1；dual ≤2）；PlanGate 同通道后到丢弃（先到保留） |
| dual | 默认真并行（`tokio::join!`） |
| 合成+裁决 | §8.2 |
| re-brief | ≤1，**host 结构触发**（D4） |

### 6.4 提示词资产

| 资产 | 路径建议 |
|------|----------|
| Lead | `prompts/clusters/lead/` 或 `prompts/system/lead-*.md` |
| RAG/Web Worker | `prompts/workers/{rag,web}/` + 现 capability manuals |
| 门失败观察 | `prompts/loop/*` + **host_markers** |
| few-shot | 禁止 golden 实体 |

语气：第三人称环境事实；硬门在 host。

### 6.5 进度 UX（核查缺口 #2 相关）

Worker 跑 3–4 步时用户可见过程卡：

- **复用** `progress/mod.rs` 的 `DelegateRag` / `DelegateSearch`（及 `WorkFact::delegate`）发「通道已派发 + brief 摘要」。  
- RAG 步内可继续现有 Searching / CodeExecution 类事件。  
- **不**把 Brief/pack JSON 原文丢进用户主气泡。

### 6.6 既有残骸去留（核查缺口 #2 — 锁定）

| 残骸 | 处置 |
|------|------|
| `worker_contract.rs`（alias 元数据等） | **审计后**：仍被 alias 计数依赖的 **保留精简**；orchestrator 专用注释改写为 Lead+Workers |
| `output_compiler` / `internal_worker_handoff_v1` | agent-lane **停用**；能删则删；测试夹具迁到 `evidence_pack_v1` |
| `progress` DelegateRag/Search | **复用**（§6.5） |
| config「channel worker」类 flag | **删除**双轨 flag；新路径即主路径 |
| `app-chat` `SubagentInvoker`（writer） | **写线保留**；与 agent-lane Lead **不共享**生命周期，文档互指「非本设计」 |
| 休眠 verify 调用链 | 本路径 **不调用**；W3 后可删死代码（与 D6 一致） |

---

## 7. 迁移策略

### 7.1 原则

1. **无长期双轨**；旧 union dual 与 host 直出删除。  
2. **分层生长 vs dual-first：** 见 §7.2 辩护。  
3. 复用 SearchProvider、bridge SaC、CRW、citation；**新做** host 多 query。  
4. v6 全量 W0–W4；契约可迁 osv7。

### 7.2 波次顺序与 layered growth

**张力：** 最小端到端本应是 rag-only；本表把 **W1 dual** 放在 single-cap 前。

**辩护（锁定采用 dual-first 契约逼近）：**

1. 本设计 **主价值**是通道隔离；rag-only 验不出 dual 偏斜修复。  
2. Brief/pack/调度/并行 host web **一次做对**，避免 single-cap 契约二次撕裂。  
3. W0 仍是最小可测切片（类型+门+prompt，无 live LLM 调度）。

**可选并行加速（不改依赖方向）：** W2 rag-only 的 Worker 运行时与 W1 RAG 腿同构，可与 W1 同 PR 落地「单 Worker 路径」，但 **验收 dual 仍是 W1 闸**。

| 波次 | 交付 | 验证闸 |
|------|------|--------|
| **W0** | 契约类型；PackGate（host 重算 tool_ok）；markers；Lead/Worker prompt 骨架；BASE 分类；残骸清单落地删/留；**AGENTS.md + docs/README 同步**；host 多 query **纯函数**合并单测 | lib 测绿 |
| **W1 dual** | LeadPlan → 并行 Workers → re-brief≤1 → Lead 合成；**host 多 query + CRW enrich**；Progress Delegate；删 dual union | 双开 KB 必触或明确 gap |
| **W2 rag-only** | 全经 Lead + RAG Worker | full149 子集 / 冒烟 |
| **W3 search-only** | 删直出；Lead 合成；复用 W1 web 叶子 | 统一引用；墙钟可接受 |
| **W4** | telemetry、空库空网、预算预留、dormant verify 清扫 | 回归 |

### 7.3 删除清单（迁完）

- dual 单脑 KB∪web union  
- search-only DeepSeek/host **用户气泡直出**  
- 本路径 verify 二次 LLM（及误开入口）  
- 不可验证的 `used_only_retrieved_content` 依赖  
- prompt-only「请两边都查」

### 7.4 文档治理（含 AGENTS.md — 缺口 #6）

**W0 启动时同一提交或紧随提交必须改：**

| 文件 | 动作 |
|------|------|
| **根 `AGENTS.md`** | 三环 / verify 表改为 Lead+Workers 权威；stop decision、bypass（weather 等）改挂 Lead；删除「retrieve→synthesis→verify 默认」作为产品路径叙述 |
| `docs/README.md` | 基线句：agent-lane = Lead+Workers；SaC 在 RAG Worker |
| `plans/2026-07-30-sac-…` | A2 横幅：agent-lane 编排被本设计取代；SDK 下沉保留 |
| `…host-web-thin-loop.md` | 横幅：叶子保留；直出被 D3 取代；CRW 策略见本设计 §6.2 |
| `…verify-loop-design.md` | 横幅：产品 off；本路径收尸 |

---

## 8. 产品拍板（锁定）

| # | 锁定 | 含义 |
|---|------|------|
| **D1** | 全部经 Lead | 统一入口 |
| **D2** | 短程 SaC RAG | max_steps 3–4 |
| **D3** | search 必经合成 | 禁直出；host 只产 pack |
| **D4** | re-brief ≤1 | **host 结构门**（仅已跑通道空/insufficient）；不发明 Lead 未派通道；Lead LLM 二次裁决未接 |
| **D5** | v6 W0–W3（+W4 加固） | 全量 |
| **D6** | **无独立 verify** | **追认** yaml `verify:false`；裁决归 Lead；删/隔离休眠调用 |

### 8.1 延迟代价

```text
常见 search-only: LeadPlan + WebWorkerHost(N queries, +CRW) + LeadSynth
最坏 Lead LLM: ~3（plan + repair + synth；§4.4）
```

### 8.2 Lead 裁决义务

1. 读 packs（coverage/gaps/evidence；信任 host 重算的 tool_ok_count）  
2. 区分有证 / 不足  
3. 不足人话缺口；材料侧未见的关键事实不补全  
4. 引用对齐 alias  

**补料：** 不由 Lead 再请求 dispatch。Host 在 pack 聚合后对「已产 pack 且空/insufficient」通道结构 re-brief ≤1；Lead 在合成轮读 `[rebrief_wave]` / 最终 packs。

Host：结构门、rebrief 计数、格式闸、telemetry、**进度 Delegate**、**始终 host 装配 pack**。

---

## 9. 对提案原文的修订摘要

| 提案点 | 修订 |
|--------|------|
| 终答 JSON | telemetry；用户 prose |
| used_only_retrieved_content | **删除** |
| citations 并列 evidence | **合并进 evidence[]** |
| 幻觉后处理 L4 | 不做 |
| SaC | 进 RAG Worker |
| host_web | 叶子 **变更扩展**到 dual；直出删；**CRW 开**；**多 query 新做** |
| verify | 追认 off + 收尸，非战胜 live 三环 |

---

## 10. 验收标准

- [ ] rag/search/dual 均经 Lead；无 host 直出用户气泡  
- [ ] 通道隔离：RAG 无 web 原语；Web Worker 无 dense/grep  
- [ ] Workers 只出 EvidencePack / host partial  
- [ ] `tool_ok_count` host 重算；无自报 grounding flag  
- [ ] BASE 题不错误塞进 rag/web preferred_source  
- [ ] dual host 多 query 并行 + CRW enrich（默认）  
- [ ] Progress Delegate 可见；主气泡无 pack JSON  
- [ ] re-brief ≤1；无独立 verify LLM  
- [ ] AGENTS.md 与本设计一致  
- [ ] 无 union dual / 直出兼容开关  

---

## 11. 相关文档

| 文档 | 关系 |
|------|------|
| `ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md` | 角色祖先 SUPERSEDED |
| `plans/2026-07-30-sac-sdk-single-agent-design.md` | A2 被取代；SDK 下沉 |
| `…retrieve-synthesis-verify-loop-design.md` | 休眠；D6 收尸 |
| `…harness-llm-user-channel…` | 用户信道 |
| `…search-mode-host-web-thin-loop.md` | 叶子祖先；直出被 D3 取代 |
| `…crw-auto-scrape.md` | enrich；与 no_scrape 分叉 |
| `…agent-loop-fewer-rounds…` | 并行 / 减轮 |
| `plans/2026-08-11-osv7-go-rewrite-design.md` | 契约可迁移 |

---

## 12. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-08-11 | 初稿 + 产品拍板 D1–D6 |
| 2026-08-11 | **v1.1 代码核查并入**：K1–K4 勘误；D6 改「追认现状」；预算最坏 4×Lead；dual-first 辩护；§2.4 BASE；§6.2 CRW/多 query/fetch；§6.5–6.6 进度与残骸；Pack 去自报字段；§7.4 AGENTS.md；§13 缺口闭合表 |
| 2026-08-11 | **W0 实现**：`lead_workers` 契约 + PackGate + web merge；markers/prompts；AGENTS 同步 |
| 2026-08-11 | **W1 实现**：dual `assemble_mode` → `LeadWorkers`；`run_lead_workers` 并行 RAG host dense + Web host leaf（`execute_search`+CRW）；Delegate 进度；pack 注入 → synthesis；**暂用** host-dense 代替短程 SaC（D2 完整版后续） |
| 2026-08-11 | **W2 实现**：rag-only 亦 `LeadWorkers`；`modes/rag.yaml` `retrieve_strategy: lead_workers`；单测断言 rag-only 路径 |
| 2026-08-11 | **W3 实现**：search-only → `LeadWorkers` + 必经合成；删 DeepSeek/host 用户气泡直出；`search.yaml` loop_exit 对齐；web skill 文案更新 |
| 2026-08-11 | **W4 实现**：host 结构 re-brief≤1（空/insufficient 通道；RAG 补 lexical）；pack merge；`[rebrief_wave]`；Evaluation + DebugTrace telemetry；单测 |
| 2026-08-11 | **后续序**：Lead `fetch_lead_briefs`；RAG `run_rag_worker_short_sac`（Box::pin 嵌套）；删除 `HostWeb`/`run_host_web_retrieve`；pipeline prompts lead-plan |
| 2026-08-11 | **审查 P0–P1 修复**：BASE-only plan→空检索列表；prompts 单源；第三人称 plan/sac；PackGate 空证→insufficient；dual/web 真并行；rebrief 不强制未派通道；plan 注入对话史；嵌套 usage 累计 |
| 2026-08-11 | **再审查收尾**：lead/worker 系统提示与 SKILL 回写第三人称；PlanGate **每通道 1 Brief**；撤 SaC host dense 再接线；正文对齐 re-brief=host 结构、pack=host 装配、HostWeb 命名退役 |

---

## 13. W0 前缺口闭合表（核查第三节）

| # | 缺口 | 闭合位置 | W0 动作 |
|---|------|----------|---------|
| 1 | BASE 原语无家可归 | **§2.4** | 类型 `preferred_source` 含 base_tools/none；Lead BASE 通道测 |
| 2 | 残骸 / 进度 UX | **§6.5–6.6** | 清单执行：Delegate 复用；handoff 停用；writer invoker 隔离 |
| 3 | Lead 规划上下文 | **§13.2**（下） | 注入结构单测 |
| 4 | host 多 query | **§6.2.2** + W1 交付 | W0 合并纯函数；W1 并行 IO |
| 5 | used_only_retrieved_content | **§3.2 删除** | 契约无此字段 |
| 6 | AGENTS.md 治理 | **§7.4** | W0 同步改文 |

### 13.2 Lead 规划时注入上下文（缺口 #3 — 锁定）

LeadPlan 的 model 可见输入 **至少**包括：

| 块 | 内容 | 来源 |
|----|------|------|
| 用户当前句 | raw message | 请求 |
| 会话历史 | 近 k 轮（与现 chat 同截断策略） | session |
| capabilities | 已激活 rag/search | 请求 |
| **doc_scope 摘要** | 若 rag 激活：doc_id 列表 + 标题/短画像（**不是**全文）；空 scope 显式「本轮无挂载文档」 | workspace / 现 docscope 同源 |
| workspace 提示 | workspace_id 存在性（无内部 id 对用户念经） | 请求 |
| 环境 | 可选 user_context 已取结果（若 plan 前 host 预取） | 工具 |

**据此** Lead 才能执行 §2.3「doc_scope 非空且题面依赖文档」的覆盖度义务。  
**不**在 plan 阶段注入 dense 命中正文（那是 Worker 的事）。

### 13.3 快速决策卡（实现时勿再讨论）

| 项 | 值 |
|----|-----|
| dual web | host 叶子 + 多 query + **CRW on** |
| search 用户气泡 | Lead 合成 only |
| verify | 不调用；收尸 |
| auto_fallback | **已删且不接线**（含 SaC 失败 host dense） |
| re-brief | ≤1，host 结构触发（非 Lead LLM） |
| brief/通道 | 每通道 1；PlanGate 先到保留 |
| pack 收束 | 始终 host 从 ToolResults 装配 |
| pack grounding flag | 无自报；host tool_ok_count |
| BASE 工具 | Lead |
| Progress | DelegateRag/Search 复用 |
