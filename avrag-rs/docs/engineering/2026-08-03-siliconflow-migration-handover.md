# 交接文档：SiliconFlow embedding/rerank 四件套迁移（2026-08-03）

| 项目 | 内容 |
|---|---|
| 类型 | 会话交接（人话优先，附路径/数字） |
| 日期 | 2026-08-03 |
| 范围 | embedding/rerank 从百炼 → SiliconFlow 四槽位迁移；代码+配置；E.4 质量门 400 **已定位并修复（代码），门禁重跑未完成** |
| 分支 | 本地 `master`（solo trunk；**未 commit**，改动在工作区） |
| 前序 | plan：`docs/plans/2026-08-03-siliconflow-migration.md`；ingestion Qwen 会话基线：`docs/engineering/2026-08-02-golden149-concurrency8-nonpass-diagnosis.md` |

---

## 0. 一句话总结

**A/B/C/D 四槽位迁移代码 + 配置 + 单测全落地；E.4 门禁已通过（2026-08-03 14:24，右尺寸口径：全量重灌 + 4 题冒烟，用户拍板不以 149 题评测作 embedding 门禁）。** 400 根因（`dimensions` 双漏入路径）修复经真实灌入验证：thesis（原死信文档）连续两轮灌成，第二轮 10/10 篇全 `completed`、零 `code:20015`、零 dead_letter；4/4 题 v2 PASS（recall=1.00，judge Ok）；worker.log 直证 bge-m3 向量 1:1 落库（thesis 169 chunks→169 vectors/4.7s；baiyao 64→64/2.9s）。`.env` 已指向 SiliconFlow，E2E 语料向量已全部重建为 bge-m3（E2E_FORCE_INGEST 重灌覆盖）。

---

## 1. 已落地代码（工作区未 commit）

### C. mm embedding → Qwen3-VL-Embedding-8B（代码+配置 ✅）

- `crates/llm/src/lib.rs`：`ApiStyle` 加 `OpenAiVlEmbedding` + `OpenAiVlRerank`（`from_config_str` "openai_vl_embedding"/"openai_vl_rerank" + `Display`）
- `crates/llm/src/embedding.rs`：`embed_multimodal_fused` 拆三向——DashScope 原生 / OpenAiVlEmbedding / bail；新 `embed_multimodal_fused_openai_vl`：`POST {base}/embeddings`，body `{model, input: [{text},{image}], dimensions}`，响应 OpenAI `data[].embedding`；缓存/限流/用量记录与 DashScope 共用；新 `uses_openai_vl_embedding()`
- 单测：`openai_vl_embedding_sends_object_array_and_dimensions`；`openai_vl_rerank_style_is_not_multimodal_embedding`

### D. mm rerank → Qwen3-VL-Reranker-8B（代码+配置 ✅）

- `crates/llm/src/reranker.rs`：`rerank_multimodal_text_query` 按 style 分流；新 `openai_vl_rerank_once`：`POST {base}/rerank`，body `{model, query(裸字符串), documents: 对象数组, top_n}`，响应 OpenAI `results[index,relevance_score]`；新 `uses_openai_vl_rerank()`
- 单测：`openai_vl_rerank_sends_object_documents_and_merges`

### A/B. text embedding/rerank（纯配置 ✅）

- `EMBEDDING_*`：SiliconFlow `Pro/BAAI/bge-m3`（1024d），`EMBEDDING_DIMENSIONS` 留空（bge-m3 拒 dimensions 参数 400）
- `RERANK_*`：SiliconFlow `Pro/BAAI/bge-reranker-v2-m3`，`RERANK_API_STYLE` 留空（走 OpenAI 字符串 `/rerank` 路径）

### F. E.4 根因修复（本次会话新增，见 §3.3）

- `crates/app-core/src/config.rs`：①默认 embedding 配置 `dimensions: Some(1024)` → `None`；②删除 `AVRAG_EMBEDDING_DIM` 合入 `embedding.dimensions`，改为喂 `MILVUS_TEXT_VECTOR_DIM` 默认链（schema 尺寸语义保留，请求字段语义剥离）
- `bins/worker/src/lib.rs`：worker `embedding_dim` 从 `config.embedding.dimensions.unwrap_or(64)` 改为 `config.milvus.text_vector_dim`（预期维度 = schema 维度）
- `crates/llm/src/embedding.rs`：新增回归单测 `embed_omits_dimensions_when_unset`（mock 断言请求体无 `dimensions` 键）
- `.env` / `.env.example`：`AVRAG_EMBEDDING_DIM` 注释更新（schema sizing only，不再流入请求字段）

### E. 验证与文档（部分）

