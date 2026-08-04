# 关 MM 检索 + VLM 插图文本化（2026-08-04）

| 项目 | 内容 |
|---|---|
| 状态 | **已拍板，开发中** |
| 日期 | 2026-08-04 |
| 动机 | SiliconFlow `Qwen3-VL-Reranker-8B` 日耗 ~1e8 tokens；字多图少业务下默认 VL ANN/rerank ROI 极低 |
| 前序 | 会话结论（关 MM / VLM 描述主路径 / 扫描 Paddle / 前端直出图）；旧稿 `ingestion-routing-discussion` ING-3 仅部分重合 |
| 前端 | `citation-renderer` 已支持 `[[image:chunk_id]]` **直接 `<img>` 渲染**（不依赖点击） |

---

## 0. 一句话

**查询侧关闭多模态向量与 VL 重排；插图在 ingestion 用 `INGESTION_LLM`（qwen3.7-flash）做可检索文字描述，描述进 text 空间，chunk 绑 `asset_id`；扫描件继续 Paddle OCR；命中后答案区直接出图。**

---

## 1. 决策表（权威）

| # | 议题 | 决定 |
|---|------|------|
| D1 | 查询 MM Embedding | **默认关**（不配 / 不挂 `mm_embedding_client`） |
| D2 | 查询 VL Rerank | **默认关**；dense 精排只用 `RERANK_*`（bge） |
| D3 | 何时仍可用 mm_rerank | **仅**候选池存在 `image_path`/`asset_id` 且显式配置时（可选后置；P0 可完全关） |
| D4 | 插图检索 | VLM **描述文本** → **text embed** 召回（与正文同空间） |
| D5 | 插图存储 | 保留 `document_assets` + `multimodal_chunks`（或等价）行：`chunk_id` ↔ `asset_id` ↔ path |
| D6 | 扫描整页 | **Paddle OCR** → 正文 text chunks（**不**对已 OCR 页狂打 VLM） |
| D7 | 装饰图 | 跳过 VLM（过小 / 无语义） |
| D8 | 答案出图 | 答案 `[[image:chunk_id]]` / `AnswerBlock.image` + citation 带 `image_url` 或 `asset_id` → **前端直接渲染** |
| D9 | 成本可接受后 | 可再开 MM ANN（条件触发），**不**恢复「每次 dense 必 VL rerank」 |

---

## 2. 目标流水线

```
解析
  ├─ 可提取正文 → text chunks → text embed
  ├─ 扫描/近空 PDF → Paddle OCR → text chunks → text embed
  └─ 非装饰 figure 资产
        → INGESTION_LLM 真多模态读图
        → 检索向描述 (2–6 句，实体/关系/数字)
        → 写 multimodal_chunks.context_text + asset_id
        → **双写 text 索引**（同 chunk_id，content=描述）→ text embed
        → **不写** mm dense（P0）

查询 (dense)
  → text ANN (+ lexical/VGRAG)
  → text rerank (bge)
  → citation：text 命中若 chunk 在 multimodal 表有 asset → 填 asset_id/image_url
  → 答案区直接出图
```

---

## 3. 实现切片

### P0（本波必做）

1. **配置**  
   - `.env` / `.env.example`：清空或注释 `MM_RERANK_*`；`INGESTION_VLM_SUMMARY_ENABLED=1`  
   - `MM_EMBEDDING_*` 可留注释说明「仅未来条件开」；worker 无 client 时已 skip mm index（现状）

2. **Rerank 路由**（`rerank_item_chunks`）  
   - **有 `RERANK` 时优先 text rerank**  
   - 仅当池中存在 image 且配置了 `mm_reranker` 时才走 VL（P0 也可直接永不优先 mm）

3. **VLM 描述**（`indexing/vlm_summary.rs`）  
   - 真多模态：`ChatMessage::user_multimodal(prompt, image_urls)`  
   - 覆盖 figure 类 chunk（不限 `page_raster`；OCR 成功页的 raster 仍跳过）  
   - 描述写回 **内存 + PG** `context_text`（修「只改内存未落库」）

4. **双写 text 索引**  
   - VLM 成功后的 mm chunk：额外 `TextChunkIndexRecord{ chunk_id, content=context_text, chunk_type=figure_desc, … }`  
   - 与 body text 一并 embed 进 text collection

5. **检索 hydrate**  
   - text 命中后：按 `chunk_id` 回填 `asset_id` / `image_path`（PG multimodal 或 content store）  
   - 保证 `Citation.image_url` / `asset_id` 非空 → 前端直出图

6. **文档**  
   - 本文件；`worker-dev.md` 补「默认关 MM 查询」一行

### P1（随后）

- figure vs 装饰图启发式  
- 描述过短 → 图块 Paddle OCR 兜底  
- skill 提示：命中 figure 描述时可 `[[image:chunk_id]]`  
- 计量：VLM summary 记 `llm_usage_events`

### 非目标（本波）

- 以图搜图 / VL ANN 产品化  
- ColPali  
- 改 VGRAG 池预算（另议）

---

## 4. 验收

| # | 门 |
|---|-----|
| A | 配置关 MM 后，dense 路径日志/degrade **无** mm_reranker 成功调用 |
| B | 含插图文档灌库：`multimodal_chunks.context_text` 为非空 VLM 描述；text 索引含同 `chunk_id` |
| C | 问与图语义相关的问题：text dense 能召回 figure_desc；返回 citation 含 `asset_id` 或可解析 `image_url` |
| D | 聊天答案含 `[[image:{chunk_id}]]` 时前端 **直接显示图片**（不依赖点击） |
| E | 纯扫描 PDF 仍走 Paddle，不因 VLM 整本失败 |

---

## 5. 回滚

- 恢复 `.env` `MM_RERANK_*` / `MM_EMBEDDING_*`  
- `INGESTION_VLM_SUMMARY_ENABLED=0`  
- 代码：rerank 优先序可再切；双写 text 无害可保留

---

## 6. 与旧文档关系

| 文档 | 关系 |
|------|------|
| `ingestion-routing-discussion` ING-3 B 类 VLM-first | **吸收**：VLM→figure text；**删除**「辅助 mm 向量」作为默认 |
| SiliconFlow migration handover | 四槽位迁移保留能力；**产品默认不再热开 MM 查询** |
| 本文件 | **现行权威**（查询 MM 策略 + 插图文本化） |
