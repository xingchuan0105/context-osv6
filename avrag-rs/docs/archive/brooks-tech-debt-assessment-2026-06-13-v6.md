# Brooks-Lint Review — 技术债深度评估

**Mode:** Tech Debt Assessment
**Scope:** `avrag-rs` + `frontend_next` + `contracts` + `desktop`（v6；LiteParse P4 全量切换后深度复查，重点检查缺口、BUG、漂移）
**Health Score:** 57/100
**Trend:** 61 → **57**（-4 vs v5）

**一句话结论：** Brooks 满分计划的多数结构性债务已经偿还，但 LiteParse 全量切换后暴露出新的入库链路债务：独立图片 Paddle 路由仍缺少端到端产物保护，LiteParse 对同一 PDF 多次重复解析，`-D warnings` 门禁目标被一批未清理的 P4 残留破坏，且部分历史文档仍在描述已删除的 shadow/灰度/MinerU 过渡期。

> **归档：**
> - v1 → [`archive/brooks-tech-debt-assessment-2026-06-12-v1.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v1.md)（Health 34）
> - v2 → [`archive/brooks-tech-debt-assessment-2026-06-12-v2.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v2.md)（Health 58）
> - v3 → [`archive/brooks-tech-debt-assessment-2026-06-12-v3.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v3.md)（Health 70）
> - v4 → [`archive/brooks-tech-debt-assessment-2026-06-12-v4.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v4.md)（Health 59）
> - v5 → [`archive/brooks-tech-debt-assessment-2026-06-13-v5.md`](./archive/brooks-tech-debt-assessment-2026-06-13-v5.md)（Health 61）

> **事实更正（2026-06-13 PR Review v6）：** 本报告中关于 `LiteParseImage` 的 Critical finding 已被后续 P4 代码取代：当前 router 将独立图片路由到 `ParseRoute::PaddleOcrImage` / `ExternalParseKind::PaddleOcrImage`，worker 走 `execute_paddle_ocr_image`。剩余风险应收窄为“独立图片 Paddle 产物缺少 E2E/asset contract 保护”，而不是“图片仍走 LiteParse 抽字”。本报告未重新计分；以最新 PR Review v6 为准。

---

## 1. 审计范围与方法

| 维度 | 说明 |
|------|------|
| 配置 | 无 `.brooks-lint.yaml`，六类衰减风险全部启用 |
| 重点变更 | LiteParse 主链、MinerU/Shadow 删除、Office Excel-only、Paddle Jobs、前端 HTTP 统一残留 |
| 证据来源 | `graphify query`、代码路径追踪、`rg` 残留扫描、`cargo test -p ingestion -p avrag-worker`、`cargo test --no-run -p app --test product_e2e --features product-e2e`、`cargo check -p avrag-worker` |
| 验证结果 | 功能测试通过，但 worker/ingestion 编译输出仍有大量 warning；Product E2E 可编译但测试夹具仍有 warning |
| 优先级公式 | Pain × Spread（1–3）；7–9 Critical debt / 4–6 Scheduled / 1–3 Monitored |

### 1.1 v5 → v6 关键变化

| 指标 | v5 | v6 复查 | 结论 |
|------|----|---------|------|
| `atomic_tools` / `helpers` | 报告中仍按 860+ 行单文件计 | 已拆成目录与子模块 | ✅ v5 此项已过时，移出发现 |
| `pg_share_store.rs` | 报告中仍按 1062 行单文件计 | 已拆成 `pg_share_store/` 目录（12 个源文件：mod/mappers/port_impl + 9 个 `shards.lst` 分片） | ✅ v5 此项已过时，移出发现 |
| `agents/loop/mod.rs` | 仍是主循环热点 | 文件 1289 行；`run` / `run_auto_fallback` 仍在同文件 | ⚠️ 仍保留为认知负担项 |
| 独立图片路径 | v5 未覆盖 | 当前已路由到 `PaddleOcrImage`，但缺少 image E2E/asset contract | 🟢 PR Review v6 已收窄 |
| LiteParse PDF 解析次数 | v5 未覆盖 | 路由 probe + `page_dimensions` + `extract_blocks`，失败/预算降级还会再 parse | 🟡 新性能债 |
| P4 警告门禁 | v5 未覆盖 | `cargo test -p ingestion -p avrag-worker` 绿但有 30+ warning | 🟡 与 `-D warnings` 目标漂移 |
| 当前架构文档 | v5 未覆盖 | `liteparse-paddle-ingestion-architecture-2026-06-13.md` v1.2 + `worker-dev.md` 已对齐 P4；开关仅留 archive | ✅ M6 已关闭 |
| 前端 HTTP 统一 | v5 说基本完成 | `billing/featureFlag.ts` 仍从 `auth/client` 拿 `buildApiUrl` 并手写 fetch | 🟢 小残留 |