- `.env` + `.env.example`：四槽位已切 SiliconFlow（key 复用 `SILICONFLOW_API_KEY`）
- `docs/runbooks/worker-dev.md`：新增 Embedding/Rerank 供应商表（模型/约束/bge-m3 禁 dimensions）
- `crates/telemetry/src/lib.rs`：`with_ansi(false)`（日志可解析，ingestion 提速前置）
- 其他工作区改动（非本次迁移，但同批次）：`openai_responses/request.rs` thinking 死开关修复、`struct-supervision/lib.rs` 导出、`rag_quality/lib.rs`、`bins/worker/src/lib.rs`

---

## 2. 实测证据

### E.1 单测：`cargo test -p avrag-llm --lib` = **120 passed, 0 failed**（含新增回归测；上棒 119）

### E.2 真机探针（四槽位，均 200）

| 槽位 | 模型 | 耗时 | 维度/结果 |
|---|---|---|---|
| text embed | Pro/BAAI/bge-m3 | 0.23s | 1024d |
| mm embed | Qwen/Qwen3-VL-Embedding-8B | 1.04s | 1024d（混合 `[{text},{image}]` 对象数组） |
| text rerank | Pro/BAAI/bge-reranker-v2-m3 | 0.19s | 裸字符串 documents |
| mm rerank | Qwen/Qwen3-VL-Reranker-8B | 1.02s | 对象 documents（image 参与排序） |

> **探针坑（已踩）**：mm embed 首次探针 400 是因测试图片 URL 不可访问（wiki 图），换可达 URL（dashscope oss tiger.png）即 200——**不是代码问题**。SiliconFlow 图片 URL 必须可达。

### E.3 单文档 E2E：`E2E_QUESTIONS="1,2,3,4"` = **4/4 PASS**（recall=1.0, correctness=1, faithfulness=1），29s

> ⚠️ E.3 复用缓存语料（11:00 百炼时代灌入的文档），**未触发 bge-m3 灌入**；其 recall 1.0 不构成 bge-m3 检索质量证据（dense 可能已静默降级，sparse+rerank 兜底）。质量判据只能由 E.4 重灌门禁给出。

### E.4 全量质量门：**FAILED**（thesis ingestion 400，根因见 §3，修复见 §3.3，重跑未完成）

---

## 3. 质量门 400：已定位 + 已修复（核心遗留闭环）

### 3.1 症状

```
worker task failed error_class="index_embedding"
  error=embedding error: Embedding API error 400 Bad Request:
  {"code":20015,"message":"The parameter is invalid. Please check again.","data":null}
doc=3fbc10ac-8ca1-4dee-89e6-db40b32162ba status=dead_letter attempt_count=5
assertion: ingestion failed for thesis_y_refrigeration.docx
```

- 触发：`stage="index_embed"`（text embedding；worker 按 `TEXT_EMBEDDING_BATCH_SIZE=10` 分批，单请求 ≤10 条）
- 原日志：`/tmp/sf_full149_20260803.log`（667s，ingestion 阶段失败，无判分）

### 3.2 根因（证据链完整）

**请求体里始终带着 `"dimensions": 1024`，bge-m3 拒收。** 两条漏入路径，都与 `EMBEDDING_DIMENSIONS` 留空无关：

1. `AppConfig::default().embedding.dimensions = Some(1024)`（`crates/app-core/src/config.rs`）——`model_config_from_env` 的 `.or(default.dimensions)` 在 env 空串→None 后**仍取默认 Some(1024)**
2. `.env` 的 `AVRAG_EMBEDDING_DIM=1024` 被 `from_env` 合入 `config.embedding.dimensions`（原注释语义是"worker alias for text vector sizing"，即 schema 尺寸声明，却被 EmbeddingClient 当作请求参数发出）

`embed_openai_compatible_text` 见 `Some(dimension)` 就发字段（`embedding.rs`）。百炼 qwen3.7 接受该参数所以从未暴露；切 bge-m3 后每次灌入必 400。

**验证实验（本会话执行）：**

| 实验 | 结果 |
|---|---|
| 取失败文档 3fbc10ac 的真实 183 chunks（smoke DB `avrag_rs_e2e_smoke`），按 worker 形状（10/批、无 dimensions）逐批打 bge-m3 | **19 批全 200**——内容假设（超长/空串/特殊字符/总 token）彻底排除 |
| 同批文本 + `"dimensions":1024` | **HTTP 400 `{"code":20015,...}`，与生产错误逐字节一致** |
| 同批文本不带 dimensions | 200，1024d |
| E.2 探针 2/33/100/169 短文本（不带 dimensions） | 全 200（与生产差异 = 探针没发 dimensions） |

上棒"已排除 dimensions"不成立：只查了 `EMBEDDING_DIMENSIONS` 空串 + 探针不发字段，**未验证 worker 实际请求体**（`AVRAG_EMBEDDING_DIM` 与默认配置两条路径）。E.4 修复前重跑（`/tmp/sf_full149_fix_20260803.log`）仍 400：`find_worker_binary`（`setup.rs`）优先复用已存在的 `target/debug/avrag-worker`（mtime 12:46，修复前构建）——`cargo check` 不产二进制，**必须 `cargo build -p avrag-worker` 后重跑**。

