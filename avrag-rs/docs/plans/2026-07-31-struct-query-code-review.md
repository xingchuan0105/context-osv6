# struct-query 线代码审查报告：BUG / DRIFT / GAP + 退役残留 + 另案诊断（2026-07-31）

> **修复回执（2026-08-01）**：六轨修复已完成并全部通过 gate（分轨 commit 见本报告后续 git 历史）。
> 摘要：H1/H2/M1/M2/M3 全修；validate_sql 族模式+逗号多关系拒、catalog sample/headers 对齐、ambiguous_relations 透出；
> 退役残留 B1/B2/B5 + config/ir.rs/env.example/notices 全清（含 AGPL PyMuPDF 服务删除）；storage-pg D3 五项测试缺陷修复，两轮全绿零残留。
> 未做（用户既定边界）：VPS 停服、delegate_contract.rs、双栏修复策略、106 专项、segment 精确行级、`INGESTION_PDF_MAX_PAGES` 产品决策。

> 审查方式：主线 + 3 个 review subagent（R1 代码逐行审 / R2 退役残留全量定位 / R3 另案诊断）。只读，未改代码。
> 范围：struct-query 功能线全部新代码（extract/checks/store/runner/directives/struct_query/struct_stage/markitdown/chunker 行区间/storage-pg 证据面）+ 退役面 + 另案三项。

## 0. 总览

| 类别 | 数量 | 最高严重度 |
|---|---|---|
| BUG | 高 2 / 中 3 / 低 4（另：已修复 3 项登记） | 高（安全绕过 + 静默错证据） |
| DRIFT（双侧/注释/文档漂移） | 8 | 中（total_reconcile 求和口径） |
| GAP（承诺未兑现/兜底缺失） | 8 | 中（validate_sql 校验层承诺不实） |
| 退役残留 | 现役红灯 2（B1/B2）+ 五类 20 余项 | B1 使 L2 套件必红 |
| 另案诊断 | D1/D2/D3 全确诊 | D3 证明 5 个预存失败全是测试缺陷，无产品 bug |

---

## 1. BUG（按严重度）

### 高

**H1. 重灌失败时 `_line_map` 与旧 duckdb 静默矛盾（行级证据错配）**
`document_pipeline/mod.rs:120-172` + `struct_stage.rs:104-108,176-237`。`stage_struct_tables` 在 supervise 失败时保留旧 duckdb，但 `stage_struct_line_map` 无条件用**新版本** body chunk（新 chunk_id、新行区间）重建旧库的 `_line_map`——旧行 `__src_line` 映射到新版文本区间，行级 citation 静默指错文本。同版本重灌时该重建反而是必要的（修复旧 chunk_id 悬空），不能简单删。**修**：struct_tables 返回「本轮是否产出新库」，3.5 据此分流；或 supervise 失败时同步 DROP `_line_map` 降级纯表级。

**H2. supervision 只读守卫大小写绕过（安全）**
`struct-supervision/src/session.rs:19,243`：禁词正则无 `case_insensitive`（Python 母本 `supervise.py:32` 有 `re.I`，移植丢失；已独立复核）。`Read_Csv('/etc/passwd')` 即绕过；且内存库未关 `enable_external_access`，supervision LLM 上下文含文档正文 → 提示注入可读文件经 notes 外泄。同仓 `struct_query.rs:377` 正确用了 `RegexBuilder::case_insensitive(true)`。**修**：一行 Builder 调用 + 禁词表对齐 struct_query 全量表（缺 read_parquet/read_text/glob 等）+ 补失败测试。

### 中

**M1. `auto_rotate` 丢列后新表头未按 keep 过滤 → 制造 ragged**
`extract.rs:284-297` + Python `pipeline.py:118` 同款（惰性移植）。Unnamed 列被丢时表头比数据行宽 → column_count 全灭。`directives.rs:65-71` 的 rotate_header 做法正确可参照。两侧同修 + 补「auto_rotate+实际丢列」测试。

**M2. `runner.finish` `.expect` panic 击穿 best-effort 承诺**
`runner.rs:183`：write_duckdb 的 IO 失败直接 panic（LLM 错误都走 `?`，同函数 sidecar 却用 `let _ =`——策略自相矛盾）。struct_stage 承诺「失败只 warn」被 unwind 击穿。**修**：finish 返回 Result，IO 错走 Err 降级（与 H1 同一条失败语义线，建议同做）。

**M3. `total_reconcile` Rust 求和口径错（D1 确诊）**
`checks.rs:337-342` 只排除首个合计行，其余小计/总计仍计入求和（万科 t114 实测 Rust 7.62T vs Python 2.29T，~3.3× 膨胀；「总计在前小计在后」版式 Rust 直接误报）。**Python 侧是正确口径**（设计文档附录 C「首个合计行 vs 全叶子行」为证）。**修**：Rust 过滤器改「首格不命中 TOTAL_LABEL_RE」+ 补版式单测。

