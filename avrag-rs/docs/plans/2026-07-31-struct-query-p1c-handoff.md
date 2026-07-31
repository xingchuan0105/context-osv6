# struct_query P1c 交接文档（2026-07-31）

> 上游计划：`docs/plans/2026-07-31-struct-query-virtual-tables.md`（架构/决策/SDK 契约/验收矩阵以此为准）。
> 本文档 = P1c（Rust host 查询侧）的**完成状态 + 验证证据 + Review 遗留项 + 下一步操作手册**。

## 0. 一句话现状

P1c 功能代码全部落地、全部测试通过、**未提交**；自 review 发现的 4 个问题（R1–R4）**已全部修复并重跑验证通过**（2026-07-31 续作：`run_catalog`/`run_query` 闭包抽为 `catalog_store`/`query_store` 纯函数 + 补 5 个测试，struct_query 测试 10/10）。下一步进 P1d。

## 1. 已交付（本工作树，未 commit）

| 文件 | 内容 |
|------|------|
| `contracts/src/tool_call.rs` | 新增 `StructCatalogArgs{doc_ids}` / `StructQueryArgs{sql, doc_ids}`（`deny_unknown_fields`，与 DocGrepArgs 同款） |
| `contracts/src/lib.rs` | re-export 两个新类型（仅 2 行 diff，已避免 rustfmt 大面积重排） |
| `crates/rag-core/src/runtime/tools/struct_query.rs` | **新文件 642 行**。`run_catalog` / `run_query` + 内部函数 + 5 个测试 |
| `crates/rag-core/src/runtime/tools/mod.rs` | dispatch 注册两条 arm（+3 行 diff） |
| `crates/rag-core/src/runtime/bridge.rs` | `supported_method_names` +2；`method_to_tool_call` 两个新 arm（doc_scope 交叉与 grep 同款）；`tool_result_to_bridge_data` 全量透传 |
| `crates/code-interpreter/src/bridge.rs` | 沙箱 `_Client.struct_catalog/struct_query`（Python shim）+ `bridge_shim_client_method_names` + shim 测试断言（parity 测试要求 host ⊆ shim，两处必须同步改） |
| `crates/agent-loop/src/react_loop/sdk_gate.rs` | `RAG_PRIMITIVES` += `struct_catalog`, `struct_query`（不加会被沙箱 `capability_denied` 拒） |
| `crates/rag-core/Cargo.toml` | `duckdb = { version = "1", features = ["bundled"] }`（解析为 1.10505.0；Cargo.lock +430 行属正常传递依赖） |
| `prompts/clusters/knowledge-base/SKILL.md` | 方法表 +2 行、返回形状 +2、空结果表 +1、gotcha 表 +2、低自由度路径 +1（注意：此文件属未跟踪新目录，**不在 git 跟踪内**） |
| `docs/plans/2026-07-31-struct-query-virtual-tables.md` | §13 开工顺序 P1c 打勾；附录 A「P1c 实现证据」 |

### 核心行为（struct_query.rs）

- 存储约定：`<STRUCT_STORE_DIR 或 storage/struct_store>/<doc_id>.duckdb`；文件缺失 → 跳过/`no_relations`（「无表格」非错误）。
- 加固打开：`access_mode=READ_ONLY` → `SET enable_external_access=false; SET lock_configuration=true;`（顺序不可换，先锁后 SET 无效）。
- SQL 白名单 `validate_sql`：仅单条 SELECT（禁 `;`）；禁 FROM/JOIN 子查询（绕过 relation 解析）；禁词表含 attach/copy/install/load/pragma/set/DDL/DML/prepare/execute/macro/read_*/sqlite_scan 等；FROM/JOIN 标识符必须 ∈ scope catalog（大小写不敏感）；跨 doc relation → `cross_doc`。
- 可修复错误（forbidden/unknown_relation/no_relations/cross_doc/execute）以 `ok:false + error.code` 进 data（ToolStatus 仍 Ok），模型可自纠；系统级错误（开库失败等）走 `error_result`。
- 硬顶：`MAX_RESULT_ROWS=200`（多取 1 行判 truncated）、`MAX_SAMPLE_ROWS=3`、`MAX_CELL_CHARS=300`；duckdb 调用包 `spawn_blocking`；v1 无硬超时（计划已声明）。
- 证据：选了 `row_ord` 列时回传 `{doc_id, row_ord, __src_line}` 逐行；`__chunk_id` 映射待 2b。
- catalog `headers` 过滤掉 `row_ord`/`__src_line`（但 sample_rows 含），`_meta` 缺表时 caption/confidence 为空不报错。

## 2. 验证证据（全部实测通过）

```
cargo test -p avrag-rag-core --lib struct_query   → 10/10 PASS（R1 修复后重跑：5 原有 + 5 新增）
cargo test -p avrag-rag-core --lib                → 91 passed（R1 修复后重跑）
cargo test -p avrag-code-interpreter --lib        → 15 passed（含 shim parity）
cargo test -p agent-loop --lib sdk_gate           → 6 passed
cargo check -p app-chat                           → 通过（3 条既有 warning，与本改动无关）
```

