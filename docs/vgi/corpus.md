---
module: corpus
provides: Document, ingest(parquet) -> .vgi/corpus.sqlite, corpus_fp
depends_on: []
---

# Corpus

## Interface

    Document = {doc_id: str, body: str, heading: str, url: str}
    ingest(corpus_uri, revision, expected_n=100195) -> Manifest
    corpus_fp(revision, documents) -> hex SHA-256

`doc_id` 是 BrowseComp-Plus 官方文档 ID，与 evidence / qrels 同一字符串。

`corpus_uri` 指向 parquet 分片目录（操作员给定；Windows 原型仓不是唯一路径）。

锁定 revision：`b27b02bc3e45511b8b82a13e6f90ce761df726f6`。

锁定 `expected_n = 100195`。行数不等则 ingest **失败**并打印实测 N——改本页数字后再跑，不静默接受。

## Semantics

**列映射。** 逻辑字段 `doc_id` / `body` / `heading` / `url`。候选名：`id|docid|doc_id`，`text|contents|body|document`，`heading|title|name`，`url|locators`。第一次成功 ingest 把实际列名写入 `manifest.parquet_columns`。缺 `doc_id` 或 `body` 则失败。`heading`/`url` 缺则写空串。不把数字 ID 当成标题线索。

**DDL（M0 只有 documents）：**

```sql
CREATE TABLE documents (
  doc_id      TEXT PRIMARY KEY,
  body        TEXT NOT NULL,
  heading     TEXT NOT NULL DEFAULT '',
  url         TEXT NOT NULL DEFAULT '',
  char_len    INTEGER NOT NULL,
  body_sha256 TEXT NOT NULL,
  token_count INTEGER
);
```

`token_count` 在 token-batch pass 之前为 NULL。`passages` 表不在 M0。

**指纹。** UTF-8。`body_sha256 = SHA-256(body)`。每行：

    doc_id || NUL || body_sha256 || NUL || heading || NUL || url

按 `doc_id` 的 UTF-8 字节序排序。payload：

    revision || LF || join(rows, LF)

`corpus_fp = SHA-256(payload)` 的小写 hex。不含 tokenizer、token_count、文件系统路径。

**manifest.json**

    schema_version, corpus_revision, n_docs, corpus_fp, parquet_columns, tokenizer (M0 填 token-batch 后)

数据目录默认 `<VGI_ROOT>/.vgi/`。

## Worked example — two-doc corpus

Input:

    revision = "rev0"
    A: doc_id="a", body="hello", heading="", url=""
    B: doc_id="b", body="world", heading="W", url="http://x"

Trace:

1. `SHA-256("hello")` = `2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824`
2. `SHA-256("world")` = `486ea46224d1bb4fb680f34f7c9ad96a8f24ec88be73ea8e5a6c65260e9cb8a7`
3. 行 a：`a \0 <sha_hello> \0 \0`（heading 与 url 皆空，两个连续 NUL）
4. 行 b：`b \0 <sha_world> \0 W \0 http://x`
5. 排序后仍是 a 然后 b。
6. payload = `rev0\n` + 行 a + `\n` + 行 b
7. `corpus_fp` = `5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2`

若先写 b 再写 a 而不排序，指纹变成 `eab63d9148e8702392dc1176a1db6c957c06e4a6a7b813518c9fd1a2297aeb2e`。排序是契约。

## Anchor

**Input:** 上例两篇 + `rev0`。  
**Expected:** `corpus_fp = 5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2`；未排序 ≠ 此值。  
**Enforced by:** `docs/vgi/anchors/test_anchors.py::test_corpus_fp_two_doc`

M0 全库门（非此文件的微型 anchor）：连续两次 ingest，`n_docs=100195` 且 `corpus_fp` 相同。
