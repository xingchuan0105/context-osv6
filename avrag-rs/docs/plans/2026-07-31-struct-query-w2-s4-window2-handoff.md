# struct-query W2 S4 第二窗口交接：markitdown 唯一解析器 + 表格阶段挂接（2026-07-31）

> 上接：`docs/plans/2026-07-31-struct-query-w2-s4-progress.md`（首窗口）。
> 上游计划：`docs/plans/2026-07-31-struct-query-post-p2-dev-plan.md` W2。

## 0. 一句话

**W2 S4 主链已全部落地**：markitdown 成为生产唯一文档解析器（office parser / PDF renderer / liteparse 全线退役），表格阶段挂进 `document_pipeline`（md → supervise → per-doc duckdb → 证据入库，随 doc 生命周期清理）——代码与单测/集成验证全绿，剩余为收尾杂项（deploy 脚本 / commit / graphify）与一次生产路径验收，外加一个**非本窗口引起**的 app 测试编译阻塞。

## 1. 架构决定（用户拍板，不可逆方向）

- **整体替换，非过渡**：生产 parser 一步到位换成 markitdown 子进程，不做「office 服务 + markitdown 过渡并存」方案。worker 对一切文档类（xls/xlsx/doc/docx/ppt/pptx/pdf/txt/md/html/csv/代码文件）统一 `Local(Markitdown)` 路由。
- 退役即删除：office parser service client、PDF renderer client、liteparse 全链（probe/页路由/visual_pdf）、calamine excel 解析器、pdf_plan/page_routes、paddle 结果缓存——**代码物理删除，不保留开关**。office-parser-jvm bin 目录暂留（无人调用，VPS 停服属部署决定）。
- 表格阶段消费 markitdown 原始 markdown（`ParseRunState.markdown`），不重新跑 markitdown。

## 2. 本窗口产出

| 内容 | 文件 | 验证 |
|---|---|---|
| markitdown 子进程后端：`MARKITDOWN_BIN`（默认 PATH `markitdown`）、`MARKITDOWN_TIMEOUT_MS`（默认 120s）、临时文件进出、超时 kill、非零退出带 stderr 截断报错 | `crates/ingestion/src/parser/markitdown.rs`（新） | 真二进制单测（本机 markitdown 0.1.5） |
| md→IR：`blocks_from_markdown` 从 E2E harness `markitdown_reingest.rs` **逐字移植**（语义 parity 钉死）；`parse_markitdown_document_ir` 返回 `(DocumentIr, markdown)` | 同上 | parity 单测 |
| `ParseBackend::Markitdown`（wire 名 "markitdown"，与 harness 既有数据一致） | `crates/ingestion/src/ir.rs` | 78/78 lib 测试 |
| router 重写：文档全类 → markitdown；standalone 图片仍走 PaddleOCR；`ParsePlan::Pdf/Office`、probe、页路由类型全删 | `crates/ingestion/src/parser/route.rs` 等 | 同上 |
| worker parse 路径：Local → markitdown；office/excel/text/code/pdf 分支全删；pdf/ 模块瘦身到 `paddle.rs`（图片路径）；`ParseServiceDeps` 收口删除 | `bins/worker/src/pipeline/document_pipeline/parse.rs`、`bins/worker/src/pdf/`、`processor.rs`、`lib.rs` | worker lib 31/31 |
| **表格阶段** `struct_stage.rs`：ir_project 后挂点；grids 为空则清旧产物；`supervise` 复用 `processor.llm.ingestion_llm`；duckdb 落 `<STRUCT_STORE_DIR>/<doc_id>.duckdb`；证据 `replace_table_evidence_chunks` 幂等入库；**best-effort：任何失败只 warn，不阻断主链** | `bins/worker/src/pipeline/document_pipeline/struct_stage.rs`（新）+ `mod.rs` 挂点 | 组件级（supervise mock / repo 真 PG / 提取器 parity 各自已验） |
| runner 收尾把 evidence 随报告带出 | `crates/struct-supervision/src/runner.rs`（`SuperviseReport.evidence`） | 41/41（28 lib + 13 bin），含三语料 parity |
| 删 doc 随删 duckdb+sidecar：`remove_struct_store_files` 挂 `process_document_cleanup_task` | `bins/worker/src/ingestion_guard.rs` | 编译+接线检查 |
| E2E harness 去 mock office：mock_office_server 及全部接线删除；docx/xlsx/pptx 三测试改断言真实 markitdown 产出；转发变量换 `MARKITDOWN_*`/`STRUCT_*` | `crates/app/tests/product_e2e/`（mod/mock_servers/builder/config + 3 个 integration 测试） | 见 §4 阻塞 |
| **pptx fixture 重建**：旧 `phase0-mini.pptx` 是退化文件（缺 `<p:spPr>`），markitdown（python-pptx 系）无法解析 → 用 python-pptx 重建为合法 fixture（文本不变） | `crates/app/tests/product_e2e/fixtures/phase0-mini.pptx` | 本机 markitdown 实跑确认产出 |
| `.env.example`：退役变量标注失效 + 新增 `MARKITDOWN_BIN` / `MARKITDOWN_TIMEOUT_MS` / `STRUCT_STORE_DIR` / `STRUCT_SUPERVISE_MAX_TURNS` | `avrag-rs/.env.example` | — |