---

## Findings

### Critical

**Domain Model Distortion — [Superseded] 独立图片路径 finding 已被 P4 代码收窄**

Symptom: 原报告认为 `router/mod.rs` 将图片路由到 `ExternalParseKind::LiteParseImage` 且 worker 只调用 `LiteParseService::parse_file`。后续 P4 代码已改为 `ParseRoute::PaddleOcrImage` / `ExternalParseKind::PaddleOcrImage`，`parse_route.rs` 调用 `execute_paddle_ocr_image`。因此这条 finding 的原始事实已过时；剩余可保留的问题是独立图片缺少端到端产物测试。

Source: Evans — *Domain-Driven Design*, Ubiquitous Language；Ousterhout — *A Philosophy of Software Design*, Information Leakage

Consequence: 若继续按原 finding 执行，会把工程精力花在不存在的 `LiteParseImage` 分支上；真正需要保护的是 Paddle image ingest 产物是否包含可检索文本、Figure asset 与正确 metadata。

Remedy: 不再修 `LiteParseImage`。补一个图片 E2E 或 fake Paddle contract：上传 png，断言 `doc_type=image`、`paddle_jobs_count=1`、至少存在一个 searchable text block 或 Figure asset；如需 MM 原图落库，再把 asset 存储路径和 chunking contract 补齐。

Priority: Pain 3 × Spread 2 = **6**（Scheduled, but severity Critical because current route can silently lose image recall） | Intent: **[accidental]**

---

### Warning

**Accidental Complexity — LiteParse 主链对同一 PDF 重复解析，成本随页数放大**

Symptom: PDF 路由阶段 `probe_pdf_hybrid` 会调用 `LiteParseService::probe` 解析一次；执行阶段 `execute_pdf_parse` 又调用 `page_dimensions` 解析一次、`extract_blocks` 解析一次；预算跳过或 Paddle 失败时会再次 `extract_blocks`。这些函数内部都重新 `parse_input(PdfInput::Bytes(...))`。也就是说同一 PDF 在正常路径至少被 LiteParse 解析 3 次，异常路径更多。

Source: Hunt & Thomas — *The Pragmatic Programmer*, Orthogonality；Ousterhout — *A Philosophy of Software Design*, Tactical Programming

Consequence: P4 取消 shadow/灰度后，LiteParse 是唯一主链；重复解析会直接拉长大 PDF 入库时间，并增加内存峰值。后续调优会被迫在多个函数之间传递“已经解析过的事实”，形成新的 change propagation。

Remedy: 让 `LiteParseService` 提供一次性 `parse_pdf(bytes) -> LiteParseParsedDocument` 或 `ParsedPdfSnapshot`，包含 page probes、dimensions、text blocks；路由和执行共享该快照，或者在 `execute_pdf_parse` 内至少把 `page_dimensions` 与 `extract_blocks` 合并为一次 parse pass。

Priority: Pain 2 × Spread 3 = **6**（Scheduled） | Intent: **[accidental]**

**Accidental Complexity — P4 删除旧路径后留下大量 warning，`-D warnings` 门禁目标漂移**

