# struct-query 后 P2 开发计划（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-finetype-handoff.md`（数值规整关闭，P2 四项全收官）。
> 本文档 = 剩余全部工作的窗口编排。每个窗口：目标 / 范围与非目标 / 代码落点 / 验收 gate / 依赖。
> 执行纪律沿用主计划 §14 + SOLO_DISCIPLINE：prompts 只落 `prompts/**` 第三人称、脏树只挑自己 hunk、`CARGO_BUILD_JOBS=2`、LLM 题两轮再定论、结构性改动后 `graphify update .`。

## 0. 现状基线（已收官，勿重做）

- P0/P1 全链：markitdown → markdown-it-py 提取 → 校验套件 → supervision loop（Rust `avrag-struct-supervision`，31 测试）→ per-doc DuckDB（fts 索引）→ host 加固只读 struct_catalog/struct_query（12 测试）。
- 证据：表级 evidence chunk（sidecar → PG `chunks[chunk_type=table_evidence]`，仅水合不进检索面）。
- 数值规整：**关闭**（finetype 探针否决 + 用户拍板；重启只能走 supervision 指令扩展，禁灌入侧硬编码启发式）。
- 语料现状：ipd（370 行 high_candidate，fts 索引）、白药（9 布局网格 needs_diagnosis，fts 索引）在 `storage/struct_store/`；万科 2024 年报仅 /tmp 验证（306 表、五条勾稽恒等式对平到分）。

## 1. 窗口总览与依赖

```text
W1 观察（无代码）          ── 先行；产出 W5 的启动证据 + telemetry 数据
W2 S4 ingestion 挂接       ── 产品化主线；硬前置：生产 parser markitdown 化（另案）
W3 A5 补测                 ── 独立，小；任何空档可插
W4 提取鲁棒性（双栏/跨页）  ── 独立；改动 pipeline 提取层，建议与 W2 串行（同文件面）
W5 中文 fts                ── 默认关闭；仅 W1 证据证明中文值发现是真实短板才启动
W6 行级证据                ── 依赖 chunker 行号埋点；现表级证据够用，最后
```

---

## W1 观察窗口（先行，无代码改动）

**目标**：fts 残留 #1（`match_bm25` 谓词从未被真实 LLM 触发）+ telemetry 真实数据积累。

**范围**：
- 切片题中加入能触发表内值发现的问法（如「提到 X 的活动有哪些」类），跑既有切片脚本观察 `tool_trace` 是否出现 `match_bm25`；SKILL.md fts 语法条已补（commit f6f0801a），触发条件已改善。
- telemetry 延续：`synthesis_code_answer_repair`/`violation` 真实触发率（首轮三题全 0）；loop 预算 28K/12 轮的 `budget_exhausted` 分布——**积累数据，不凭单轮感觉调**。
- Q88 SELECTION_MISS 类既有抖动继续记录（非终答问题，不归本线）。

**非目标**：不改 loop 预算、不改 skill、不改 harness。

**落点/命令**：
```bash
cd /home/chuan/context-osv6
CARGO_BUILD_JOBS=2 STRUCT_STORE_DIR=$PWD/avrag-rs/storage/struct_store \
  QUESTIONS=86,88,106 bash avrag-rs/scripts/sac-skill-fail6-reg.sh
# artifact：v2_*/q0NN.json 的 mode_debug.general["activity_counts"] + tool_trace
```

**gate**：≥2 轮切片（LLM 抖动纪律）；每题记录 tool_trace 有无 match_bm25、activity_counts、budget_exhausted 次数；结论写入当日交接。
**状态**：✅ 已完成（2026-07-31，86/106/113 两轮）——`docs/plans/2026-07-31-struct-query-w1-observation.md`：repair 首触发且有效、match_bm25 零触发（fts 残留 #1 维持）、W5 维持关闭。

---

## W2 S4 ingestion 挂接（产品化主线，最大项）

**目标**：P2 文档 S4 ◐ → ✅——pipeline 从 PoC 脚本挂入生产 ingestion 管线。

**硬前置**：生产 parser 统一走 markitdown（主计划前置，另案推进）——表格阶段消费 markitdown md；前置未就位时本窗口只做不依赖它的部分（②③）。

**范围**：
1. **表格阶段入管线**：ingestion 处理链中（parse 后、chunk 前后）挂表格提取——grids 提取（Rust 已有）→ supervision loop → per-doc `<doc_id>.duckdb` 落 struct_store（随 doc 生命周期：删 doc 删文件，doc_version 变更重建）。
2. **证据入库 Rust 化**：`load_evidence_chunks.py`（Python，幂等先删后插 + RLS `app.current_user`）的逻辑迁入 storage-pg 路径，`table_evidence` 插入随灌入事务完成。
3. **struct-supervision 库化**：当前 CLI（`src/bin/struct-supervise.rs`）形态 → ingestion 内直接库调用（grids 提取/supervise/write_duckdb 已是 lib API）。
4. supervision LLM 配置复用 `INGESTION_LLM_*`（主计划 §4.6 既定）。

**非目标**：不改查询侧 struct_query；不动 chunker 主链（W6 才动）；不做行级证据。

