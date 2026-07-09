# 谓词归一化两阶段设计：registry 文档 + 入库前归一化 pass（2026-07-04）

> **状态：搁置（2026-07-05 架构重估，未实施）**
>
> 重估结论：本系统是纯向量图谱——BFS 多跳按**实体名**精确 join（谓词不参与遍历，
> `relation_hints` 为死字段），关系召回与 rerank 走 relation_text 的**向量语义**，
> agent 阅读关系也无表面形式要求。谓词变体的实际代价只有三项：
> (1) 同义冗余边挤占 top-k / 扇出名额；(2) 跨语言关系句的向量折损；
> (3) 谓词分布的治理观测。均非检索质量瓶颈，不值得本方案
> registry 文件契约 + LLM 批量归一 pass + 写回/晋升机制的复杂度。
>
> **保留落地**（零成本部分）：
> - 抽取提示词卫生规则：谓词遵循文档原语言、动词原形、禁模糊谓词
>   （`prompts/pipeline/triplet-extraction.system.md`）；
> - parse 时内置静态映射表归一化 + `original_predicate` 元数据
>   （已实现：`bins/worker/src/pipeline/predicate_normalize.rs`）。
>
> **搁置部分**：§1 registry 文件、§3 归一化 pass（L2 LLM 批归一）、§4 写回与晋升、
> §5 env、§6 改动清单中除提示词外的条目。若未来迁移到符号图谱查询（按边类型过滤）
> 或谓词冗余实测成为瓶颈，再启用本方案。
>
> 高杠杆方向改投：**实体对齐**（join 与跨文档连通的真正瓶颈）与**链式关系完整抽取**。

以下为原方案全文，留档备查。

---

取代上级文档 `2026-07-04-vector-graph-rag-upgrade.md` §1.2「提示词闭集」与 §1.3「代码层归一化」
的实现方式（验收目标 §1.4 不变）。本文是实施依据，所有机制细节以本文为准。

## 0. 决策记录（讨论定案）

| 决策 | 结论 | 理由 |
|------|------|------|
| 谓词语言（抽取阶段） | **遵循 chunk 原语言**，不强制中文也不强制英文 | 抽取忠实、免翻译噪声；「英文更归一」被基线证伪（adr-0004 全英文谓词照样爆炸出 20+ 变体），归一性来自闭集约束而非语言 |
| 谓词语言（入库形态） | 归一化后统一到 **canonical 规范形式**，当前规范词为中文（语料主语言） | relation_text 参与向量化，与中文查询对齐；规范词语言是 registry 文件内容，未来切英文不改代码 |
| 闭集位置 | **不注入抽取提示词**，外置为 registry JSON 文档 | 冷启动无闭集可注入；闭集更新会作废抽取 completion cache（prompt hash 机制）；注入还导致批内规则漂移 |
| 归一化时机 | **后置**：三元组合并去重后、建图入库前，跑一次归一化 pass | 全局视角（看全批 distinct 谓词）优于抽取时逐条局部判断 |
| 增量更新 | LLM 判定的新谓词写入 registry **候选区**，达频次阈值自动晋升 canonical | 直接写回闭集正文会让闭集膨胀回开放集 |
| 批内一致性 | pass 开始读 registry 快照，结束时一次性写回 | 避免边抽边改导致同批文档规则不一致 |

## 1. Registry 文件契约

### 1.1 路径与开关

- Env：`INGESTION_PREDICATE_REGISTRY_PATH`（绝对路径或相对 worker CWD）。
- **未设置（默认）**：不读写任何文件，归一化退化为内置 seed 静态表匹配（行为≈现状，安全默认）。
- 已设置但文件不存在（冷启动）：首次 pass 用内置 seed 表生成初始文件后继续。

### 1.2 JSON Schema

```json
{
  "version": 1,
  "updated_at": "2026-07-04T15:00:00Z",
  "canonical": [
    {
      "id": "属于",
      "aliases": ["隶属于", "归属于", "part of", "belongs to"],
      "count": 42
    }
  ],
  "candidates": [
    { "id": "评审", "count": 2, "first_seen": "2026-07-04T15:00:00Z" }
  ]
}
```

字段约定：

| 字段 | 约定 |
|------|------|
| `canonical[].id` | 规范谓词表面形式（即入库形态）。语言无约束，当前 seed 为中文 |
| `canonical[].aliases` | 变体表。匹配规则：`trim` + `to_lowercase` 后精确比较（中文不受影响） |
| `canonical[].count` | 累计命中次数（含别名命中），每次 pass 写回时累加 |
| `candidates[].id` | LLM 判定为 new 或 LLM 不可用时的原谓词 |
| `candidates[].count` | 累计出现次数，跨 pass 累加 |
| `candidates[].first_seen` | 首次出现时间（审计用） |

