# 视觉 PDF 入库与检索改造 —— 需求整理（2026-06-10）

> **修订**：2026-06-10 审核后更新（§8.2 为准）。初版「彻底删除 MinerU / Phase 1 全做 triplet / 默认 4 页」已按 P0/P1 意见调整。

来源：E2E `llm_real` 真实 PDF 语料验证、MinerU 配置/限流排查、OCR 替代方案讨论（PaddleOCR / 多模态 embedding / PyMuPDF）。

关联代码：`crates/ingestion/`、`bins/worker/`、`crates/llm/src/embedding.rs`、`crates/rag-core/src/runtime/`、`crates/app/tests/product_e2e/llm_real/pdf_corpus.rs`。

---

## 1. 背景与动机

### 1.1 原始目标

在 **真实大部头 PDF**（非 40 行 txt 夹具）上验证 `llm_real` RAG 行为，确认：

- 多工具检索是否在真实语料上仍「单步 dense」为主；
- citation / chunk 粒度是否足够支撑复杂对比题；
- 整条链路（上传 → 入库 → Milvus → codegen 检索 → 回答）可在生产配置下跑通。

### 1.2 测试语料

| 文件 | WSL 路径 | 体量 | 备注 |
|------|----------|------|------|
| Antifragile | `/mnt/e/OneDrive/桌面/Taleb_Antifragile__2012.pdf` | ~7.7MB，581 页 | 电子版，大部分页可抽字 |
| Black Swan（pdfdrive 扫描版） | `/mnt/e/OneDrive/桌面/the-black-swan_-the-impact-of-the-highly-improbable-second-edition-pdfdrive.com-.pdf` | ~2.8MB，567 页 | 每页 `text_chars=0` |

环境变量（`pdf_corpus.rs` 已支持默认值）：

- `E2E_LLM_REAL_ANTIFRAGILE_PDF`
- `E2E_LLM_REAL_BLACK_SWAN_PDF`

### 1.3 已实现的 E2E 基建（本对话前序）

- `pdf_corpus.rs`：`real_llm_rag_complex_query_antifragile_pdf`、双书对比用例
- `test_context.rs`：`upload_file_from_path`、`fetch_document_status`、`worker_log_tail`；llm_real worker 透传 `MINERU_*` / `OFFICE_PARSER_*` / embedding 等
- `new_with_real_llm_pdf()`：入库等待超时 1800s
- txt 夹具 `llm_real` 6 项已通过；smoke 18 项已通过

---

## 2. 当前入库架构（现状）

```
PDF 上传
  → ParseProbe 逐页路由
      ├─ EdgeParse（lopdf 抽字）     — 数字页
      └─ MineruOcr（云端 OCR）      — 低文字页 / 复杂版式页
  → DocumentIr → text_chunks + multimodal_chunks（Figure/SlideImage）
  → text-embedding-v4 + BM25(text_sparse) + multimodal_dense + graph(triplets)
```

### 2.1 两本书实测路由（probe，与 worker 一致）

| 书 | 总页数 | EdgeParse | MinerU OCR | 判定原因 |
|----|--------|-----------|------------|----------|
| Antifragile | 581 | **515（88%）** | 66（12%） | 66 页单页文字 < 100 字（封面/插图/版式页），**非**「整书图像」 |
| Black Swan 扫描版 | 567 | **0** | **567（100%）** | 每页 0 可抽字，整本当扫描件 |

**结论**：并非「两本书都全走 MinerU」；Antifragile 主要走本地抽字。慢的主因是 **MinerU 云端 OCR + API 限流**，不是本地 PDF 处理。

### 2.2 MinerU 故障记录

| 现象 | 根因 |
|------|------|
| 入库 `queued → processing → failed`，约 13 分钟 | 非 Key 失效（新 Key 已验证 `POST /extract/task` 可用） |
| Worker 日志 | `429 Too Many Requests — 50 files/min limit`（66 页 OCR 一次 batch 超限） |
| 代码修复（已写，需 worker 重编译生效） | `mineru.rs` OCR batch 拆为每批 ≤50 页，批次间等待 61s |
| E2E 踩坑 | `cargo test -p app` 不会自动重编 `avrag-worker`；旧二进制未带上分批逻辑 |

### 2.3 本地没有「PDF → PNG」

当前 MinerU 路径：`pdfseparate` / `lopdf` **拆单页 PDF 字节**上传云端，**不做**本地渲染。