### 低（登记，不展开）

- `directives.rs:182-201` reparse_region 切格与 Python 不一致（跳空格/末格保留面）
- `repository_assets.rs:232` `::bigint` 强转：一行坏 md_line_start → 全 doc _line_map 重建失败（加 `~'^\d+$'` 过滤）
- `directives.rs:122-133` merge_tables 目标表角案（tid 不在首位/自并）守卫未覆盖
- 已修复登记：PDF 校验豁免（780b931e）、table_evidence 检索重建存活（9da69557）、mock_office_abort（780b931e）

## 2. DRIFT

| 项 | 位置 | 差异 |
|---|---|---|
| total_reconcile 容差 | `checks.rs:344` vs `pipeline.py:232` | Rust 加性 vs Python 取大（实务无害，随 M3 同改） |
| `grid::clip` 丢省略号 | `grid.rs:92` vs `supervise.py:57` | LLM 观察无法区分截断值/完整值；struct_query.rs:93 有省略号（仓内自相矛盾） |
| 进度观察条件 | `runner.rs:147` vs `supervise.py:333` | Rust 少了「至少一次工具调用」门槛，空调用时每 8 轮多一条干扰 |
| sequence 整数口径 | `checks.rs:304-319` vs `pipeline.py:214` | Rust 接受负号/拒绝 unicode 数字，与 Python 相反；`hi-lo+1` 极端值 i64 溢出 debug panic |
| `ParseBackend` 语义滞留 | `ir.rs:46-118` | `canonical_pdf_text()` 死函数（无调用方）注释称 "canonical main chain"；LiteParse*/CalamineExcel 未收录进 `is_historical_ir_only()` |
| paddle 元数据名不副实 | `bins/worker/src/pdf/paddle.rs:129,177` | 活代码仍写 `ingest_route_version="liteparse-v1"` |
| 活文档过时 | `docs/runbooks/worker-dev.md:48-205`（教人跑已删脚本）、`docs/README.md:33`（liteparse 架构文仍称「当前实现真相源」）、`CONTEXT.md:22` | 操作性误导最严重的一类 |
| `e2e-test-registry` 生成器与产物 | `generate-e2e-test-registry.py:25-513` + `docs/e2e-test-registry.yaml:456-615` | 仍注册 liteparse/office 项，B1 死测试标 `ci_default: true` |

## 3. GAP

| 项 | 位置 | 缺口 |
|---|---|---|
| `validate_sql` 校验层承诺不实 | `struct_query.rs:35-65,375-401` | `read_csv_auto`/`read_parquet` 家族绕过词边界禁词；逗号连接第二关系逃出 FROM/JOIN 采集。三层兜底（连接加固+per-doc 隔离）实测仍拦住，但「标识符 ∈ catalog」在校验层不成立——要么补强（族模式 `\bread_[a-z_]+\b`）要么文档明示兜底分层 |
| `STRUCT_STORE_DIR` 部署无兜底 | `struct_stage.rs:21` / `struct_query.rs:67` / `deploy/systemd/*` | 默认相对 cwd，worker/app 两侧 cwd 不同即静默分裂（catalog 永空无报错）；本地验收靠手工设绝对路径 |
| `markdown=None` 不清旧产物 | `struct_stage.rs:62-64` | doc 换类型重灌（markitdown→图片）旧 duckdb/旧证据永久残留（grids 空才清，None 直接 return） |
| catalog sample_rows/headers 列数不齐 | `struct_query.rs:204-227` | headers 滤系统列、sample 保留（SELECT *），模型按位置对不齐 |
| 跨 doc 同名表静默归属 | `struct_query.rs:493` | 多 doc scope `FROM t0` 落首个含 t0 的 doc，不 surfaced |
| supervise 成功后证据写失败悬空 | `struct_stage.rs:132-167` | duckdb 已重写但 PG 证据未入 → 表级水合失效到下次灌入，仅 warn |
| `INGESTION_PDF_MAX_PAGES` 静默失效 | 4 个测试仍设置，生产无读取方 | 页数上限能力是否在 markitdown 管线重建属产品决定 |
| `<>` 白名单维护陷阱 | `repository_bootstrap.rs:246` | 未来新增外挂证据型 chunk_type 会被检索重建静默擦掉——新增 chunk_type 时必须记得加保留清单（注释已注明，无机制兜底） |
| worker 可观测性 | D2 | stage 日志无 `attempt_count`（「stage×2」不自解释）；`task_timeout_secs=300` 对重型 PDF 偏小 |

## 4. 退役残留清单（R2 全量，按处置优先级）

**现役红灯（先灭）：**
1. `crates/app/tests/product_e2e/integration/liteparse_pdf_e2e.rs`（+`integration/mod.rs:12`）——**无 `#[ignore]`**，`test-l2-integration.sh` 无过滤 → L2 套件必红。删或改写为 markitdown 断言。
2. `avrag-rs/scripts/run-liteparse-staging-e2e.sh`（整删）+ `run-staging-ingest-e2e.sh:29-51`（liteparse/office 段）——点名跑死测试的死脚本。