### 1.3 Seed（内置初始表）

现 `predicate_normalize.rs` 的 `PREDICATE_SYNONYMS` 静态表整体转换为 canonical 初始内容
（目标词全集约 30 个规范谓词 + 各自 aliases）。静态表在代码中保留，用途仅两个：
默认模式（无 registry path）的匹配源、冷启动生成初始文件的模板。
Registry 文件一旦存在，**文件是唯一事实来源**，代码不再合并静态表。

### 1.4 写入与并发

- **原子写**：写 `<path>.tmp` 后 `rename` 覆盖。
- **进程内并发**（同 worker 多文档同时灌库）：registry 挂 `PgTaskProcessor`，
  `Arc<tokio::sync::Mutex<...>>`；pass 读快照和写回各持锁一次，pass 中间不持锁。
- **跨进程**（多 worker 实例）：本版不处理（当前单 worker 部署）。若将来多实例，
  迁移到 PG 表 + `SELECT ... FOR UPDATE`，schema 不变平移。

## 2. 抽取提示词变更（`prompts/pipeline/triplet-extraction.system.md`）

**删除**：

- 「Predicate rules (closed-set + normalization)」节中的 14 词闭集清单；
- 「predicates are always Chinese」强制中文规则。

**保留 / 改写为卫生规则（语言中立）**：

1. 谓词 = 动词或动词短语，用**原形**，禁时态/语态/名词化变体（`implements`→`implement`，
   「被使用」→「使用」除非被动语义本身是关系方向的一部分，如「被替代为」）。
2. **语言遵循 chunk 正文主要语言**：中文 chunk 出中文谓词（2–8 字），英文 chunk 出英文谓词
   （1–4 词，全小写）。
3. 禁模糊谓词：涉及 / 相关 / 有关 / relates to / is related to / involves。
4. G1（`标识为` 表格门控）、G3（长度）、G4（grounding）不变。
5. 实体命名规则不变（标识符照抄原文、禁「结构体/机制」后缀、同名统一）。
6. 链式关系强制节不变，例句改为双语各一（废弃/替代链、实现/定义于链）。

**影响**：本次修改使 `TRIPLET_PROMPT_VERSION_HASH` 变化一次（旧抽取缓存作废，预期行为）；
此后 registry 演化**不再触碰提示词**，抽取缓存长期稳定。

## 3. 归一化 pass

### 3.1 插入点

`document_pipeline.rs` 图索引分支内（当前 L647–681）：

```text
extract_triplets_for_index            (文本三元组)
extract_visual_triplets_for_index     (视觉三元组，可选)
merge_extracted_triplets              (合并去重)
──► normalize_triplet_predicates      (本设计新增)
──► merge_extracted_triplets(normalized, [])   (归一化可能造成新重复，二次去重)
build_graph_index_records             (embed + 入库记录)
```

签名：`async fn normalize_triplet_predicates(processor, triplets, parse_run_state) -> Vec<ExtractedTriplet>`，
放在改造后的 `predicate_normalize.rs`。

### 3.2 三层流程

| 层 | 动作 | 成本 |
|----|------|------|
| L1 静态/registry 匹配 | 谓词 `trim+lowercase` 后查 canonical id 与 aliases；命中即替换 | 零 |
| L2 LLM 批量归一 | L1 未命中的 **distinct** 谓词打包成一次调用（见 3.3） | 每文档 ≤1 次小调用 |
| L3 兜底 | LLM 判 new / 调用失败 / 开关关闭 → 保留原谓词，记入 candidates | 零 |

替换发生时填 `ExtractedTriplet.original_predicate`（已实现的字段与
`graph_index.rs` metadata 透传机制沿用）。

### 3.3 LLM 调用契约

- 复用 `processor.triplet_llm`；env `INGESTION_PREDICATE_NORMALIZE_LLM`（默认 `1`；
  `0` 时只走 L1/L3）。`triplet_llm` 未配置时自动跳到 L3。
- **输入**（user message，单行 JSON）：

```json
{
  "unmapped": ["extends with", "superseded by"],
  "canonical": ["属于", "实现", "被替代为", "…"]
}
```

  参照集 = canonical 全量 + candidates 中 `count ≥ 2` 者，按 count 降序，**上限 50 个**
  （防 prompt 膨胀）。
- **输出**（单行 JSON）：

```json
{ "mappings": { "extends with": "扩展", "superseded by": "被替代为" } }
```

- **合法性校验（防幻觉）**：映射目标必须 ∈（参照集 ∪ 本次 unmapped 集合）。
  目标在 unmapped 内 = **批内自聚类**（冷启动时 canonical 为空，允许 LLM 把
  `implements` 归并到同批的 `实现`）；目标在两个集合之外 → 该条映射丢弃，原词按 new 处理。