**代码落点**：
- `crates/ingestion/src/`（`runtime.rs` TaskProcessor 实现侧挂表格阶段；处理面在 `chunker.rs`/`parser/` 邻域）
- `crates/struct-supervision/src/lib.rs`（库化 API 确认/补全）
- `crates/storage-pg/src/lib_impl/`（table_evidence 插入；`get_chunks_by_ids` 已支持，2b）
- 配置：struct_store 目录随部署（现为 `STRUCT_STORE_DIR` env，默认 `storage/struct_store`）

**状态**：✅ 已完成（2026-07-31 三窗口）——首窗口 ②③ + 提取器 Rust 化；第二窗口 markitdown 唯一解析器 + 表格阶段挂接（commit b0dc1722）；收尾窗口 deploy 依赖/jvm 删除/A5/**本地验收门四环全过**（含真 bug 修复：`store_document_body_chunks` 不再擦除 table_evidence）。详见 `docs/plans/2026-07-31-struct-query-w2-s4-final-handoff.md`。

**gate**：
```bash
CARGO_BUILD_JOBS=2 cargo test -p ingestion -p avrag-struct-supervision -p storage-pg
```
E2E：经生产 ingestion 灌一篇含表 doc（ipd xlsx）→ `struct_catalog` 可见该 doc 关系（fts=true）→ 明细查询 evidence chunk_id 水合通 → 删 doc 后 duckdb 文件随删。

---

## W3 A5 补测（独立，小）

**目标**：主计划 §11 A5 ⬜ → ✅——监督干预成功案例进回归。

**范围**：构造「表头被吃」样本（sheet 标题行成假表头，IPD 方言同款）→ supervision `rotate_header(header_row=1)` 指令 → 代码应用 → SQL 复验过 → 终态 high。纯确定性（不需真 LLM）：直接驱动 `apply_directive`。

**落点**：`crates/struct-supervision/src/`（directives/store 测试）或 `scripts/struct_query_poc/check_supervise.py` 增一例。

**gate**：`cargo test -p avrag-struct-supervision` 全绿含新例；断言 rotate 后表头正确 + 行数净化 + 复验全过。
**状态**：✅ 已完成（2026-07-31 随 W2 收尾窗口并入）——`store::tests::a5_eaten_header_rotate_header_sql_recheck`：IPD 方言假表头（sheet 标题 + Unnamed:N）→ `rotate_header(header_row=1, drop_columns_matching=^Unnamed)`（守卫两侧同验：全空 Unnamed 列丢、非空保留）→ checks 全过 high_candidate + SQL 复验（COUNT + 序号自校验）。

---

## W4 提取鲁棒性：双栏/跨页（独立）

**目标**：附录 C 已知限制——万科年报「流动资产区被双栏另一面板行混入」与 PDF 跨页/分栏提取质量。

**范围**：
- 双栏版式检测与面板行归属（markitdown 产物中左右栏行交错混入同一 grid）；先出**检测**（确定性信号入健康报告/notes），修复策略再议（可能交 supervision 裁决，符合「代码召回 + LLM 裁决」funnel 原则）。
- 跨页续表合并鲁棒性：表头签名聚类在 PDF 方言下的边界（白药/万科样本回归）。
- 改动面：pipeline 提取层（`extract_tables.py`/grids 提取、`merge_continuations`）+ struct-supervision Rust 对齐（S4 parity 纪律：两侧同形状）。

**非目标**：不重写提取器；不追求全版式覆盖（漏网边界诚实声明原则不变，主计划 §5）。

**gate**：万科 306 表重灌回归——t114 五条勾稽恒等式仍对平到分；双栏混入行数有检测计数（前后对比）；`check_pipeline.py` 既有 11 例不破。

---

## W5 中文 fts（默认关闭，观察驱动）

**条款**：仅当 **W1 证据**表明「中文表内值发现」是真实短板（模型频繁需要表内中文值定位且 grep 路径证伪）才启动；否则保持关闭——grep 已覆盖子串发现，与「不过度归整」steer 一致。

**若启动，选项**（D3 物理现实：duckdb fts 空格分词，整串中文单 token 零命中）：
- 灌入侧 jieba 预切影子 token 列（ft 索引建在该列）；jieba 不在 duckdb 扩展生态内 → 预切在 pipeline 侧做。
- 或评估其它 tokenizer 前置。**禁止**引社区扩展进查询侧（D4 教训：finetype 只读库 LOAD abort）。

**落点**：`pipeline.py` + `struct-supervision/src/store.rs`（索引列选择），查询侧不变。

---

## W6 行级证据映射（最后，依赖最重）

**目标**：附录 C 另案——行级 `__src_line → chunk_id`（现表级证据已使 Q86 recall 0→100%，本项是精度增强非功能补缺）。

**范围**：chunker 行号元数据埋点（切块时记录每 chunk 源行区间）→ 灌入侧算行级映射 → 查询侧 evidence 行级回填。

**依赖**：`crates/ingestion/src/chunker.rs` 改造（行号元数据）；影响面含既有语料重灌。

**gate**：行级 evidence 水合正确 + 检索/计数路径零污染回归（2b 已核的 `repository_retrieval.rs` 过滤口径不动）。

---

## 附：关闭项登记

| 项 | 状态 | 依据 |
|---|---|---|
| 数值规整（finetype/确定性规整列） | **关闭** | `2026-07-31-struct-query-finetype-handoff.md` §1–§2：探针否决 + 用户三条拍板；重启仅走 supervision 指令扩展 |
| W5 中文 fts | **默认关闭** | 本计划 W5 条款：W1 证据驱动 |