此前对话中「转 PNG 慢」是按 poppler `pdftoppm` 的**保守假设**，非实测，也非现有代码路径。

### 2.4 现状声明核验（代码对表，2026-06-10 审核）

| 文档声明 | 核验 |
|----------|------|
| 多模态块仅 `Figure \| SlideImage` | ✅ `ir.rs:279` `supports_multimodal_chunking` 严格匹配 |
| ParseProbe 逐页 `text_chars` 路由 | ✅ `probe.rs` + `ir.rs` `page_text_chars` |
| `dense_retrieval` 只查 text_dense + lexical 兜底 | ✅ `dense.rs:93` 仅 `retrieve_text_dense_stage`，空则 fallback lexical |
| 图输入 rate limiter 只估 100 tokens | ✅ `embedding.rs:217` 纯图 `unwrap_or(100)`，单图实际 ~896+ token（~9× 低估） |
| graph 走向量 / triplet 路线 | ✅ `graph.rs` `placeholder_triplets` → `search_graph` |
| MinerU 牵连面 | ✅ 6 源文件 + 37 处 `MINERU_` 引用 |

### 2.5 已有基建（比文档初稿低估的好消息）

**多模态检索不是从零做**。以下已就绪，当前缺口是「接线」而非「新建子系统」：

| 层级 | 已有能力 | 关键位置 |
|------|----------|----------|
| Embedding | `embed_multimodal_fused` / `MultiModalEmbeddingInput::text_image` | `crates/llm/src/embedding.rs` |
| Milvus | `multimodal_dense` 字段 + 索引、`search_multimodal`、index ops | `crates/storage-milvus/` |
| RAG runtime | `retrieve_multimodal_dense_stage` 已在 `execute_plan` 并行通道中 | `retrieval.rs:225`、`execute.rs` |
| 缺口 | `dense_retrieval` **工具**只调 `text_dense`，未接 multimodal stage | `dense.rs:93` |

→ §4.5 / Phase 2 工作量更接近 **「把已有 stage 接线 + 调权重 + 补 hint」**，而非扩展融合多模态检索基建。

---

## 3. 已评估、暂不采纳的方案

### 3.1 MinerU（云端 OCR）——目标废弃，分阶段退场

- **优点**：版面/Markdown 质量高，与现有 `mineru.rs` 集成完整；财报/公式类文档的质量后备
- **缺点**：429 限流、小时级入库、E2E 临时 localhost URL 问题、持续 API 成本
- **决策（审核修订）**：
  - **Phase 1–3**：保留 MinerU **旁路**（`INGEST_MINERU_ENABLED=1` 或 route 开关），视觉路径为默认
  - **Phase 3 前**：用 MinerU 在代表性 query 上录 **答案质量基线**，供视觉路径回归对比
  - **Phase 3 达标后**：物理删除 `mineru.rs` 及 `MINERU_*`（6 源文件 + 37 处引用）
  - 与「谨慎 Phase 0 spike」一致：**验证达标前不做单向门**

### 3.2 PaddleOCR 本地替代 MinerU

- **结论**：VPS **跑不动**完整文档解析产线
- PaddleOCR-VL / PP-StructureV3 生产配置需 **8GB+ 显存**；CPU 全书不现实
- 仅 PP-OCRv5 轻量 OCR 可本地跑，但不如「云端多模态 embedding」贴合 VPS 约束

---

## 4. 目标方案（待实现）：视觉入库 + 双 Dense 检索

### 4.1 核心思路

VPS 不做本地 OCR；**视觉路径为默认**，MinerU 旁路保留至 Phase 3 质量门槛（见 §3.1）：

1. **有字页**：`lopdf` 抽文字 → `text-embedding-v4` / text chunk（**不渲染、不 OCR**）
2. **无字/扫描页**：页图 → `qwen3-vl-embedding` multimodal chunk（页数策略见 §4.3，**Phase 0 量化后再锁定默认**）
3. **页内插图**：抠 Figure XObject → 单独 image chunk（**不整页渲染**）
4. **Summary / metadata**：Phase 1 必做——视觉 LLM（`INGESTION_LLM`，**已验证可传图**）
5. **Triplets**：**Phase 1.5**（非 Phase 1 硬门槛）——等 spike 确认 VLM triplet 质量后开启；入图前加可信度阈值 / `source: vlm_page_summary` 标注，避免幻觉首日污染 KG
6. **检索**：`text_dense` + **已有** `multimodal_dense` stage 接线；**BM25 不全局废除**；视觉 chunk 带 `retrieval_hint`（见 §4.5）
7. **Graph**：**不关闭**；triplet 走 VLM + 现有 graph 向量索引
8. **Rate limiter**：Phase 1 **硬性前置**——修 `embedding.rs:217` 纯图 100-token 低估（见 §4.5），否则重蹈 MinerU 429

