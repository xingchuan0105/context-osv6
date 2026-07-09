# Graph 检索通道分析与改进路线（2026-07-04）

基于 realistic 7-doc 语料的 graph 通道端到端验证：golden set v2 评测、推理轨迹分析、
KG 数据审计、跨文档连通性量化。本文档记录已完成的改动、实测数据、已确认的 bug、
架构结论与分级改进路线。

---

## 1. 已完成的改动（本轮）

### 1.1 方案 A：graph_search `entities` 参数接线

沙箱 shim 的 `graph_search` 此前只传 `query`（仅用于关系路径 rerank），
`query_entities` 恒为空 → Milvus BFS 无种子 → 永远返回空。已打通全链路：

```
client.graph_search(query, depth=2, entities=["A", "B"])
  → RPC payload.entities
  → GraphRetrievalArgs.query_entities   (contracts/src/tool_call.rs, #[serde(default)])
  → GraphSearchRequest.query_entities   (rag-core/src/runtime/tools/graph.rs)
  → Milvus BFS seed_entities            (storage-milvus/src/ops/graph.rs)
```

改动文件：
- `contracts/src/tool_call.rs` — `GraphRetrievalArgs` 新增 `query_entities: Vec<String>`
- `crates/rag-core/src/runtime/bridge.rs` — 解析 `entities`（数组或逗号分隔字符串）
- `crates/rag-core/src/runtime/tools/graph.rs` — 透传 `args.query_entities`
- `crates/code-interpreter/src/bridge.rs` — Python shim `graph_search(query, depth=2, entities=None)`

### 1.2 Prompt 收窄（路由信号）

- `prompts/clusters/codegen/SKILL.md` / `prompts/orchestrators/rag-system.md`
- graph 触发条件：≥2 具名实体 + 结构关系信号（交集型「同时/既…又…」、
  路径型「经由/映射到/追溯到」、跨文档「两篇文档名同句」）
- 去掉裸「哪些」触发；单跳属性题（X属于哪个/由谁负责）显式排除 graph
- `entities` 参数用法：query 供 rerank，entities 供 BFS 种子（链两端实体）

### 1.3 Golden set v2（`tests/rag_quality/golden_set_graph.json`）

- negative_controls（4 题，`graph_must_not_call: true`）+ multi_hop（5 题，
  `expected_tool: graph_retrieval`）
- MH5（IPD 角色交集）已移除：FPDT-58/59 未被抽入 KG，图上无法成立
- multi_hop 题新增 `seed_entities` 文档字段（serde 忽略，不影响评测）

### 1.4 Eval v3 结果（`crates/app/tests/e2e_output/realistic_graph_eval_v3.log`）

| 指标 | v2 | v3 |
|------|----|----|
| negative_controls 误调 graph | 0/4 | **0/4** |
| multi_hop graph_called | 0/6 | **3/5 (60%)** |
| answer_correct | 10/10 | **9/9 (100%)** |
| source_recall | 0/10 | 0/9（评测盲区，见 §3.3） |

---

## 2. 架构现状：graph 检索到底怎么工作

`storage-milvus/src/ops/graph.rs::search_graph` 分两个环节：

| 环节 | 机制 | 语义（向量）参与情况 |
|------|------|---------------------|
| **种子确定** | `entity_names`（graph_hints）原样插入；`query_entities` **小写化**后插入；`query_entity_vectors` 走 `entity_dense` ANN 扩种子 | 设计支持，**但全链路无人填**（`tools/graph.rs:62` 恒传 `Vec::new()`）——死参数 |
| **BFS 行走** | 每跳 `subject in [...] || object in [...]` **精确字符串匹配**，过滤范围 = org + doc_scope（跨全部文档） | **无**，设计上就没有 |

另：`relation_dense`（关系向量）灌库时已算已存，检索时完全未使用。

**关键结论 1**：引擎天然支持跨文档遍历（hop filter 覆盖整个 doc_scope，按实体名 join），
不存在也不需要「跨文档边」这种存储对象。跨文档路径 = 两篇文档的关系共享同名实体。