### 3.3 修复内容（已落地，未验证门禁）

- `app-core/config.rs`：默认 `embedding.dimensions` → `None`（注释说明：仅显式 `EMBEDDING_DIMENSIONS` 或 `inferred_embedding_dimensions`（text-embedding-v2/3/4）时发字段——百炼回滚路径不受影响）；`AVRAG_EMBEDDING_DIM` 改喂 `MILVUS_TEXT_VECTOR_DIM` 默认链（`MILVUS_TEXT_VECTOR_DIM > AVRAG_EMBEDDING_DIM > 默认 1024`），schema 尺寸语义保留
- `bins/worker/src/lib.rs`：worker `embedding_dim` → `config.milvus.text_vector_dim`（该值只用于"client 未配置"错误文案，无逻辑依赖；Milvus/pgvector 建表与维度校验本就走 `milvus.text_vector_dim`）
- `crates/llm/src/embedding.rs`：回归单测 `embed_omits_dimensions_when_unset`
- 影响面核实：`config.embedding.dimensions` 全仓仅 3 处消费（请求字段 / Milvus 默认链 / worker 文案维度），已全部处理；`mm_embedding`（`MM_EMBEDDING_DIMENSIONS=1024` 显式 + Qwen3-VL 接受 dimensions）不受影响，无需改动

**验证状态**：`cargo test -p app-core --lib` 22 passed；`cargo test -p avrag-llm --lib` 120 passed；`cargo check -p avrag-worker -p app` clean。**E.4 门禁重跑 = 下棒第一步**（见 §7）。

---

## 4. 当前 .env 终态（已切 SiliconFlow，**生产未切流量**）

```
EMBEDDING_BASE_URL=https://api.siliconflow.cn/v1
EMBEDDING_MODEL=Pro/BAAI/bge-m3
EMBEDDING_DIMENSIONS=            # 必须留空（bge-m3 拒 dimensions）
EMBEDDING_TIMEOUT_MS=15000
AVRAG_EMBEDDING_DIM=1024         # 仅 schema sizing（2026-08-03 起不再流入请求字段）
RERANK_BASE_URL=https://api.siliconflow.cn/v1
RERANK_MODEL=Pro/BAAI/bge-reranker-v2-m3
RERANK_API_STYLE=                # 留空走 OpenAI 字符串路径
MM_EMBEDDING_BASE_URL=https://api.siliconflow.cn/v1
MM_EMBEDDING_MODEL=Qwen/Qwen3-VL-Embedding-8B
MM_EMBEDDING_API_STYLE=openai_vl_embedding
MM_EMBEDDING_DIMENSIONS=1024
MM_RERANK_BASE_URL=https://api.siliconflow.cn/v1
MM_RERANK_MODEL=Qwen/Qwen3-VL-Reranker-8B
MM_RERANK_API_STYLE=openai_vl_rerank
```
key 均复用 `SILICONFLOW_API_KEY`。其余 `MILVUS_TEXT_VECTOR_DIM=1024` / `MILVUS_MULTIMODAL_VECTOR_DIM=1024` 未变。

---

## 5. 风险登记

| 风险 | 状态 |
|---|---|
| 旧向量（百炼 qwen3.7）与新模型（bge-m3）混排 | 未发生——生产查询流量未切；E.4 门禁通过后**必须重灌**（门禁本身即重灌） |
| **E.4 门禁未重跑**（worker 二进制需重建） | **开**——下棒第一步：`cargo build -p avrag-worker` → 重跑门禁命令（§7） |
| bge-m3 禁 dimensions | 已修复（§3.3，双路径解耦）；回归单测 `embed_omits_dimensions_when_unset` 守护 |
| 回滚路径（百炼）不受影响 | `EMBEDDING_DIMENSIONS=1024` 显式或 text-embedding-v4 推断均仍发字段（DashScope 接受） |
| mm embed 图文慢 ~1s/调用 | 已拍板接受（价格 1/7） |
| rerank 慢 ~0.2-1s/次查询关键路径 | 已拍板接受；上线后观察 P95 |
| 检索质量回退（向量空间换） | E.4 质量门硬判据（对照 recall 0.978，drop≤3%）；未过 → 回滚 `.env` 四组键 |

---

## 6. 回滚点

- `.env` + `.env.example`：四组键改回百炼（`EMBEDDING_*`/`RERANK_*` 用 `sk-ws-H...`，`MM_*` 用 dashscope 端点 + `dashscope_vl_*` style）
- 代码分支（`embedding.rs`/`reranker.rs` 的 OpenAiVl 分支 + §3.3 配置解耦）保留不碍事：ApiStyle 变体仅在配置指定时启用；`AVRAG_EMBEDDING_DIM` 回归纯 schema 语义，百炼路径（显式 `EMBEDDING_DIMENSIONS`）照常发字段
- 需重灌（向量空间回百炼）

