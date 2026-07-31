# struct-query W2 S4 收尾窗口交接：本地验收门 + 退役收尾（2026-07-31）

> 上接：`docs/plans/2026-07-31-struct-query-w2-s4-window2-handoff.md`（主链落地）。
> 本窗口 = W2 §5 剩余项全清 + W3 A5 并入。执行方式：主线 + 3 个并行 subagent（secondary 模型）。

## 0. 一句话

W2 S4 **全部收官**：本地生产路径验收门四环全过（真 api+worker 灌 ipd xlsx → struct stage → duckdb+证据 → 删 doc 随删），验收中抓出并修复一个**真 bug**（检索重建擦除 table_evidence）；deploy 脚本/jvm bin/死脚本/retired 引用清理完毕；A5 进回归。

## 1. 本窗口产出

| 内容 | 文件 | 验证 |
|---|---|---|
| 窗口 2 变更入库（66 文件，+803/-7226） | commit `b0dc1722` | 复测：ingestion 78 / worker 31 / supervision 41 / storage-pg 1 全绿 |
| deploy 脚本 markitdown provisioning（幂等：uv → pip --user fallback → 显式报错；office:9090/PDF renderer:9091 标注退役） | `scripts/deploy-backend.sh:142-174` | `bash -n` 过；**未执行部署** |
| `bins/office-parser-jvm` 物理删除 + workspace member 移除 | `avrag-rs/Cargo.toml`、`Cargo.lock` | `cargo metadata` 无此包；check worker/ingestion 绿 |
| 退役死引用清理：`product-dev-up.sh` 去 office 窗口 + pdf-renderer 块；删 4 个死脚本 | `scripts/product-dev-up.sh`、`avrag-rs/scripts/{office-parser,pdf-renderer}-{up,down}.sh` | `bash -n` 过 |
| **bug 修复**：`store_document_body_chunks` 全量 DELETE chunks → 改为 `chunk_type <> 'table_evidence'`（表格阶段先于 materialize 跑，证据曾被检索重建擦掉——本地验收实测） | `crates/storage-pg/src/lib_impl/repository_bootstrap.rs:243` | 回归测试断言证据在检索重建后存活 |
| A5 监督干预成功案例进回归（假表头方言 → rotate_header 守卫两侧 + SQL 复验） | `crates/struct-supervision/src/store.rs`（`a5_eaten_header_rotate_header_sql_recheck`） | 29 lib + 13 bin 全绿 |

## 2. 本地验收门实测（四环）

环境：本地 api + worker（debug build，`E2E_ENABLED=true`，`STRUCT_STORE_DIR` 两侧同绝对路径），真实 PG/Milvus/MinIO，xlsx 原件 = `crates/app/tests/product_e2e/fixtures/huawei_ipd_370_activities.xlsx`。

| 环 | 结果 |
|---|---|
| ① ingestion + struct stage | worker 日志：`struct table stage done grids=1 evidence_chunks=1 turns=2`（supervision 真 LLM 2 轮） |
| ② 表格存储 | duckdb t0 **370 行** + `fts_main_t0` schema + `match_bm25('LPDT')` 47 命中 + `_meta confidence=high`；catalog 可见性另有 W1 切片（同 store 目录）覆盖查询侧 |
| ③ 证据水合 | **首跑失败**：`table_evidence` 行被 materialize 阶段的 `store_document_body_chunks` 全量 DELETE 擦掉 → 修复后重灌：`body=128 summary=1 table_evidence=1` 共存 |
| ④ 删 doc 随删 | DELETE `/api/v1/documents/{id}` → cleanup task 完成 → duckdb + sidecar + PG chunks 全清，doc status=deleted |

排障记录（下次省时）：PG `chunks` 是 forced RLS——直查须事务内 `set_config('app.current_user', <owner>, true)`；cleanup task 执行有轮询延迟（本次 ~100s），「no tasks」日志不等于任务未入队；DELETE 路由是 `/api/v1/documents/{id}`（不在 /workspaces/{id} 下）。

## 3. 验证 gate（收尾态）

```bash
CARGO_BUILD_JOBS=2 cargo test -p avrag-struct-supervision      # 42（29 lib + 13 bin）全绿
set -a; source .env; set +a
CARGO_BUILD_JOBS=2 cargo test -p avrag-storage-pg --lib table_evidence  # 1/1 真 PG（含新回归）
CARGO_BUILD_JOBS=2 cargo check -p avrag-worker -p ingestion -p avrag-struct-supervision  # 绿
bash -n scripts/deploy-backend.sh scripts/product-dev-up.sh  # 过
```

## 4. 残留（不阻塞，另案/另线）

1. **VPS 部署与停服**：deploy-backend.sh 已备 markitdown provisioning 但未执行；VPS 上 office parser / PDF renderer 服务停属部署决定（用户：VPS 先不管）。
2. **staging e2e 三测试**（`office_{docx,pptx,xlsx}_staging_e2e.rs`，`#[ignore]`，探 `OFFICE_PARSER_BASE_URL` 外部服务）与 `test_context/config.rs:268` env 转发残留——staging 退役时一并清。
3. `THIRD_PARTY_NOTICES.md:378`（+ frontend_next 副本）仍列 `avrag-office-parser-jvm` 包名——法务清单更新另案。
4. `cargo check -p app --tests` 仍被另一条在途线阻塞（`delegate_contract.rs`）——非本线，不修。
5. storage-pg 全量 lib 5 个预存失败（开发库数据状态）——另线另查（首窗口已双盲证明非本线）。

## 5. 操作要点

- 本地起 dev 栈：`scripts/product-dev-up.sh`（tmux：minio/api/worker/next；office/renderer 窗口已去）；worker 需要 PATH 上有 `markitdown` CLI。
- 手动验收脚本骨架：`/tmp/accept_ids.json` 流程（register → workspace → POST documents → PUT `/dev-upload/{id}` → poll status → 验 duckdb/PG → DELETE）。注意 `E2E_ENABLED=true` 才开 `/dev-upload`。
- 改提取器后必跑三语料 parity（`dump_grids` vs `--emit-grids`）。
- 脏树纪律不变：本窗口 commit 只挑上述文件；SaC 线脏文件不碰。