**关键结论 2**：当前系统严格说不是 vector graph RAG——是「向量索引了实体、
但用字符串精确连接的 graph」。语义唯一的挂载点（种子 ANN）没接线。

---

## 3. 已确认的 bug 与评测盲区

### 3.1 Bug：种子小写化 vs 关系原始大小写（P0）

`storage-milvus/src/ops/graph.rs:85` 把 `query_entities` 转小写，但
`kg_relations.subject/object` 存原始大小写（`bins/worker/src/pipeline/graph_index.rs:127`
原样写入）。Milvus `in` 过滤大小写敏感。实测（realistic 语料）：

| 种子 | 命中 |
|------|------|
| `runtimebridge`（小写化后） | 0 |
| `RuntimeBridge`（原样） | 2 |
| `evidencegate` | 0 |
| `EvidenceGate` | 1 |

后果：`RuntimeBridge`/`EvidenceGate`/`LlmProvider` 等混合大小写种子全部失效，
只有天然小写的标识符（`graph_search`、`parse_rag_plan_decision`）能命中。
这是 v3 中 graph 每次仅返回 1 条（raw_hits=1）的直接原因之一。

**修法**：种子先查 `kg_entities.normalized_name`（该字段本就是小写）解析回规范名
`name`，再用规范名进 BFS。不要直接去掉小写化（`entity_names` 路径未小写，行为需统一）。

### 3.2 Bug：```json 围栏被当 Python 执行（P0，每题浪费一轮）

`app-chat/src/agents/loop/parse.rs::extract_all_markdown_code_blocks` 提取围栏
**不看语言标签**。模型合成答案时惯用 ```json 包裹 → 被当代码送沙箱 →
`NameError: name 'null' is not defined` → 多烧一轮 + 一次 LLM 调用后模型才改裸 JSON。
在 Q5/Q6/Q8 三条 probe 轨迹中 **100% 复现**。

**修法**：提取围栏块时跳过语言标签为 `json` 的块（放行给 Content 路径走合成解析）。

### 3.3 评测盲区：source_recall 恒为 0

`app-chat/src/chat_streaming.rs:56` 在 done 事件里剥掉 `tool_results[].data`
（防超客户端流窗口），评测 `extract_retrieved_chunks` 从中取 chunk 自然恒 0。
SSE trace 事件（`tool_result.code_gen`）里的数据是完整的（单条 ~116KB 含全部 chunks）。

**修法**：评测改从 SSE trace 或 citations 取数；顺带把 `graph_called` 指标升级为
`graph_cited`（graph chunk 进入最终引用）——Q6 已证明这才是有效贡献信号。

---

## 4. 跨文档多跳：量化事实与结论

### 4.1 实测数据（realistic 语料，5 篇有关系的文档）

- 540 实体 / 383 关系；7 篇中 2 篇（baiyao、consulting_platform）关系数为 0
- 任意两篇文档间实体名**精确重叠：0**；忽略大小写后新增重叠：**0**；
  两篇 ADR 间连子串包含的近似名都没有
- 词汇割裂根因：adr-0004 抽取全英文（`native tool calling`、`complete_with_tools`），
  adr-0009 中英混杂（`桥接 shim`、`检索后端`）——同一概念两套词汇
- 文档内链也断：adr-0004 抽了「parse_rag_plan_decision is deprecated」，
  没抽「被 structured tool parsing 替代」这条边

### 4.2 推理轨迹证据（probe Q5/Q6/Q8）

- **Q5（MH1）**：路由正确（识别跨文档信号，传了 entities），但 graph 两次各返回
  1 条；答案实际由 lexical（88–100 hits）撑起
- **Q6（MH2）**：graph 首轮调用，返回的关系块「doc_scope 强制于 Rust」
  **被最终答案引用**——graph 通道首次可验证的证据贡献
- **Q8（MH4）**：模型判断「三个子问题各自独立、无结构关系信号」→ 不调 graph。
  判断正确，**是 golden 标错**（题面实为三个独立单跳打包）
- 路由有随机性：Q6 在 probe 调了 graph，在 v3 eval 同题未调。60% 的
  multi_hop graph_called 单次运行方差大

### 4.3 「不做跨文档边影响检索效果吗」——分层答案

- **端到端答案质量**：当前不影响（实测 9/9）。agent 多轮循环本身是多跳机器，
  跨文档证据链靠多轮检索拼齐
- **graph 通道自身**：影响已发生——跨文档题上 graph 退化为单文档关系查询
  （每次 1 条），价值形态 = Q6 那种单文档关系直接命中
- **题型分界**（假设语义种子已接线，见 §5.2）：
  - **两端具名题**（题面点名两篇文档的实体，MH1/MH4/MH6 形态）：**不受影响**。
    语义种子同时命中两篇文档 → 各返回局部子图碎片 → LLM 合成时拼接。
    检索结果不需要连通路径，只需要两块碎片都被捞回
  - **中间环节未知 / 路径 / 聚合题**（「A 经由什么影响 Z」「谁跨阶段最多」）：
    **受影响且语义救不了**——行走环节是字符串精确匹配，向量无参与。
    这类题需要词汇收敛或别名表
- **代价三项**：多轮换正确率的效率成本（Q8 用 8 次调用 3+ 轮）；
  大语料下 lexical 首轮命中率下降、多轮循环可靠性衰减；关系型题型无兜底

### 4.4 增量灌入场景

查询时按名 join 的设计**天生增量友好**：新文档实体名与旧文档一致，
灌入即连通，零回填。「天然成边」的关键在词汇收敛，不在图结构：

| 机制 | 增量成本 |
|------|---------|
| 抽取规范化（提示词规则） | 零 |
| 词表注入（灌新文档时把语义相关的已有实体名注入抽取 prompt） | 每篇 +1 次向量检索 |
| 别名表（新实体向量近邻匹配旧实体，归组） | 每篇 1 次 ANN，不碰旧数据 |
| ~~显式跨文档边合成（成对 LLM）~~ | 随语料增长，**不建议**作生产路径 |

---

## 5. 改进路线（分级，做完一级验证再进下一级）

### P0 — bug 修复（✅ 2026-07-04 已实现）

1. **种子大小写** ✅ — `storage-milvus/src/ops/graph.rs::resolve_seed_entities`：
   `query_entities` / `entity_names` 经 `kg_entities.normalized_name` 解析为规范 `name`
2. **```json 围栏** ✅ — `app-chat/src/agents/loop/parse.rs`：跳过 `json` 语言标签围栏块
3. **待验证**：重跑 `realistic_graph_eval`（v4），关注 graph raw_hits 与轮数变化