Symptom: `cargo test -p ingestion -p avrag-worker` 全绿，但输出包括 `ParseProbe` 未用导入、旧 `router/stages/*::route` 未用、`office_convert.rs` 的 `is_image_file/temp_pdf_path` 未用、worker pipeline 多个未用导入、`DocumentIrPdfExt` 未用等 30+ warning。`cargo check -p avrag-worker` 同样输出这些 warning。此前 Brooks 计划已把 `RUSTFLAGS="-D warnings"` 作为 smoke/integration 的质量门槛，但当前 worker/ingestion 路径还不能稳定满足这个目标。

Source: McConnell — *Code Complete*, Construction Hygiene；Winters et al. — *Software Engineering at Google*, Code Sustainability

Consequence: warning 会掩盖真实新增问题，也会让 `-D warnings` 在不同 workflow 或本地命令中表现不一致。新同学无法判断哪些是“可忽略的历史遗留”，哪些是 P4 删除旧路径造成的漏清理。

Remedy: 删除未用 `router/stages` 旧分阶段路由或重新接回；清理 worker/pipeline 未用导入；移除 `parse_json` 的 shadow diff 注释和未用函数，或用 `#[cfg(test)]`/明确 allow 标注历史兼容原因。然后在 ingestion/worker 的 CI job 中加入 `RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker`。

Priority: Pain 2 × Spread 3 = **6**（Scheduled） | Intent: **[accidental]**

**[Resolved — M6] Knowledge Duplication — LiteParse 架构文档与 runbook 已对齐 P4**

Symptom（历史）: 架构文档曾把 shadow/灰度/MinerU 开关写成现行部署路径。

现状（2026-06-13）: `docs/liteparse-paddle-ingestion-architecture-2026-06-13.md` v1.2 标明 P4 已实现；§0/§14 明确无 shadow/灰度；删除项指向 `archive/p4-mineru-shadow-migration-historical.md`；`docs/runbooks/worker-dev.md` 已标注 MinerU 与 `LITEPARSE_*` 已删除。

验收: `rg 'LITEPARSE_ENABLED|LITEPARSE_SHADOW|MINERU_' docs/*.md docs/runbooks` 命中仅限 archive/历史说明或「已删除」标注行。

Priority: ~~Pain 2 × Spread 2 = **4**~~ → **已偿还**

**Cognitive Overload — `execute_pdf_parse` 同时编排路由、OCR、降级、metadata 与状态写入**

Symptom: `bins/worker/src/pdf/parse.rs` 约 362 行；`execute_pdf_parse` 内部同时计算 text/ocr/table 页面集合、调用 Paddle Jobs、处理预算跳过、做 LiteParse 降级、做 Visual fallback、写 metadata/warnings、再调用 B 类图片增强和 page_status。P4 删除 legacy 后，这个函数成为唯一 PDF 主控路径，但它仍混合了策略判断和执行细节。

Source: Fowler — *Refactoring*, Long Method / Divergent Change；McConnell — *Code Complete*, High-Quality Routines

Consequence: 修 OCR 预算、修 Visual fallback、修 metadata schema、修 B 类图增强都会进同一个函数；每次改动都要重新理解整条入库链，容易引入重复 page parse 或 page_status 漏标。

Remedy: 拆出 4 个内部阶段：`collect_page_routes`、`run_ocr_pages`、`apply_text_fallbacks`、`attach_ingest_metadata_and_status`。每个阶段只返回一个结构化结果，`execute_pdf_parse` 保持薄 orchestration。

Priority: Pain 2 × Spread 2 = **4**（Scheduled） | Intent: **[accidental]**

**Change Propagation — 前端 HTTP 统一仍有 `billing/featureFlag.ts` 残留旧入口**

Symptom: `frontend_next/lib/http/request.ts` 已提供 `buildApiUrl`/`decodeApiError`/`requestJson`，但 `frontend_next/lib/billing/featureFlag.ts` 仍从 `../auth/client` 导入 `ApiError, buildApiUrl` 并手写 fetch。v5 报告中“8 个 client 全部 import 统一入口”的结论因此不完整。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY；Winters et al. — *Software Engineering at Google*, Hyrum's Law