### 4.2 硬约束（API / 平台）

| 约束 | 说明 |
|------|------|
| PDF **不能**直接 multimodal embed | `qwen3-vl-embedding` 只收 text / image(URL或Base64) / video(URL) |
| 单次请求最多 **5 张图**（fusion） | 多页 1 chunk 上限约 5 页/向量 |
| 图 token | 响应含 `image_tokens`；示例约 **~896 tokens/图**（随分辨率变）；总上下文 32k |
| 扫描页无法跳过「变成 image」 | 页本身就是位图，必须进 embedding API |
| `dense_retrieval` 工具**只查 text_dense** | 空结果 fallback lexical；**multimodal_dense stage 已有**（§2.5），工具层未接线 |
| multimodal chunk 类型 | 现仅 `Figure \| SlideImage`；需扩展 **PageRaster** 或等价块类型 |

### 4.3 分页 / chunk 策略（双轨，非一刀切）

与原始动机（citation 粒度支撑复杂对比题）存在张力：4 页 fusion 会让命中块覆盖 1–4 页，对比题更难定位。

| 策略 | API 次数（567 页） | 检索粒度 | 用途 |
|------|-------------------|----------|------|
| 1 页 1 chunk | ~567 | 页级，最细 | **评测 / 对比题**（Phase 0 spike 必测） |
| 4 页 1 chunk（fusion） | ~142 | 4 页一段 | **批量入库默认**（成本达标后） |
| 5 页 1 chunk | ~114 | 更粗 | 省费备选 |

**审核修订**：

- Phase 0 spike **同时测 1 页 vs 4 页**召回（细粒度 query 集）
- 评测集用 1 页；生产批量入库用 4 页——**不在 spike 完成前拍板「默认 4 页」**
- 每 chunk 附短 `caption`（页码范围）参与 fusion，便于 debug 与 citation

### 4.3.1 API 成本粗算（待 Phase 0 实测校准）

瓶颈是 **API 次数与费用**，非渲染。以下为 **断言级粗算**，Phase 0 须产出实测表作为 Phase 1 准入条件：

| 书目 | 视觉页数 | multimodal embed 次数（4页/chunk） | VLM 调用（若每 chunk 1 次 summary+metadata） |
|------|----------|-----------------------------------|---------------------------------------------|
| Black Swan 全书 | 567 | ~142 | ~142（triplet 另计，Phase 1.5） |
| Antifragile 低字页 | 66 | ~17 | ~17 |

**待 spike 填写的限额字段**（否则无法评估会否重蹈 MinerU 429）：

- `qwen3-vl-embedding`：每分钟请求数 / token 限额、单次 fusion 实际 `image_tokens`
- `INGESTION_LLM`（gemini/dmx 网关）：每分钟请求数 / 日配额
- 单书总 token 估算、单书费用区间
- 入库吞吐：在 rate limiter 修复前后的安全 batch 大小与 cooldown

### 4.4 渲染层：PyMuPDF sidecar（独立 HTTP 服务）

- **不用** poppler `pdftoppm`（此前慢估来源于此）
- **采用 PyMuPDF (`fitz`)**：1000 页秒级（抽 embedded JPEG 或低 DPI pixmap）
- **部署形态**：与现有 `office-parser-jvm` 统一——**独立 HTTP sidecar**，非 worker 内嵌进程
  - 参考：`OFFICE_PARSER_BASE_URL=http://127.0.0.1:9090` + `./scripts/office-parser-up.sh`
  - **默认值已定**：`PDF_RENDERER_BASE_URL=http://127.0.0.1:9091`（`PDF_RENDERER_BIND=127.0.0.1:9091`）
  - 启动脚本：`pdf-renderer-up.sh` / `pdf-renderer-down.sh`（对齐 office-parser 模式）
