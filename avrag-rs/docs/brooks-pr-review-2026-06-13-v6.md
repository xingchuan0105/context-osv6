# Brooks-Lint Review

**Mode:** PR Review  
**Scope:** staged changes（360 files, +46118/-12813）+ unstaged changes（61 files, +1910/-3058）；采样覆盖 LiteParse/Paddle P4 入库切换、MinerU 删除、worker PDF 主链、product smoke runner、Brooks 文档归档、前端 runtime transport 残留、CI workflow 根目录归并。**审查为采样模式**，变更体量远超单次 PR 可完整逐行评审范围。  
**Health Score:** 48/100  
**Trend:** 63 -> 48 (-15) vs PR Review v5

**一句话结论：** P4 方向继续推进，图片路由已从 v5 误判的 `LiteParseImage` 改成 `PaddleOcrImage`，但当前工作区再次处于“删旧与录新不在同一层”的半完成态，且 PR smoke 脚本会在真正执行测试前被清单解析挡住，不应合并。

---

## Findings

### 🔴 Critical

**Change Propagation — Git 变更集再次分裂：MinerU 删除、LiteParse 新文件、文档归档不在同一完整层**

Symptom: `git status --short` 显示 staged 层已有 360 文件，同时 unstaged 层还有 61 文件；其中 `crates/ingestion/src/parser/mineru/**` 是删除，新的 `liteparse.rs`、`liteparse_config.rs`、`liteparse_ir.rs`、`liteparse_probe_bridge.rs`、`paddle_cache.rs`、`router/page_routes.rs`、LiteParse tests、`bins/worker/src/pdf/office_convert.rs` 仍是 `??` 或 unstaged 修改。文档层也同时存在 current docs 修改、archive 新文件、旧报告移动。

Source: Brooks — *The Mythical Man-Month*, Brooks's Law / coordination cost；Feathers — *Working Effectively with Legacy Code*, change set safety

Consequence: reviewer 和 CI 看到的可能不是同一个系统。只提交 staged 层会留下已引用但未提交的 LiteParse/worker 文件，或者留下已删除 MinerU 但未完整替代的入库链路；只看本地编译也会被磁盘上的未跟踪文件掩盖。

Remedy: 先决定本轮是“P4 入库切换 PR”还是“Brooks 文档 PR”。若是 P4，使用 `git diff --name-status` 对照确认 MinerU 删除、LiteParse 新文件、worker wiring、tests、docs 同批进入；若要拆 PR，先切出 license/workflow、Brooks docs、LiteParse runtime 三个独立变更集，每个变更集独立可编译。

**Coverage Illusion — PR smoke runner 的模块清单解析为空，测试会在执行前失败**

Symptom: `scripts/run-product-smoke-e2e.sh` 用 `sed -n 's/^smoke::\([^:]*\)::.*/\1/p'` 解析 `cargo test --test product_e2e -p app --features product-e2e smoke:: -- --list`；实际列表前缀是 `product_e2e::smoke::auth_boundary::...`。我刚运行同一列表命令可看到 20 个 smoke tests，但套用脚本里的 sed 后输出为空。

Source: Google — *How Google Tests Software*, change coverage；Meszaros — *xUnit Test Patterns*, Erratic Test / Mystery Guest

Consequence: `smoke-e2e.yml` 的关键保护不是“跑红了业务测试”，而是清单守卫误判。PR 可以在 smoke 真实覆盖缺失的情况下卡死，开发者也无法从失败判断 auth/share/chat/rag 哪条路径真的坏了。

Remedy: 把解析改为匹配 `.*::smoke::<module>::`，或用 Rust test list JSON/稳定结构化输出替代 sed；给 `assert_smoke_module_coverage` 加一个 fixture 级脚本测试，随后重跑 `./scripts/run-product-smoke-e2e.sh` 确认至少进入 non-RAG smoke 阶段。

---

### 🟡 Warning

**Accidental Complexity — LiteParse PDF 主链对同一 PDF 至少解析三次**

Symptom: 路由阶段 `probe_pdf_hybrid` 调 LiteParse probe；执行阶段 `execute_pdf_parse` 又调用 `page_dimensions`，随后 `extract_blocks`；预算降级或 Paddle 失败时还会再次 `extract_blocks`。这些 API 内部都各自 `parse_input(PdfInput::Bytes(...))`。

Source: Hunt & Thomas — *The Pragmatic Programmer*, Orthogonality；Ousterhout — *A Philosophy of Software Design*, Tactical Programming

Consequence: P4 后 LiteParse 是主链，重复 parse 会直接放大大 PDF 入库耗时和内存峰值。后续要做性能优化时，已解析的页面事实会被迫穿过 router、worker、fallback 多处接口，继续扩大 change propagation。

Remedy: 让 `LiteParseService` 暴露一次性 parsed snapshot（包含 page signals、dimensions、text blocks），在 router/worker 内共享；短期至少合并 `page_dimensions` 与 `extract_blocks` 的 parse pass。

**Accidental Complexity — P4 删除旧路径后仍留下 warning 噪音，`-D warnings` 门禁目标漂移**

Symptom: 轻量 smoke list 命令本次重新编译时仍输出 ingestion 10 个 warning 与 product_e2e 2 个 warning，包括 `ParseProbe` unused import、旧 `router/stages/*::route` unused、`TEXT_QUAL_THRESHOLD` unused、`ready_rag_context/shared_*` unused 等。

Source: McConnell — *Code Complete*, construction hygiene；Winters et al. — *Software Engineering at Google*, code sustainability