Consequence: 认证 URL 构造和 HTTP 错误格式仍有一条旁路；如果 `http/request` 后续调整 API base、错误 envelope 或 IPC/HTTP 分叉，这个文件可能静默漂移。

Remedy: 将 `featureFlag.ts` 改为使用 `lib/http/request` 的统一函数；若它必须保留轻量 fetch，至少从 `http/request` 导入 `buildApiUrl` 和 `ApiError`，不再依赖 `auth/client`。

Priority: Pain 1 × Spread 2 = **2**（Monitored, but counted as Warning because it invalidates the previous completion claim） | Intent: **[accidental]**

---

### Suggestion

**Domain Model Distortion — LiteParse 全量切换后仍保留 `EdgeParse` / `Mineru*` 历史枚举名**

Symptom: `PdfPageBackend::EdgeParse` 现在实际代表 LiteParse 数字文本路径；`ParseBackend::MineruImage` / `MineruPdfOcr` 仍在 IR 枚举中用于历史文档兼容。代码中也有 `merge_page_legacy_backend`、`PdfPageBackend::PaddleOcr` 等历史语义。

Source: Evans — *Domain-Driven Design*, Ubiquitous Language；Fowler — *Refactoring*, Primitive Obsession / Alternative Classes

Consequence: 新开发者看到 `EdgeParse` 会以为 lopdf 主链仍在；看到 `MineruImage` 会以为 MinerU 仍是支持后端。短期为历史 IR 兼容可以接受，但需要明确边界。

Remedy: 保留 `ParseBackend::Mineru*` 但加 `#[deprecated]` 或注释“历史 IR only”；新增 `PdfPageBackend::LiteParseText`，逐步替换计划层的 `EdgeParse` 命名；迁移完成后只在历史反序列化层保留旧名。

Priority: Pain 1 × Spread 2 = **2**（Monitored） | Intent: **[intentional]**（历史兼容）

**Accidental Complexity — `LiteParseService` 仍暴露 shadow-era API 与注释**

Symptom: `LiteParseService::parse_json` 注释为 “Export parse result as JSON for shadow diff”，`LiteParsePageProbe::to_pdf_page_probe` 注释为 “legacy probe struct for router compatibility during migration”。P4 已删除 shadow diff 文件和 shadow CLI，但这些 API 仍留在生产模块里。

Source: Fowler — *Refactoring*, Speculative Generality / Lazy Class

Consequence: 这些函数会暗示 shadow diff 仍是支持能力，也会诱导后续代码继续依赖迁移期适配层。它们本身不是高风险，但会加重命名漂移。

Remedy: 若仅测试使用，移入 `#[cfg(test)]`；若没有引用，直接删除。迁移期注释改为 P4 后真实语义，例如“convert LiteParse probe into router probe model”。

Priority: Pain 1 × Spread 1 = **1**（Monitored） | Intent: **[accidental]**

**Cognitive Overload — `agents/loop/mod.rs` 仍是 1289 行主循环聚合点**

Symptom: M2 已将 `iteration_codegen`、`iteration_tools`、`run_result` 等拆出，但 `agents/loop/mod.rs` 仍为 1289 行，并保留 `pub async fn run` 和 `run_auto_fallback` 两个核心入口。这个文件不再是本轮最高风险，但仍是 agent 行为变更的主要导航成本。

Source: Fowler — *Refactoring*, Long Method；Ousterhout — *A Philosophy of Software Design*, Deep Modules

Consequence: 新增 exit policy、fallback 或 tracing 时仍会回到同一个大文件；虽然风险比 v5 降低，但仍会拖慢后续 agent 改动。

Remedy: 在下一轮小步拆 `run` 的 LLM-call/parse/dispatch/state-update 四段；把 fallback 策略移到 `fallback.rs`，`mod.rs` 只保留编排接口。

Priority: Pain 1 × Spread 2 = **2**（Monitored） | Intent: **[accidental]**

---

## Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|-------------|----------------|--------|
| Cognitive Overload | 2 | 3.0 | Monitored/Scheduled | accidental |
| Change Propagation | 1 | 2.0 | Monitored | accidental |
| Knowledge Duplication | 1 | 4.0 | Scheduled | accidental |
| Accidental Complexity | 3 | 4.3 | Scheduled | accidental |
| Dependency Disorder | 0 | — | Clean | — |
| Domain Model Distortion | 2 | 4.0 | Scheduled | mixed |

**Recommended focus:** 先修独立图片链路（功能缺口），再清 warning 与文档漂移，最后优化 LiteParse 单次解析缓存。Agent loop 可继续排期，但已经不是这次 P4 后最紧急的风险。

---

## 2. 偿还路线图（v6）

### 2.1 第一档：必须先修（57 → 72）

| # | 任务 | 验收 |
|---|------|------|
| 1 | 独立图片 `PaddleOcrImage` 补 E2E/asset contract | 图片 E2E：上传 png 后 text chunk 或 MM chunk 至少一类存在；无 MinerU 引用 |

### 2.2 第二档：上线卫生（72 → 97）

| # | 任务 | 验收 |
|---|------|------|
| 2 | LiteParse parse pass 缓存/合并，避免同一 PDF 正常路径 parse 3 次 | 单 PDF 正常路径 LiteParse parse 次数 ≤1 或 ≤2；大文件耗时基线下降 |
| 3 | 清理 ingestion/worker warning，恢复 `-D warnings` 可用性 | `RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker` 通过 |
| 4 | 更新 LiteParse 架构文档与 runbook，删除 shadow/rollout/MinerU 现行描述 | ✅ M6：`rg` 命中仅限 archive/「已删除」标注 |
| 5 | 拆 `execute_pdf_parse` 阶段函数 | 主函数 <120 行；各阶段有单元测试或现有 worker tests 通过 |
| 6 | 前端 `billing/featureFlag.ts` 接统一 HTTP 层 | `rg 'from "../auth/client"' frontend_next/lib/billing` 为 0 |

### 2.3 第三档：命名与收尾（97 → 100）

| # | 任务 | 验收 |
|---|------|------|
| 7 | `EdgeParse` / `Mineru*` 历史枚举加注释或迁移命名 | 新计划层不再用 `EdgeParse` 表示 LiteParse；历史 IR 兼容保留有注释 |
| 8 | 删除/测试门控 `parse_json`、`to_pdf_page_probe` 等迁移 API | `rg 'shadow diff' crates/ingestion/src/parser` 为 0 |
| 9 | `agents/loop/mod.rs` 继续薄化 | `mod.rs` <900 行；fallback 迁到 `fallback.rs` |

---

## 3. 验证记录

```bash
cargo test -p ingestion -p avrag-worker --quiet
# 通过；但输出 30+ warning

cargo test --no-run -p app --test product_e2e --features product-e2e --quiet
# 通过；product_e2e fixture 仍有 warning

cargo check -p avrag-worker --quiet
# 通过；但输出 worker/ingestion warning
```

---

## 4. 附录：关键代码锚点

| 路径 | 观察 |
|------|------|
| `crates/ingestion/src/parser/router/mod.rs` | 图片扩展名路由到 `ParseRoute::PaddleOcrImage`；PDF 始终 `probe_pdf_hybrid` |
| `bins/worker/src/pipeline/parse_route.rs` | `PaddleOcrImage` 分支调用 `execute_paddle_ocr_image`；剩余缺口是 E2E/asset contract |
| `crates/ingestion/src/parser/liteparse.rs` | `probe` / `extract_blocks` / `page_dimensions` 各自重新 parse bytes |
| `bins/worker/src/pdf/parse.rs` | 唯一 PDF 主链，混合 OCR、fallback、metadata、page_status |
| `docs/liteparse-paddle-ingestion-architecture-2026-06-13.md` | 仍描述 shadow/rollout/一键回退旧链 |
| `frontend_next/lib/billing/featureFlag.ts` | HTTP 统一残留旁路 |

---

*生成工具：Brooks-Lint Tech Debt Assessment · 2026-06-13 v6（LiteParse P4 后深度探查）*