### P1 — 语义种子接线（✅ 2026-07-04 已实现）

- `rag-core/src/runtime/tools/graph.rs::embed_query_entities`：`args.query_entities`
  过 `embedding_client.embed` 填入 `query_entity_vectors`；embed 失败降级为纯文本种子
- 可选进阶（未做）：**关系向量直搜**——`relation_dense` ANN 独立召回入口

### P2 — 抽取质量（改提示词，需重灌）

- 实体名规则：优先照抄原文代码标识符/专有名词（反引号内容原样），
  不翻译、不加「结构体/机制」后缀
- 关系密度：要求抽「替代/实现/依赖」链式关系，不只「属于」属性关系
- 重灌后重跑跨文档重叠度检查（脚本见 §6），验证共享实体从 0 上涨
- 注：2 篇 0 关系文档的抽取覆盖问题一并观察

### P3 — 按需投入（题型分布证明需要再做）

- 词表注入 / 别名表（§4.4）：服务「中间环节未知/路径/聚合」题型与大语料
- 决策依据：生产日志对用户 query 做题型分类统计，关系型问题占比说话
- Golden 修正：MH4 重写为真链式问法或改 expected_tool；MH2 缺半数实体同理

---

## 6. 验证命令与关键文件

```bash
# graph eval（全量 9 题）
cd avrag-rs && source scripts/e2e-env.sh
E2E_MODE=nightly cargo test -p app --test product_e2e realistic_graph_eval \
  --features product-e2e -- --ignored --nocapture --test-threads=1

# 推理轨迹 probe（1-based 题号，artifacts 落 e2e_output/rag_quality_smoke_v5/{run}/graph_qN/）
RAG_GRAPH_QUERIES=5,6,8 E2E_MODE=nightly cargo test -p app --test product_e2e \
  realistic_graph_observable_probe --features product-e2e -- --ignored --nocapture --test-threads=1

# 跨文档实体重叠度检查（P2 重灌后的验收脚本）
python3 - <<'EOF'
from pymilvus import MilvusClient
from collections import defaultdict
c = MilvusClient(uri='http://127.0.0.1:19530')
org = "org_id == '00000000-0000-0000-0000-000000000001'"
rows = c.query(collection_name='avrag_e2e_00000000_rag_kg_relations', filter=org,
               output_fields=['doc_id','subject','object'], limit=1000)
doc_ents = defaultdict(set)
for r in rows:
    doc_ents[r['doc_id']].update([r['subject'], r['object']])
docs = list(doc_ents)
total = 0
for i in range(len(docs)):
    for j in range(i+1, len(docs)):
        inter = doc_ents[docs[i]] & doc_ents[docs[j]]
        if inter:
            total += len(inter)
            print(f'{docs[i][:8]} x {docs[j][:8]}: {sorted(inter)}')
print('total cross-doc shared entity names:', total)  # 当前基线：0
EOF
```