探针（`/tmp/duckdb_probe`）实测：duckdb-rs bundled 能读 Python duckdb 1.5.5 写的 `/tmp/poc_ipd.duckdb`（COUNT=370）；READ_ONLY 拒 CREATE、`enable_external_access=false` 拒 read_csv、`lock_configuration=true` 后 SET 撤销被拒。bundled 编译约 5m37s（单 crate，一次性）。

## 3. Review 发现（**2026-07-31 已全部应用并验证**，按优先级）

### R1（重要）测试盲区：query_store 闭包逻辑零覆盖

`run_query` 闭包内的可见性收集 / unknown_relation / no_relations / cross_doc / execute 错误码 / 证据行回传——正是安全核心——**没有测试**。现有 5 个测试只覆盖 `validate_sql` / `catalog_for_file` / `query_rows` / 加固断言。

修法：把 `run_catalog` / `run_query` 的闭包抽成纯函数

```rust
fn catalog_store(dir: &Path, doc_uuids: &[Uuid]) -> Result<Vec<serde_json::Value>, String>
fn query_store(dir: &Path, doc_uuids: &[Uuid], sql_arg: &str) -> Result<serde_json::Value, String>
```

`run_*` 只剩参数解析 + `run_blocking(move || ...)`。然后补测试（fixture 已就绪）：
- `DELETE FROM t0` → `error.code == "forbidden"`
- `SELECT * FROM t9` → `unknown_relation`
- 空目录 + `SELECT 1` → `no_relations`
- 双 fixture 文件 `t0 JOIN t1` → `cross_doc`
- `SELECT no_such_col FROM t0` → `execute`
- happy path `WHERE 阶段='验证阶段' ORDER BY row_ord` → `row_count==2`、`evidence[0].row_ord=="1"`（注意证据值是 String）
- `catalog_store` 混合存在/缺失文件 → 只返回存在的

⚠️ 注意：文件已被 `rustfmt --edition 2024` 重排（如 `resolve_doc_uuids` 的 error_result 折行、测试断言折行），做 edit 时 oldText 必须以**当前文件**为准，别抄本会话旧片段。

### R2 死代码

`run_query` 闭包里 `files.clear();` 多余（闭包结束即 drop），连带 `let mut files` 的 `mut` 可去。R1 重构时一并消除。

### R3 冗余分支

`run_query` 末尾 `if is_ok { ok_result(...) } else { ok_result(...) }` 两分支相同，合并为 `Ok(data) => ok_result("struct_query", data, started)`。R1 重构时一并消除。

### R4 SKILL.md 语气（third-person 规则）

`prompts/clusters/knowledge-base/SKILL.md`「默认低自由度路径」新增行是祈使式。把

> 先 `struct_catalog` 确认表名与列名，再 `struct_query` 写单条 SELECT；…catalog 为空 = 该 doc 无表格存储，回到 grep。

改为观察式：

> `struct_catalog` 给出可见表名与列名；`struct_query` 执行单条 SELECT；「第一个」= `row_ord` 升序第一行（表出现序），非编号字典序；catalog 为空 = 该 doc 无表格存储，此情形 grep 仍可用。

## 4. 环境/操作要点

- **全部命令直接在 WSL 执行**（agent bash 即 WSL Ubuntu），禁止套 `wsl.exe`。
- Rust 构建遵守 `CARGO_BUILD_JOBS=2`（`docs/agent/rust-resources.md`）；libduckdb-sys 编译重，别并发跑多个全量 test。
- 完整验证序列（R1 修复后重跑）：
  ```bash
  cd /home/chuan/context-osv6/avrag-rs
  CARGO_BUILD_JOBS=2 cargo test -p avrag-rag-core --lib struct_query
  CARGO_BUILD_JOBS=2 cargo test -p avrag-rag-core --lib
  CARGO_BUILD_JOBS=2 cargo test -p avrag-code-interpreter --lib
  CARGO_BUILD_JOBS=2 cargo test -p agent-loop --lib sdk_gate
  ```
- Python 侧 venv：`/tmp/struct_poc/bin/python3`（duckdb 1.5.5 / markdown-it-py）；pip 用清华镜像。
- PoC 资产：`scripts/struct_query_poc/`（extract_tables / pipeline / supervise + check_*）、测试产物 `/tmp/poc_ipd.duckdb`、`/tmp/sup_baiyao.duckdb`、markdown 源 `/tmp/markitdown_out/`。
- **工作树有大量非本任务脏改动**（modes/、prompts/ 重构、orchestrator 等）。提交时必须只挑本功能文件，别 `git add -A`。本功能涉及文件见 §1 表格 + `Cargo.lock`。

## 5. 下一步

1. ~~落 R1–R4~~（已完成 2026-07-31：R1 抽纯函数 + 5 测试、R2/R3 随重构消除、R4 语气改写；验证序列全绿）。
2. ~~P1d~~（已完成 2026-07-31：重灌 ipd/白药 struct store → `avrag-rs/storage/struct_store/<doc_id>.duckdb`；切片 86/88 答对、106 sticky 未解（末轮停在 code 块未出 synthesis，非 struct 缺陷）；证据见计划文档 §11 B3/B4 与附录 B）。
3. 2b：`__src_line → __chunk_id` 证据映射（需与 chunker 联动）；真实会计报表样本验合计对账（q106 类问题）。
4. P2：supervision loop 工具化 / fts 字段发现 / 数值规整 / telemetry。