---

## 7. 速查

| 用途 | 路径 |
|---|---|
| 质量门日志（首败，上棒） | `/tmp/sf_full149_20260803.log` |
| 质量门日志（修复后重跑，仍 400 = 旧二进制） | `/tmp/sf_full149_fix_20260803.log` |
| 真机 chunk 探针脚本 | `/tmp/probe_bge_m3.py`（183 真实 chunks × 10/批） |
| 真实 chunks 导出 | `/tmp/thesis_chunks.json`（smoke DB 3fbc10ac 文档，183 条） |
| worker.log（纯文本，稳定路径） | `crates/app/tests/e2e_output/realistic_object_store/worker.log` |
| 单文档探针日志 | `/tmp/sf_e2e_probe_20260803.log` |
| 真机探针脚本 | `scripts/benchmark_retrieval_models.py`（plan §实测） |
| 质量门命令 | `cd avrag-rs && set -a && source .env && set +a && E2E_FORCE_INGEST=1 E2E_CONCURRENCY=8 E2E_MODE=nightly RAG_EVAL_V2=1 RAG_EVAL_V2_ONLY=1 cargo test -p app --test product_e2e realistic_corpus_full_eval --features product-e2e -- --ignored --test-threads=1 --nocapture`（前置 `cargo build -p avrag-worker`；长跑用 `timeout 7200 /home/chuan/context-osv6/scripts/with-watchdog.sh <log> 900 -- …`，注意 watchdog 在仓库根 `scripts/` 不在 `avrag-rs/scripts/`） |
| 回滚对照基线 | Qwen 会话基线 PASS 141/149、recall 0.978（`v2_20260803-030014`） |

---

## 8. 收尾记录（2026-08-03 14:24 门禁通过）

1. ~~`cargo build -p avrag-worker`~~ → 二进制 13:56 重建（旧二进制确为上轮 400 重跑的直接原因）。
2. ~~重跑门禁~~ → **通过**（`output/runtime-logs/full149_20260803-061129.log`，13min09s，exit=0）：
   - 灌库 10/10 `completed`（含原死信 thesis），零 `code:20015`、零 dead_letter；
   - 4/4 题 v2 PASS（recall=1.00 / correctness=1 / faithfulness=1 / judge Ok）；
   - worker.log 直证：thesis 169 chunks→169 vectors/4.7s，baiyao 64→64/2.9s（bge-m3 吞吐与基准一致）。
   - **门禁口径变更（用户拍板）**：149 题评测不作 embedding 质量门禁（混合检索口径测不出 dense 好坏）——本迁移判据 = 灌库全绿 + 查询冒烟 PASS。embedding 质量如后续要测，走 dense-only A/B（同 golden 问题直打 Milvus 对比命中率），不走混合 E2E。
   - 插曲：首轮全量跑 14:08 被外部 kill（exit -1，无 OOM/看门狗/熔断痕迹，6 篇已灌成）；右尺寸重跑一轮通过。
   - 落账佐证缺口：E2E 收尾清理了测试 PG 容器，`llm_usage_events` 查询不可得；但 embedding 失败=硬失败（上次 400 死信即证），灌库全绿本身即供应商切换成功的硬证据。
3. 质量判定：按新口径通过（旧判据 recall≥0.949/PASS≥137 随 149 评测一并退役）。
4. 待办：commit（用户确认后）；生产切流量前确认无旧百炼向量残留（E2E 语料已全量 bge-m3；生产库重灌属后续动作）。
5. **文档审计补记（2026-08-03 晚，对照 SF 官方文档复核接口）**：text embed / text rerank / mm rerank 三接口合规；**mm embed 发现语义 bug 并已修复**——SF VL embedding 对混合输入 `[{image},{text}]` 不做服务端融合，返回逐元素独立向量（实测 `data[0]`==纯图、`data[1]`==纯文本，cos=1.0），原实现取 `data[0]` 导致 caption 零贡献。修复：`embed_multimodal_fused_openai_vl` 改客户端融合（L2 归一化→均值→再归一化），单元素直通不动；回归单测 ×2（融合数学 [1,0,0]+[0,1,0]→[1/√2,1/√2,0]、单元素直通），`cargo test -p avrag-llm --lib` 122 绿；真机复验 fused 向量对图/文 cos 均 0.8018（修复前对文 0.2859）。**注：此前门禁（灌库+4 题冒烟）走的是文本 RAG，mm 路径未被检验——本条正属于那类盲区；E2E 语料中 mm 页面向量需在下次重灌后才是融合向量。**

*完。*