Consequence: 若 CI 恢复 `RUSTFLAGS="-D warnings"`，当前入库/worker/test 组合会在真正业务测试前失败；若不恢复，P4 删除旧路径后的死代码会继续混入主链，后续很难判断哪些 stage 仍有语义价值。

Remedy: 删除或重新接线 `router/stages/*` 旧 stage，清理 unused imports/constants；把 `RUSTFLAGS="-D warnings" cargo check -p ingestion -p avrag-worker` 加回一个明确 gate，避免 warning 从“临时噪音”变成常态。

**Domain Model Distortion — P4 已切到 LiteParse/Paddle，但核心枚举仍用 `EdgeParse` / `Mineru*` 旧语言**

Symptom: `PdfPageBackend::EdgeParse` 现在实际代表 LiteParse 数字文本路径，`ParseBackend` 仍暴露 `MineruPdfOcr` / `MineruImage`。同时 router tests 仍断言 Figure/Text 页 backend 为 `EdgeParse`，读者需要知道“EdgeParse 名字已经不等于旧 lopdf 主链”。

Source: Evans — *Domain-Driven Design*, Ubiquitous Language；Fowler — *Refactoring*, Primitive Obsession / Alternative Classes

Consequence: 新开发者排查 P4 入库问题时会把旧 EdgeParse/MinerU 语义带回当前链路，误判回退路径仍存在；监控与 backend metadata 也更难表达“当前真实 parse backend”。

Remedy: 新增 `PdfPageBackend::LiteParseText` 或注释 `EdgeParse` 为历史 wire name；`ParseBackend::Mineru*` 若仅为历史 IR 反序列化保留，应加 deprecated/compat 注释并限制新代码引用。

**Knowledge Duplication — 技术债报告已加事实更正，但未重新计分**

Symptom: `brooks-tech-debt-assessment-2026-06-13.md` 现在已加 PR Review v6 事实更正，并把原 `LiteParseImage` finding 标为 superseded；但它仍保留原 Health Score 57/100 和原 Critical 段落作为审计记录，没有重新跑完整 Tech Debt Assessment。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY；Ousterhout — *A Philosophy of Software Design*, Information Leakage

Consequence: 读者如果只看分数或旧段落，仍可能误判当前风险级别；如果只看更正，又可能以为整份技术债报告已完成复评。

Remedy: 后续单独跑 `/brooks-debt` 生成 v7，正式重算技术债 Health Score；在此之前，以 PR Review v6 的图片路径结论和 LiteParse/Paddle 架构文档为当前事实源。

---

### 🟢 Suggestion

**Dependency Disorder — `billing/featureFlag.ts` 仍绕过 runtime transport**

Symptom: 前端多数 client 已走 `lib/http/request.ts` -> `runtime/transport.ts`，但 `frontend_next/lib/billing/featureFlag.ts` 仍从 `auth/client` 引 `buildApiUrl` / `ApiError` 并直接 `fetch("/api/v1/billing/usage/window")`。

Source: Martin — *Clean Architecture*, DIP；Winters et al. — *Software Engineering at Google*, Hyrum's Law

Consequence: Web 路径没问题，但桌面 Tauri IPC 下这个 bucket-aware pricing probe 仍绕过统一 transport；错误类型、cookie/credential 行为和 abort 语义可能与其他 billing client 漂移。

Remedy: 改为复用 `request<UsageWindowProbeEnvelope>` 或在 `http/request.ts` 暴露 probe 所需的轻量 helper；保留 feature-disabled 判断，但不要再依赖 auth client 的 URL builder。

**Coverage Illusion — LiteParse 新 E2E 只覆盖 PDF，独立图片主链缺少端到端保护**

Symptom: 新增 `liteparse_pdf_e2e.rs` 覆盖 `phase0-mini.pdf` 上传、完成入库、chunk 数、`liteparse_hybrid` metadata 与 page raster 断言；router unit test 覆盖图片扩展名路由到 `PaddleOcrImage`。但 worker 的 `execute_paddle_ocr_image` 没有对应 E2E 或 fake Paddle test 证明 image upload 会产出 text chunk / figure asset / metadata。

Source: Google — *How Google Tests Software*, change coverage；Feathers — characterization tests

Consequence: 独立图片是 P4 替代 MinerU 的显性入口之一，目前只验证“路由选择”，没有验证“入库产物”。Paddle API shape 或 asset mapping 漂移时，问题会在真实上传后才暴露。

Remedy: 加一个 fake Paddle image ingest 测试或 product E2E fixture：上传 png，断言 `doc_type=image`、`paddle_jobs_count=1`、至少存在一个 searchable text block 或 Figure asset。

**Recommended fix order:** 先修变更集完整性与 smoke parser 两个 blocker，再清 warning gate；随后处理 LiteParse parse snapshot、旧枚举命名和文档事实收敛；最后补前端 billing probe 与独立图片 E2E。

---

## Summary

v6 相比 v5 有两处重要修正：前端统一 HTTP 的大部分残留已收敛，只剩 `billing/featureFlag.ts`；独立图片路由也已不再是 `LiteParseImage`，而是 `PaddleOcrImage`。当前最大风险转移到 P4 切换的变更集完整性、smoke runner 真实保护能力，以及 LiteParse 主链的重复 parse/警告噪音。合并前不要只看本地磁盘编译结果，先把 staged/unstaged/untracked 拆清楚，并让 smoke 脚本真正跑到测试阶段。

---

*报告生成：2026-06-13 · Brooks-Lint PR Review v6 · v5 已归档至 `docs/archive/brooks-pr-review-2026-06-13-v5.md`*