- 接口：`POST /render-pages` → 返回 PNG/JPEG bytes（页码范围 + DPI/策略参数）
- 优先 **`page.get_images()` 抽原图**；必要时 `get_pixmap(matrix=0.75)`（~72dpi）
- **资源隔离**（用户上传 PDF，必须防滥用）：
  - 单请求页数上限（如 ≤20 页）
  - 单页 pixmap 像素上限 / 输出字节上限
  - 请求总超时（如 60s）
  - 解压炸弹防护：PDF 展开后对象数 / 嵌套深度上限
  - 进程级内存上限；崩溃不影响 worker

### 4.5 检索改造要点

**BM25：不全局废除**。仅对视觉路径文档做差异化处理：

- 全局 channel budget **保持 BM25 通道**（txt/md 专名、精确词召回不受影响）
- 视觉入库文档（`ingest_route=visual` 或 `PageRaster` chunk 占比高）：
  - 上调 `multimodal_dense` 权重（扫描书主通道）
  - BM25 若命中视觉 chunk（通常无 text_sparse 或极短 caption），在检索结果中附带**合理提示**，例如：
    - `modality: "page_raster"`、`text_sparse: false`
    - `retrieval_hint: "该片段来自页图向量，无 OCR 正文；引用请标注页码范围"`
  - Agent / synthesizer 消费 hint，避免把空 BM25 命中当正文引用

**dense_retrieval 工具**（`dense.rs`）——**接线为主**：

- 调用已有 `retrieve_multimodal_dense_stage`（`retrieval.rs:225`），与 `text_dense` 合并结果（或按 `modality` 分流）
- embedding 失败时的 lexical fallback **保留**
- 扫描书场景优先 multimodal；text_dense 空结果不强行 fallback 到无意义的 BM25 词匹配
- 工作量：**小**（基建已存在，见 §2.5）

**Graph 通道**（不关闭，triplet 延后）：

- Phase 1：summary/metadata 文本可进 text_dense / summary chunk
- Phase 1.5：VLM triplet → 现有 graph 向量索引；`placeholder_triplets` 带 `source` + 可信度
- 入图前须通过 spike 质量门槛

**Rate limiter（Phase 1 硬性前置，P1→P0 级风险）**：

- 现状：`embedding.rs:217` 纯图输入恒按 **100 token** 计，实际单图 ~896+ token（~9× 低估）
- 批量页图入库会瞬间击穿提供方 token/min 限额（MinerU 429 的同类风险）
- **验收标准**：按 API 响应 `image_tokens` 回写限流；或按分辨率/页数预估算；单测覆盖纯图 fusion 路径
- **必须在 Phase 1 批量 embed 之前合入**，不可作为 Phase 2 可选项

### 4.6 与混合书（Antifragile）的策略

**不要全书渲染**。Probe 后：

- `text_chars ≥ 100` → EdgeParse 文字路径（维持现状，零 API 渲染成本）
- `text_chars < 100` → 页图 multimodal chunk only（66 页量级，可接受）

Black Swan 扫描版：全书走视觉路径，无 EdgeParse。

---

## 5. 非目标 / 已知代价

| 项 | 说明 |
|----|------|
| 句级 / 段级 citation | 页级或 4 页 chunk 级，难对标 OCR Markdown |
| 公式、复杂表格 | 纯图向量弱于 MinerU/Structure OCR |
| Triplet 精度 | VLM 幻觉可能进 KG；**Phase 1.5 才入图**，须可信度阈值（§6） |
| 存储 | 页图 + 向量 + 可选 VLM 文本，一书约 0.5–1GB |
| BM25 对扫描页无效 | 视觉 chunk 无正文时 BM25 自然空命中；靠 multimodal_dense + 检索 hint 引导 Agent |

---

## 6. 建议实施阶段

### Phase 0 — Spike + 成本量化（**准入门槛**，未做）

**产出物即 Phase 1 范围锁定依据**，不仅是技术可行性：

- [ ] **MinerU 质量基线**：代表性 query（含对比题）在 MinerU 路径下录答案 + citation，供 Phase 3 回归
- [ ] Black Swan **前 40 页**，PyMuPDF 抽图/低 DPI 渲染
- [ ] **1 页 vs 4 页/chunk** 各跑一遍 → `qwen3-vl-embedding` fusion embed
- [ ] 细粒度 query 集（3–5 条对比/定位题），测 multimodal_dense 召回与 citation 可解释性
- [ ] **量化表**（必填）：渲染耗时、API 耗时、单次 `image_tokens`、provider 限额、单书费用估算、安全 batch/cooldown
- [ ] VLM 页摘要样本（5–10 chunk），人工评 triplet 可信度，决定 Phase 1.5 是否开启