退役删除清单（物理删除）：`crates/ingestion/src/parser/{excel.rs, office_service.rs, probe.rs, liteparse*.rs, visual_pdf.rs, pdf_renderer_service.rs, pdf.rs, pdf_image.rs, paddle_cache.rs, pdf_plan/, page_routes.rs}` 及 3 个 liteparse 集成测试、`bins/worker/src/pdf/` 7 个文件、`crates/app/tests/product_e2e/mock_office_server.rs`。`page_status.rs` 保留（图片多模态索引仍在读）。worker Cargo.toml 去掉 calamine/liteparse/lopdf/flate2/base64，加 `avrag-struct-supervision`。

## 3. 行为变化矩阵（运维/排障必读）

| 场景 | 旧行为 | 新行为 |
|---|---|---|
| docx/xlsx/pptx/pdf 摄入 | office parser 服务 / liteparse 管道 | worker 子进程 `markitdown`（worker 环境须预装 `markitdown` CLI） |
| 文档里的图片/多模态资产 | liteparse 视觉链 + PDF renderer | **消失**（已知取舍）；standalone 图片文件仍走 PaddleOCR |
| 扫描件 PDF | liteparse OCR 页路由 | markitdown 提不出文本 → 空 IR → 按既有零块校验拒收 |
| office 服务（:9090）/ PDF renderer（:9091） | 摄入依赖 | 不再被调用；`OFFICE_PARSER_*`/`LITEPARSE_*`/`PDF_RENDERER_*` env 失效 |
| 无表格文档重灌 | — | 表格阶段清旧 duckdb + 删旧证据行（幂等） |
| 表格阶段失败（LLM 挂等） | — | 只记 warn，主链照常；**旧 duckdb/证据保留**（同 version 内容一致，优于清空） |

## 4. 验证 gate（本窗口实测）

```bash
CARGO_BUILD_JOBS=2 cargo test -p ingestion --lib                 # 78/78 绿
CARGO_BUILD_JOBS=2 cargo test -p avrag-worker --lib              # 31/31 绿
CARGO_BUILD_JOBS=2 cargo test -p avrag-struct-supervision        # 41/41 绿（28 lib + 13 bin，含三语料 parity）
set -a; source .env; set +a
CARGO_BUILD_JOBS=2 cargo test -p avrag-storage-pg --lib table_evidence  # 1/1 真 PG 绿
CARGO_BUILD_JOBS=2 cargo check -p avrag-worker                   # 绿（无新 warning）
```

**已知阻塞（非本窗口引起）**：`cargo check -p app --tests` 挂在 `crates/app/tests/delegate_contract.rs:94/123`——`AgentApp::execute_chat` 不存在、`ChatRequest` 缺 `capabilities/client_context/client_ip`。这是工作区另一条在途线（agent-loop/app-chat 重构，约 49 个脏文件）的半成品。**不要替那条线修**；等其收尾或更新该测试调用点后，app 测试套件（含本窗口改的三个 office e2e）才能整体编译。

## 5. 剩余（下一窗口）

1. deploy 脚本（`scripts/deploy-*.sh`）加 markitdown 安装依赖（`pip install 'markitdown[all]'` 或 uv tool）——未动。
2. `graphify update .`（结构性改动后必跑）+ commit：**只 add 本窗口文件**，工作树有另一条线的脏文件。
3. 生产路径验收门：app+worker 活进程灌 `huawei_ipd_370_activities.xlsx` → struct_catalog 可见 → 证据水合 → 删 doc 后 duckdb/证据随删。注意 `STRUCT_STORE_DIR` 两侧（app 查询/worker 写入）须指向同一目录（默认相对各自 cwd 的 `storage/struct_store`）。
4. office-parser-jvm bin 处置 + VPS 停服（部署决定，随 1 一起做）。

## 6. 操作要点

- worker 新运行时依赖：**markitdown CLI 必须在 worker 的 PATH**；缺失时文档摄入以「markitdown 子进程启动失败」报错（非静默降级）。
- 排障入口：backend_summary 里 route/plan 现为 `local`/`markitdown`；`probe_result`、`page_backends`（per-page 后端明细）字段已随 plan 类型删除。
- parity 对拍（改提取器后必跑三语料）：`cargo run -p avrag-struct-supervision --example dump_grids <md>` vs `pipeline.py <md> --emit-grids -`。
- 脏树纪律不变：commit 只挑本窗口文件；storage-pg 全量 lib 的 5 个预存失败与本线无关（首窗口已双盲验证）。