| 文件 | 作用 |
|------|------|
| `crates/storage-milvus/src/ops/graph.rs` | BFS 实现；§3.1 bug 位置（L85）；语义种子挂载点（L63） |
| `crates/rag-core/src/runtime/tools/graph.rs` | graph_retrieval 工具；P1 接线位置（L62） |
| `crates/rag-core/src/runtime/bridge.rs` | shim RPC → ToolCall；entities 解析 |
| `crates/code-interpreter/src/bridge.rs` | Python shim（graph_search 签名） |
| `crates/app-chat/src/agents/loop/parse.rs` | §3.2 bug 位置（围栏提取） |
| `crates/app-chat/src/chat_streaming.rs` | §3.3 data 剥离位置（L56） |
| `bins/worker/src/pipeline/graph_index.rs` | 三元组 → Milvus 行；实体 normalized_name 生成 |
| `tests/rag_quality/golden_set_graph.json` | v2.0.0，9 题（MH5 已移除） |
| `prompts/clusters/codegen/SKILL.md` | 工具路由 + entities 用法 |
| `crates/app/tests/e2e_output/realistic_graph_eval_v3.log` | v3 基线结果 |

---

## 7. 图增强通道（graph augment，2026-07-05 落地）

承接 `2026-07-04-vector-graph-rag-upgrade.md` §2：图检索从「LLM 显式 `graph_search`」
降级为 dense/lexical 的**自动增强通道**，由 `RuntimeBridge::call` 在
`dense_search` / `lexical_search` 时并发执行 `graph_augment`，结果以独立键
`graph_context`（`chunk_type=graph_relation`）并入 observation，不混入 `chunks`。

- **开关**：`RETRIEVAL_GRAPH_AUGMENT`（默认 off）；冗余控制见
  `GRAPH_AUGMENT_MAX_RELATIONS` / `GRAPH_AUGMENT_SEED_LIMIT` / `GRAPH_AUGMENT_HOPS`
- **显式 `graph_search` 工具保留**（§2.5 分工不变）
- **评测口径**（`realistic_graph_eval`）：主指标 **`graph_cited`**（关系 chunk 进入最终
  citations）；`graph_called` 在增强开启时失去区分度。A/B 对比：
  `RETRIEVAL_GRAPH_AUGMENT=0` vs `=1`（§3.3）；辅助观测 `graph_context_hit`、
  selection_precision、耗时 — 见 upgrade 文档 §3

---

## 8. 操作禁忌

- 不要自动重灌 realistic 语料（重灌是显式一次性操作：
  `RAG_QUALITY_REALISTIC_TRIPLET_ENABLED=1 … realistic_graph_reindex`）
- PG 查询需带 org 上下文（RLS），不要裸查
- Milvus e2e 集合前缀 `avrag_e2e_00000000`，保留策略由
  `E2E_PRESERVE_MILVUS_ON_DROP=1` 控制