- 映射不必覆盖全部 unmapped；未出现在 mappings 里的词一律按 new 处理。
- 参数：temperature 0.1，max_tokens 2048。
- **失败降级**：任何错误（超时/解析失败）→ `tracing::warn` + `record_graph_degrade`
  记录原因，全部 unmapped 按 L3 处理，**不阻塞入库**。
- 该调用不经过 completion cache（输入含动态 registry 状态，命中率无意义）。

### 3.4 冷启动行为

registry 为空或极小时不跳过 L2：LLM 依然对本批 distinct 谓词做批内自聚类 + 卫生归并，
聚类目标词成为首批 candidates。闭集从第一批语料自然长出，无需人工预置。

## 4. 写回与晋升

pass 结束持锁一次性执行：

1. canonical 命中（L1 或 L2 映射到 canonical）：对应 `count` 累加。
2. L2 新映射：alias 追加到目标 canonical 的 `aliases`（去重）；下批起 L1 直接命中。
3. 批内自聚类目标词与 new 词：进 `candidates`，`count` 累加（已存在则 +n）。
4. **晋升**：写回时检查 `candidates.count ≥ INGESTION_PREDICATE_PROMOTE_THRESHOLD`
   （默认 `3`）→ 移入 canonical（初始 aliases 为空）。
5. `updated_at` 刷新，原子写盘。

无 registry path 时本节整体跳过（无写回、无晋升，candidates 不持久化）。

## 5. Env 清单

| 变量 | 默认 | 说明 |
|------|------|------|
| `INGESTION_PREDICATE_REGISTRY_PATH` | 空（禁用文件） | registry JSON 路径；空 = 仅内置 seed 表 L1 匹配 |
| `INGESTION_PREDICATE_NORMALIZE_LLM` | `1` | L2 LLM 批量归一开关 |
| `INGESTION_PREDICATE_PROMOTE_THRESHOLD` | `3` | 候选晋升 canonical 的累计频次阈值 |

`.env.example` 增补注释；测试环境 `.env` 配置
`INGESTION_PREDICATE_REGISTRY_PATH=./data/predicate_registry.json`。

## 6. 代码改动清单

| 文件 | 改动 |
|------|------|
| `prompts/pipeline/triplet-extraction.system.md` | §2：删闭集清单与强制中文，改卫生规则 + 原语言规则，链式例句双语化 |
| `bins/worker/src/pipeline/predicate_normalize.rs` | 改造：seed 表保留 + `PredicateRegistry`（load/save/match/promote）+ `normalize_triplet_predicates` pass（L1/L2/L3）|
| `bins/worker/src/pipeline/triplet_extraction.rs` | **回滚** `parse_triplet_item` 内的逐条 `normalize_predicate` 调用（parse 输出原始谓词；归一化统一在 pass 做）|
| `bins/worker/src/pipeline/document_pipeline.rs` | 插入 pass 调用 + 二次去重（§3.1）|
| `bins/worker/src/pipeline/processor.rs` | `PgTaskProcessor` 增 `predicate_registry: Option<Arc<Mutex<...>>>` 字段与初始化 |
| `bins/worker/src/main_tests.rs` | `parse_triplet_response_normalizes_predicate_synonyms` 改为针对 pass 的测试 |
| `.env.example` | §5 三个 env |
| `docs/plans/2026-07-04-vector-graph-rag-upgrade.md` | §1.2/1.3 加一行「实现方式改由本文档定义」指向 |

不变：`graph_index.rs` 的 `original_predicate` metadata、`ExtractedTriplet.original_predicate`
字段、`TRIPLET_PROMPT_VERSION_HASH` 缓存机制。

## 7. 测试与验收

单测（无网络、stub LLM）：

1. registry：冷启动生成初始文件；alias 大小写匹配；count 累加；晋升阈值；原子写回（tmp+rename）。
2. pass：L1 命中替换且填 original_predicate；L2 输出合法性校验（幻觉目标丢弃、
   批内自聚类目标接受）；LLM 失败降级到 L3 且 pass 不报错；归一化后二次去重
   （`implements`/`实现` 两条合并为一条，supporting_chunk_ids 并集）。
3. 无 registry path 模式 = 仅 seed 匹配，不写文件。

验收（重灌后，指标继承上级文档 §1.4）：唯一谓词 <60、长尾占比 <50%、
adr-0004 替代链边存在。**重灌是显式一次性操作，不自动执行。**

## 8. 非目标（本版不做）

- 跨进程锁 / PG 存储（单 worker 部署下 YAGNI，schema 已预留平移路径）
- 嵌入聚类式谓词审计（规则 + LLM 批归一在当前语料规模够用）
- 历史已入库关系的回填（由重灌覆盖）
- 实体名的归一化（本设计只管谓词；实体照抄原文规则已在提示词层）