**顺手清（小）：**
3. `scripts/product-dev-up.sh:99-100,110` 三行死服务回显（office:9090/renderer:9091 健康址 + renderer 日志路径）。
4. `test_context/config.rs:268,272-273` 从转发白名单删 4 个死 key（OFFICE_PARSER_BASE_URL/PDF_RENDERER_BASE_URL/PDF_VISUAL_PAGES_PER_CHUNK/PADDLE_OCR_RESULT_CACHE_ENABLED）。
5. `ir.rs` 死函数/滞留注释（DRIFT 表已列）。
6. `docs/runbooks/worker-dev.md`、`docs/README.md:33`、`CONTEXT.md:22` 三处活文档改写。

**法务链（有先后）：**
7. 删 `avrag-rs/services/pdf-visual-renderer/`（含 AGPL PyMuPDF）→ `check-licenses.sh:91-95` 段 → 改 `generate-third-party-notices.sh:51,104-107` → 重生成 `THIRD_PARTY_NOTICES.md`（:12/:378/:535/:940）→ `sync-legal-assets.sh` 同步 frontend → `licenses/page.tsx:16` 改写 markitdown。

**其余登记：**
8. staging e2e 三测试（`office_*_staging_e2e.rs`，`#[ignore]`）删除；`llm_real/pdf_{rag,corpus}.rs` liteparse 断言改写（nightly 门）。
9. `.env.example:165-175` LITEPARSE_* 整段 + `:261-262` PDF_VISUAL_*；`.env` 死变量（惰性无害）。
10. 注释级：`docker-compose.milvus.yml:50`、`stage-desktop-sidecars.sh:86`、`runbooks/milvus-wsl-manual.md:65`。
11. VPS :9090/:9091 停服——随部署决定（deploy-backend.sh provisioning 已备）。

## 5. 另案诊断结论（D1/D2/D3）

- **D1 total_reconcile**：Python 侧正确，修 Rust（M3，附精确位置与最小修复）。
- **D2 worker 重复处理**：**同一 task 失败重试、整条 pipeline 从头重放**（audit_log 实证：1 次入队 → N 次 started/failed → completed；并发 claim 被 `FOR UPDATE SKIP LOCKED`+进程内 advisory 锁排除，重复入队被幂等键排除）。幂等兜底有效（文档最终正常），代价是每次重试全额重付 parse/embedding/LLM 成本。建议：超时调大或分阶段、stage 日志加 attempt_count、parse-validation 类错误尽早不可重试化。
- **D3 storage-pg 5 预存失败**：**全是测试缺陷，无产品 bug**——3× 裸 SQL 缺 `set_config('app.current_user',…)`（forced RLS 静默过滤）、1× 伪造租户未注册 users（FK 23503）、1× 共享库队列隔离（自续污染：每轮失败净增 ≥2 条 queued 行，此后 claim 必挂）。修法逐条在案。警示：多轮失败已向开发队列灌 4 条 queued 残留，本地 worker 起后会误 claim 烧重试——是否清库由你决定（纪律上我没动）。

## 6. 建议处置顺序

1. **灭红灯**：B1 死测试（L2 必红）+ B2 死脚本。
2. **安全与静默错**：H2 大小写（一行+测试）、H1 _line_map 矛盾（阶段分流）。
3. **口径**：M3 total_reconcile 修 Rust（含容差顺手）、M1 auto_rotate（双侧）。
4. **失败语义线**：M2 finish Result + R1#18 悬空证据（同一条线）。
5. **兜底明示**：validate_sql 族模式或文档分层声明、STRUCT_STORE_DIR 部署固定绝对路径。
6. **退役长尾**：§4 第 3-11 条按序。
7. **另案**：D2 观测增强（attempt_count/超时分级）；D3 测试修复（归 storage-pg 线或顺手）；低危 DRIFT 组随过路修。

## 7. 确认无问题（抽查过的区域，摘）

- extract.rs：split_cells 转义奇偶、围栏状态机、CRLF/unicode、ragged 截断补空、merge_continuations 首行保留/剔除口径——与 markdown-it 实证一致。
- store.rs：quote_ident/FTS 列名注入面已封、quarantine 两侧一致、rebuild_db 幂等。
- struct_query 加固主线：READ_ONLY + LOAD 顺序、多语句/子查询/非 SELECT 拒、catalog 排除面（_meta/_line_map/fts 六内表）、行级 containing/nearest 语义与测试。
- W6 段：chunker 缺键降级是有意保守（markitdown 单一路径下不触发）；repository_assets 两个新方法的过滤/排序/RLS 有集成测试。
- runner loop 预算/终态/工具配对（除 M2/进度观察两项外）与 Python 母本对齐。
