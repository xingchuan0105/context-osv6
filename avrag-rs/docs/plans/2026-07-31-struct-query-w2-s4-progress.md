# struct-query W2（S4 ingestion 挂接）首窗口交接（2026-07-31）

> **后续**：第二窗口已完成 markitdown 换血 + S4 挂接，见 `2026-07-31-struct-query-w2-s4-window2-handoff.md`。
> 上游：`docs/plans/2026-07-31-struct-query-post-p2-dev-plan.md` W2。
> 硬前置核对结论：**生产 parser 未走 markitdown**（worker `bins/worker/src/pipeline/document_pipeline/` 走自有 parse 阶段；markitdown 仅在 E2E 换血 harness `markitdown_reingest.rs`）→ 按计划条款做不依赖前置的 ②③ + 提取器 Rust 化。

## 0. 一句话

S4 的 Rust 侧拼图本窗口全部就位：markdown 表格提取器 Rust 化（三语料行级 parity 全对）+ 证据入库 storage-pg 路径（真 PG 验证）+ `SuperviseInput::from_markdown` 库化入口——剩余仅「表格阶段挂进 document_pipeline」，仍等 markitdown 前置。

## 1. 本窗口产出

| 内容 | 文件 | 验证 |
|---|---|---|
| 提取器 Rust 化：`extract_grids` / `merge_continuations` / `auto_rotate` / `prepare`（`pipeline.py` 移植） | `crates/struct-supervision/src/extract.rs`（新，含 10 单测） | **三语料 parity：ipd 1 表 / 白药 9 表 / 万科 306 表，行级 cells + notes 全等**（对拍工具 `examples/dump_grids.rs` vs `pipeline.py --emit-grids`） |
| 库化入口：`SuperviseInput::from_markdown(doc_id, text)`（ingestion 直接库调用，无需 Python/JSON 中转） | `crates/struct-supervision/src/lib.rs` | 随 extract 测试 |
| 证据入库 Rust 化：`replace_table_evidence_chunks`（幂等先删后插；owner 取自 documents 行；`TableEvidenceChunkRow` 入参） | `crates/storage-pg/src/lib_impl/repository_assets.rs` | 真 PG 测试 `tests/table_evidence.rs`：装载 2→水合可见→重载替换 1→无文档跳过，全过 |

## 2. markdown-it 语义对齐记录（探针实证，提取器的行为依据）

手扫行扫描器（非完整 CommonMark parser），以下语义均经 markdown-it-py 实测钉死：

- 表头行须含 `|`；分隔行格 `:?-+:?`（≥1 个 `-`，单 `-` 亦可）且列数 == 表头列数，否则非表。
- 表体吸收一切非空行（**含无 `|` 的段落行**）；ragged：多出截断、不足补空串到表头列数。
- 单元格 trim，`\|` → `|`；`**`/`` ` `` 等行内标记保留原文。
- 围栏代码（``` / ~~~）与 ≥4 空格缩进代码内的管道行不成表。
- **表体终止**：新块起始行（ATX 标题 / 块引用 / 列表项 / hr）中断表体（万科 t140 实证：`- 转回第二阶段` 列表行）。
- **列表上下文**：非空列表项吞并后续非空行（lazy continuation，万科 7949-7958 实证：20 个伪表被正确抑制）；**空标记项**（孤 `-`，万科 13324 实证）只终止表体、不开启列表上下文。

## 3. 验证 gate

```bash
CARGO_BUILD_JOBS=2 cargo test -p avrag-struct-supervision   # 41（28 lib + 13 bin）全绿
CARGO_BUILD_JOBS=2 cargo test -p avrag-rag-core --lib struct_query  # 12/12（未动，回归）
DATABASE_URL=<.env> cargo test -p avrag-storage-pg --lib table_evidence  # 1/1 真 PG
```

- **storage-pg 全量 lib**：5 个预存失败（cleanup_delete_soft / ingestion×3 / document_ir），**干净树同样失败**（已 stash 双盲验证），与本窗口无关——开发库 `avrag_rs` 数据/RLS 状态所致，归 storage-pg 线另查。

## 4. W2 剩余（下一窗口，仍等前置）

1. **① 表格阶段入 `bins/worker/src/pipeline/document_pipeline/`**：stage 序列 parse_validate → project_ir → **【表格阶段挂点】** → materialize_chunks → retrieval_index。消费侧备齐：md 文本 → `SuperviseInput::from_markdown` → `runner::supervise`（LLM 配置复用 `INGESTION_LLM_*`）→ `store::write_duckdb`（落 struct_store，随 doc 生命周期：删 doc 删文件、doc_version 变更重建）→ `replace_table_evidence_chunks` 入库。
2. **前置**：生产 parser markitdown 化（另案；届时表格阶段直接消费 markitdown md）。前置未到位时可选过渡：worker 对 office 类原件子进程调 markitdown（与 E2E harness 同法）——是否过渡由用户拍板。
3. gate：生产 ingestion 灌 ipd xlsx → struct_catalog 可见 + 证据水合通 + 删 doc 文件随删。

## 5. 操作要点

- parity 对拍：`cargo run -p avrag-struct-supervision --example dump_grids <md>` vs `pipeline.py <md> --emit-grids -`；改提取器后必跑三语料。
- `replace_table_evidence_chunks` 由 ingestion 事务内调用（RLS 经 `pool.begin(context)` 自动 set_config，与邻域方法同款）。
- 脏树纪律：storage-pg 全量 lib 的 5 个预存失败不要误伤归因；commit 只挑本窗口文件。
