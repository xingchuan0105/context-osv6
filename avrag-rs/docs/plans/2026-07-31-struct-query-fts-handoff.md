# struct-query fts 表内值发现交接（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-p2-handoff.md`（P2 四项队列）；本文档 = fts 项落地后的交接：改了什么、怎么验的、残留、下一项。

## 0. 一句话现状

fts 表内值发现（P2 第 4 项）已收官：灌入侧（struct-supervision Rust + pipeline.py）每表建 FTS 索引，查询侧（struct_query）标注 catalog 可用性并放行 `match_bm25` 谓词——全部在加固只读连接内完成，12/12 struct_query + 31 struct-supervision 全绿。P2 仅剩数值规整。

## 1. 本窗口产出（1 commit）

| commit | 内容 | 关键文件 |
|---|---|---|
| `6940edd1` | fts 表内值发现 | `struct_query.rs`（open_readonly LOAD fts + catalog fts 标注 + 过滤 fts 内部表）、`struct-supervision/src/store.rs`（write_duckdb 建索引）、`pipeline.py`（INSTALL+LOAD fts + 建索引）、`examples/fts_probe.rs`（技术验证） |

三个已确认的决策：
- **D1** 查询侧先 LOAD fts 再 SET 加固——bundled duckdb 内建 fts 扩展（`create_fts_index`/`match_bm25` 宏是 core），LOAD 需扩展目录访问（离线缓存 `~/.duckdb/extensions/`）；先 LOAD 后锁，查询期文件访问仍全禁（read_csv 被拦已实测）。
- **D2** catalog 过滤 fts 内部表——`create_fts_index` 在 per-doc 库产出 `dict/docs/fields/stats/terms/stopwords` 内部表（schema `fts_main_<table>`），用户表查询（`information_schema.tables`）必须排除，否则 catalog 出现 6 张假表。
- **D3** 中文不分词——DuckDB fts tokenize 是空格分词（`tokenize('概念阶段')` = 单 token），`match_bm25('概念')` 对整串中文表零命中；fts 与 grep 互补（grep 管子串、fts 管空格分隔 token），计划中「配合 chunk grep」的意图即此。

## 2. 验证证据

- **单测**：`cargo test -p avrag-rag-core --lib struct_query` **12/12 全绿**（新增 `fts_predicate_works_on_indexed_store`：fixture 建索引 → catalog `fts=true` → `match_bm25` 命中 1 行 / 无命中空结果；老 fixture 无索引 → catalog `fts=false`）。
- **struct-supervision**：`cargo test -p avrag-struct-supervision` **31 全绿**（write_duckdb 测试断言 FTS 索引已建 + `match_bm25` 命中）。
- **真实语料**：pipeline.py 灌 ipd → `match_bm25('概念阶段')` 命中 370 行表。
- **技术验证**：`examples/fts_probe.rs`（bundled 内建 fts 无需 LOAD；只读+加固连接先 LOAD 后锁，`match_bm25` 可用、`read_csv` 被拦）。

## 3. 残留与观测点

1. **fts 谓词从未被真实 LLM 触发过**——`validate_sql` 放行 `match_bm25`（FROM 只有用户表，FTS 谓词在 WHERE），但切片（86/88/106）尚未跑过带 FTS 关键词的题；下次切片观察 `tool_trace` 是否出现 match_bm25 调用。
2. **中文表 fts 命中率为 0 是物理现实**（D3）——白药/ipd 均为整串中文表，fts 索引对它们无效；对英文/混合表（如万科年报）才有效。若需中文 fts，得换 jieba 分词（duckdb 无 jieba 扩展）或索引时预切（改灌入侧，不在本项范围）。
3. **老 per-doc 库无 FTS 索引**（P1d 灌的 ipd/白药）——catalog `fts=false`，查询侧 `match_bm25` 会报 schema 不存在（模型可见错误，有容错）；重灌后自动获得索引。

## 4. 操作要点

- **fts 查询语法**（供 skill/模型参考）：`SELECT * FROM <table> WHERE fts_main_<table>.match_bm25(row_ord, '关键词') IS NOT NULL`——标量宏、每行调用、无匹配返回 NULL；`fields := '列名'` 可限定列；`conjunctive := true` 多词全中。
- **catalog fts 标注**：relation JSON 新增 `fts: bool` 字段（schema `fts_main_<table>` 存在即 true）。
- **验证命令**：
  ```bash
  cd /home/chuan/context-osv6/avrag-rs
  CARGO_BUILD_JOBS=2 cargo test -p avrag-rag-core --lib struct_query
  CARGO_BUILD_JOBS=2 cargo test -p avrag-struct-supervision
  # 真实语料验证（需 /tmp/markitdown_out 语料）：
  cd scripts/struct_query_poc
  /tmp/struct_poc/bin/python3 pipeline.py /tmp/markitdown_out/huawei_ipd_370_activities.xlsx.md --out /tmp/poc_ipd_fts.duckdb
  /tmp/struct_poc/bin/python3 -c "import duckdb; con=duckdb.connect('/tmp/poc_ipd_fts.duckdb',read_only=True); con.execute('LOAD fts'); print(con.execute(\"SELECT row_ord FROM t0 WHERE fts_main_t0.match_bm25(row_ord,'概念阶段') IS NOT NULL LIMIT 3\").fetchall())"
  ```

## 5. 下一步

P2 仅剩**数值规整**（finetype 扩展：货币/日期语义类型 / TRY_CAST）——计划 §6.1 标注 v1.1 可选；若做，与 fts 同款模式（灌入侧规整列 + 查询侧只读消费），需评估 finetype 扩展是否 bundled 内建（fts 已证内建，finetype 待探针验证）或跳过（v1 范围已足）。