**Phase 1 准入条件**：上表完成 + rate limiter 修复方案评审通过 + 1页/4页策略选定。

### Phase 1 — 入库管线

**前置（硬）**：

- [ ] 修 `embedding.rs:217` rate limiter（纯图按 `image_tokens` 或页数预估，见 §4.5）

**主体**：

- [ ] `pdf-visual-renderer` sidecar（PyMuPDF，`PDF_RENDERER_BASE_URL=http://127.0.0.1:9091`，含资源隔离）
- [ ] 新 ingest 分支：`PdfVisualIngest`（视觉为默认；**MinerU 旁路保留**，`INGEST_MINERU_ENABLED`）
- [ ] 扩展 IR：`BlockType::PageRaster`——**跨切面改动**，须点名：
  - `ir.rs` `supports_multimodal_chunking`
  - `ir_validator`、`chunker.rs`
  - Milvus index 映射
  - 所有 `BlockType` 穷举 match（编译器兜底，但须逐处审）
- [ ] 按 probe 分流：有字 EdgeParse / 无字 visual
- [ ] VLM：**summary + metadata**（已验证可传图）
- [ ] chunk 页数：按 Phase 0 结论（评测 1 页 / 批量 4 页，或统一策略）

**不含（延后 Phase 1.5）**：

- [ ] VLM triplet 入 graph（须 spike 质量门槛 + 可信度阈值）

### Phase 2 — 检索与 Agent（接线为主，工作量偏小）

- [ ] `dense_retrieval` 接线已有 `retrieve_multimodal_dense_stage`（§2.5）
- [ ] 视觉文档检索结果附带 `retrieval_hint` / `modality` 元数据
- [ ] 上调 multimodal_dense channel budget（扫描书场景）；BM25 保持全局可用
- [ ] Agent prompt：消费视觉 chunk hint，正确引用页码范围

### Phase 3 — 验证与 MinerU 退场

- [ ] `pdf_corpus` llm_real：Antifragile + Black Swan 入库成功、`chunk_count > 1`
- [ ] **答案质量回归**：对比 Phase 0 MinerU 基线（非仅 `distinct_tools` / citation 数）
- [ ] Black Swan **继续用扫描版**作最坏情况压测
- [ ] 质量门槛达标 → **物理删除** MinerU（`mineru.rs`、6 源文件、`MINERU_*` env、E2E 透传）

### Phase 1.5 — Graph triplet（spike 后择机）

- [ ] VLM triplet 管道 + `source: vlm_page_summary` + 可信度阈值
- [ ] 入现有 graph 向量索引；低置信 triplet 不入图或仅作 hint

---

## 7. 已有代码改动清单（本对话，可能未提交）

| 文件 | 改动 |
|------|------|
| `crates/ingestion/src/parser/mineru.rs` | OCR batch ≤50 + 61s cooldown（Phase 3 前保留作旁路/基线） |
| `crates/app/tests/product_e2e/llm_real/pdf_corpus.rs` | 真实 PDF llm_real 用例 |
| `crates/app/tests/product_e2e/test_context.rs` | 文件上传、status、worker 环境变量透传 |
| `crates/app/tests/product_e2e/setup.rs` | `mime_type_for_filename` |
| `avrag-rs/.env` | `MINERU_API_KEY` 已更新（Phase 3 删除 MinerU 时一并清理） |

**Phase 3 MinerU 清理清单**（物理删除时）：

- 源文件：`mineru.rs`、`router.rs` 路由、`mod.rs` 导出、`ir.rs` 引用、`worker/main.rs`、`test_context.rs` 透传
- 环境：`MINERU_API_KEY`、`MINERU_BASE_URL` 等 37 处 `MINERU_` 引用
- 文档：runbook / `.env.example` 注释

**验证命令**（视觉路径实现后）：

```bash
./scripts/pdf-renderer-up.sh      # 待实现
./scripts/office-parser-up.sh
cargo build -p avrag-worker
cargo test -p app --test product_e2e llm_real::pdf_corpus::real_llm_rag_complex_query_antifragile_pdf \
  -- --ignored --test-threads=1 --nocapture
```

