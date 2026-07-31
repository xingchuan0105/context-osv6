# struct-query W4/W6 交接：双栏检测 + 行级证据（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-post-p2-dev-plan.md`（W1✅ W2✅ W3✅）。
> 执行方式：主线 + 3 个 subagent（secondary）：T1=W4 检测、T2a=W6a 行号埋点、T2b=W6b 映射+查询侧。

## 0. 一句话

W4/W6 全收官：双栏检测信号入库（万科 t114 真实签名命中、ipd/白药零误报）、行级证据全链通（chunk md 行号埋点 → `_line_map` 区间映射 → 明细 evidence 候选集合 `chunk_ids`）；验收中抓出并修复**窗口 2 的 PDF 摄入全灭 bug**（校验器页元数据规则未豁免 markitdown 后端）与 product_e2e 编译残留。

## 1. W4 双栏/跨页检测（T1）

- **规格纠偏（实证）**：字面「非空列集合两个互不相交列组」在 t114 不成立（列共现图 1 个连通分量）且误报白药 t2。真实机制：双栏两面板共用列布局被 `merge_continuations` 竖向拼接，表体留下兄弟面板**近重复表头行**（6084/6135/6160）。
- 落地 check（Rust `checks.rs` + Python `pipeline.py` 同语义）：
  - `dual_column_suspect`（passed=false → needs_diagnosis）：信号1 列组分离（加各 ≥2 列防误报，本语料零触发，休眠防御）+ 信号2 面板表头行混入（t114 命中）。
  - `section_header_rows`（passed 恒 true 仅记录）：孤立段标行计数（t114 记 12 行；确定性口径不区分段标与稀疏行，裁决交 supervision）。
- 证据：`cargo test -p avrag-struct-supervision` 36+13 全绿；`check_pipeline.py` 17 断言全过；万科全库 5 表触发（t64/t88/t93/t114/t268，均属 302 既有 needs_diagnosis）；Rust↔Python dry-run 逐值一致；三语料 parity 复跑 306/306 行级+notes 全对。
- merge_continuations 只补回归测试未改规则。

## 2. W6 行级证据（T2a+T2b+主线修正）

链路（全部已验）：

```text
markitdown.rs blocks_from_markdown：block 记 md 行区间（metadata md_line_start/end，0-based 闭区间，常量 ir.rs:373）
  → chunker.rs：chunk 聚合 min/max（并入加宽；成员缺键则整键降级）
  → PG chunks.metadata->'block_metadata'（整 map 透传，无白名单）
  → worker stage_struct_line_map（post-materialize，best-effort）：_line_map(md_line_start, md_line_end, chunk_id) 幂等重建入 per-doc duckdb
  → struct_query 明细 evidence：__src_line → 区间包含的全部候选 chunk_ids（granularity="row"）；
    无包含就近 floor/ceil（"row_nearest"，如实区分）；老库无 _line_map → 表级降级（"table"）
```

- **主线修正 1（诚实粒度）**：T2b 初版是「下端点 floor 单值」——一个 block 拆多 chunk 共享区间时（ipd 巨表 [1,373]×127 chunk）会指向不含该行的 chunk。改为区间 schema + `chunk_ids` 候选集合（附 `chunk_id` 首候选兼容字段）。查询侧另把 `_line_map` 加入 catalog 排除清单（防泄成用户 relation）。
- **粗粒度声明**：行级证据的诚实粒度是「候选 chunk 集合」（块级区间）；精确到行 → 需在拆分处记 segment 偏移（另案，非必要——召回/引用场景候选集合已可用）。
- 测试：ingestion 85/85、struct_query 14/14、worker 31/31、storage-pg 真 PG 2 例全绿。

## 3. 验收中抓出并修复的既有 bug（均入本窗口 commit）

| bug | 现象 | 修复 |
|---|---|---|
| **PDF 摄入全灭**（窗口 2 引入） | `ir_validator` 要求 PDF 块带页元数据，markitdown 后端无页概念 → 校验失败 → 任务反复重试至死信（白药 PDF 实测四轮 300s 超时循环） | 校验规则豁免 `ParseBackend::Markitdown`（`ir_validator.rs`），含两侧单测（豁免生效 + 非 markitdown 仍拦截） |
| product_e2e 编译残留（窗口 2 引入） | `builder.rs:820` 初始化已删除的 `mock_office_abort` 字段 → `cargo test -p app --test product_e2e` 挂 | 删该行（一行） |
| runner.rs 漏提交（本线窗口 2 漏 add） | 干净检出 b0dc1722 worker 编译失败 | commit `4717c976` 补 |

## 4. 生产路径验收（真 api+worker，四环 + PDF 附加环）

| 环 | 结果 |
|---|---|
| ipd xlsx 全链 | struct stage（supervision 真 LLM）→ duckdb 370 行 → PG body 128/128 带行号键 → `_line_map` 区间版 128 行 → 行 0/200/369 各解析 127 候选（巨表块粗粒度如实） |
| 白药 PDF（修复后） | parse_validate 3s 过（修复前无限重试）→ 全链完成（fixture md 无管道表 → grids 空 → 无 duckdb，符合预期；body=17 + embedding 17 vectors） |
| 删 doc 随删 | duckdb + sidecar + PG chunks 全清（复验） |
| 切片回归 | 86 PASS 1.0（与 W1 基线一致）；106 两轮 0.6/0.5（SELECTION_MISS/RETRIEVAL_MISS）——机理分析：本轮评估库无 `_line_map` 走老库降级路径（W6 未参与），属 106 慢性抖动非回归；smoke 库实证 body=128 与 table_evidence=1 共存（W2 修复在 harness 路径同样生效） |

## 5. 残留（另案/另线）

1. `total_reconcile` Rust/Python 求和口径漂移（T1 发现：Py 排除全部合计行 vs Rs 只排首个；t114 上 detail 数值不同）——另案对齐。
2. markitdown 拆分块粗粒度：精确行级定位需 segment 偏移埋点（另案，非必要）。
3. 106 慢性抖动（白药 638 子主张）：跨窗口观测项，不归 W6。
4. worker 对同一 doc 重复处理两遍（每 doc stages ×2，历史上即如此；本窗口验收反复因此抢到中间态）——worker 队列行为另查。
5. `cargo check -p app --tests` 仍被另一条线阻塞（delegate_contract.rs）；storage-pg 全量 5 个预存失败；staging e2e 三测试与 THIRD_PARTY_NOTICES 包名（W2 残留清单照旧）。
6. `source .env` 不设 `set -a` 时真 PG 测试静默 skip（T2b 发现）——跑 storage-pg 测试统一 `set -a && source .env && set +a`。

## 6. 操作要点

- 行级证据字段语义：evidence 行 `chunk_ids`（候选全集）+ `chunk_id`（首候选/就近/表级）+ `chunk_granularity`（row / row_nearest / table）；`scanned_chunks`/`chunks`（召回抽取）维持表级。
- `_line_map` 在 per-doc duckdb 内（随 doc 生命周期删除），幂等重建；查询侧对老库自动降级。
- 本地验收流程同 W2 收尾窗口（register → workspace → documents → /dev-upload → poll → 验 duckdb/PG → DELETE）；cleanup 轮询有 ~100s 延迟。
- 改提取器后必跑三语料 parity（`dump_grids` vs `--emit-grids`）。
