---
module: embed
provides: uid, embed_windows, window_vectors
depends_on: [corpus, token-batch]
---

# Embed (M1)

文档级向量：每个 token 窗一条 1024 维 fp16，uid = `doc_id#batch`。查询时按文档 **max** 聚合（见 [retrieval](retrieval.md)）。

## Interface

    uid(doc_id, batch: uint) -> str      # "a#0"
    parse_uid(uid) -> (doc_id, batch)    # 非法则失败

    Window = {uid, doc_id, batch, text}
    windows_for_doc(doc, tokenizer) -> [Window]   # 空窗 [0,0) 不产出

    embed_texts(texts: [str]) -> [[f32; 1024]]
    # HTTP OpenAI-compatible POST {base}/embeddings
    # body: {model, input}  —— 禁止 dimensions 字段
    # model 默认 Pro/BAAI/bge-m3；凭据读 avrag-rs/.env 的 EMBEDDING_*（不打印 key）

    checkpoint table window_vectors(uid PK, doc_id, batch, dim, fp16 BLOB)
    resume: 已有 uid 跳过

测试后端 `fake`：SHA-256 混入 1024 维，再加字符 bag，L2 归一化；相同字符串 cosine=1。不走网络。生产默认 HTTP。

## Semantics

窗文本：`encode(body, add_special_tokens=false)`，切 `ids[start:end)`，`decode`。与 [token-batch](token-batch.md) 同一 tokenizer.json。

查询侧：整段问题一条向量；Challenge-20 题远小于 LIMIT，不切窗。

批次：最多 10 条/请求（与产线 worker 一致）。不发送 `dimensions`（bge-m3 会 400）。超时、429/5xx 可重试。TPM/RPM 按 `.env` 令牌桶。

开始全库 HTTP 前必须打印：`n_windows_pending`、`sum_window_tokens`（含重叠），等确认口径；M1 授权后可直接跑。断点续跑。

内存：按篇流式。只持有当前 `body`、tokenizer 和最多 `EMBED_BATCH` 条窗文本。禁止 `SELECT body` 一次性装进 `Vec`。`window_vectors.uid` 集合可以全量放内存（17 万条字符串，MB 级）。文档级 fp16 向量全库约 0.36 GB，不是多 GB。

zvec 索引：`.vgi/zvec-docs/`，字段 `uid`（pk）、`doc_id`、`batch`、`embedding` VectorFp16/1024/HNSW cosine。已有目录用 `open`，不要 `create_and_open`。`window_vectors` 是源；zvec 可由表重建。

## Profile `qwen-flash`（第二套索引，不覆盖 bge-m3）

百炼 `qwen3.7-text-embedding-flash`：`LIMIT=120000`（API 上下文 131072，HF tokenizer 会少计约 3%，128000 会 400）、`OVERLAP=256`、`dim=256` fp16、请求体**必须**带 `dimensions=256`。凭据：`DASHSCOPE_API_KEY` + `E2E_EMBEDDING_BASE_URL`。灌库走 **Batch File**（半价），不走同步 TPM。

- tokenizer：`Qwen/Qwen3-8B` tokenizer.json（与 bge 的 XLM-R 分开，token 数写入 `doc_tokens`）
- 表：`window_vectors_qwen_flash`
- 索引：`.vgi/zvec-docs-qwen-flash/`
- CLI：`vgi tokens --profile qwen-flash` 然后 `vgi embed --profile qwen-flash`
- 限流（百炼实测配额）：RPM 24000、TPM 1e6。瓶颈是 TPM。默认 **8 路 HTTP 并发**（`tpm / 128000`），漏桶按秒补充；可用 `QWEN_EMBED_CONCURRENCY` 覆盖。单请求仍 ≤20 行且 ≤128k token。

bge-m3 的 `window_vectors` / `zvec-docs` 不动。

## Worked example — uid

1. `uid("a", 0)` = `a#0`
2. `uid("79680", 12)` = `79680#12`
3. `parse_uid("a#0")` = `("a", 0)`
4. `parse_uid("a")`、`parse_uid("a#")`、`parse_uid("#0")`、`parse_uid("a#x")` 失败

## Anchor

| input | expected |
|---|---|
| uid a,0 | `a#0` |
| parse `79680#12` | `79680`, 12 |
| parse `a` / `a#` / `#0` / `a#x` | error |

**Enforced by:** `docs/vgi/anchors/test_anchors.py::test_uid`