---

## 8. 决策记录（含审核修订）

### 8.1 初版决策（2026-06-10 用户确认）

| # | 问题 | 初版决策 |
|---|------|----------|
| 1 | MinerU | 彻底删除 |
| 2 | 多页 chunk | 4 页/chunk fusion |
| 3 | Phase 1 范围 | 全做（含 triplet） |
| 4 | INGESTION_LLM 视觉 | 已验证可传图 |
| 5 | BM25 | 不全局废除；视觉 chunk 带 hint |
| 6 | PyMuPDF sidecar | 独立 HTTP，对齐 office-parser |
| 7 | Black Swan | 继续扫描版压测 |
| 8 | Graph/triplet | 不关闭 |

### 8.2 审核修订（2026-06-10，**以本节为准**）

| # | 原决策 | 修订后 |
|---|--------|--------|
| 1 MinerU | 彻底删除 | **Phase 1–3 保留旁路**；Phase 0 录质量基线；**Phase 3 达标后物理删除** |
| 2 多页 chunk | 默认 4 页 | **双轨**：spike 测 1 页 vs 4 页；评测 1 页 / 批量 4 页；**spike 前不拍板默认** |
| 3 Phase 1 范围 | 全做含 triplet | **summary + metadata + embed**；**triplet 延后 Phase 1.5**（须质量门槛） |
| 4 INGESTION_LLM | 已验证 | **维持** |
| 5 BM25 | 视觉 hint | **维持** |
| 6 PyMuPDF sidecar | 端口待定 | **`PDF_RENDERER_BASE_URL=http://127.0.0.1:9091` 定死**；加资源隔离 |
| 7 Black Swan | 扫描版 | **维持** |
| 8 Graph | 不关闭 | **维持**；triplet 入图随 Phase 1.5，加可信度标注 |
| + | （新增）成本量化 | Phase 0 产出 API 耗时/限额/费用表，**作为 Phase 1 准入条件** |
| + | （新增）rate limiter | **Phase 1 硬性前置**，修 `embedding.rs:217` 9× 低估 |
| + | （新增）基建评估 | multimodal_dense **已有**，Phase 2 为接线非新建 |

---

## 9. 一句话决策摘要（审核后）

> 视觉路径为默认；**MinerU 旁路至 Phase 3**；数字页 lopdf 抽字，扫描页 PyMuPDF sidecar（`:9091`）+ qwen3-vl-embedding；**Phase 0 量化成本/限额 + 1页vs4页 spike** 锁定 Phase 1；**先修 rate limiter** 再批量 embed；Phase 1 做 summary/metadata，**triplet 延 Phase 1.5**；检索 **接线已有 multimodal_dense stage**，BM25 保留、视觉 chunk 带 hint；graph 不关闭。

渲染不是瓶颈（PyMuPDF 秒级）；瓶颈是 **embedding/VLM API 次数与费用**——须在 Phase 0 量化后再承诺 Phase 1 范围。

---

## 10. 审核意见回应摘要

| 优先级 | 意见 | 文档处置 |
|--------|------|----------|
| P0 | MinerU 单向门与分阶段矛盾 | §3.1、§6、§8.2：旁路保留至 Phase 3 |
| P0 | 成本/限流未量化 | §4.3.1、§6 Phase 0：量化表为 Phase 1 准入条件 |
| P1 | rate limiter 100-token 低估 | §4.1、§4.5、§6：升为 Phase 1 硬性前置 |
| P1 | 4 页 fusion vs 对比题粒度 | §4.3：双轨策略，spike 必测 1 页 |
| P1 | VLM triplet 首日污染 KG | §4.1、§6：延 Phase 1.5 + 可信度阈值 |
| P1 | 删 MinerU 前留质量基线 | §6 Phase 0：MinerU 答案基线 |
| nit | §7 env 矛盾 | §7：MINERU_KEY 进 Phase 3 清理清单 |
| nit | PageRaster 跨切面 | §6 Phase 1：穷举改动点 |
| nit | sidecar 资源隔离 | §4.4：内存/超时/解压炸弹上限 |
| nit | 端口漂移 | §4.4：`9091` 定死 |
| 好消息 | multimodal 基建已有 | §2.5、§4.5、§6 Phase 2：接线为主 |
