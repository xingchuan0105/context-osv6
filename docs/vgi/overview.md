---
module: overview
provides: intent, non-goals, milestone map
depends_on: []
---

# Overview

VGI-RAG 是固定大文档库上的**评测工具**，不是工作区搜索产品。Rust 本机工具：索引 BrowseComp-Plus 级语料；循环里提供**排序 hybrid**；结果紧凑带出处。循环属于宿主 agent。zg 是「循环里要有排序检索 + 紧凑出处」的参照，不是产品形态。

## 目标

1. 单机（Windows 优先）索引约 10 万文档。
2. 循环内排序检索（向量 + 词法，文档级融合）+ 原文阅读。
3. 索引级指标先于 agent 级 A/B。
4. 规格在 `docs/vgi/`，代码在独立 `vgi-rs`。

## 非目标（可执行裁剪）

本机评测工具。默认路径不接产品 SAC 栈、不引入向量数据库服务器、不复刻 zg 代码、不做 Web UI。Milvus / `vgi-store` / 代码模式沙箱 / `vgi install` 不在 M0–M4 默认树；见 [deferred](deferred.md)。

## 评审决议

对象：[zg 对齐稿](../plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md) 的 [评审](../reviews/2026-09-13-vgi-rag-rust-zg-aligned-design-review.md)。

| Issue | 决议 |
|---|---|
| 1 指纹/schema | [corpus](corpus.md) 给出算法、URI、DDL、anchor |
| 2 非目标矛盾 | 本页裁剪；云/SAC/install 进 deferred |
| 3 12 crate | M0 只从 corpus + token-batch + zvec-spike 生成 |
| 4 对齐 zg 过声称 | 上文：参照不是形态；默认工具见 deferred |
| 5 RRF/rg/chunk | [retrieval](retrieval.md)：融合身份 `doc_id`；read 为 char 窗；rg 只扫已 scope 的正文 |
| 6 M1 官方数字 | [eval](eval.md)：绝对 recall；官方数字不是 bge-m3 及格线 |
| 7 自研倒排 | deferred：先官方 BM25 oracle，默认嵌入式库 |
| 8 M3 根覆盖 | deferred：覆盖只诊断；门是 route recall@k |
| 9 代码模式预算 | deferred：默认不做；若做，桥接调用计入 tool call |
| 10 fd vs TCP | deferred：Windows 为 TCP loopback |
| 11 字段 boost | deferred：M2 先测 heading/url 非空与是否含答案 |
| 12 嵌入费用 | M1 前用 token-batch 总量估价 |
| 13 JSON schema | M4 前锁；M0 不实现工具面 |
| 14 100,194/100,195 | 期望 `n_docs = 100195`；不符则 ingest 失败，改规格后再跑 |

## 五件已定（修订后）

1. **本机向量**：zvec（M0 spike）。不达标换 `hnsw_rs`，上层接口不变。无云第二实现。
2. **落点**：独立 `vgi-rs`。
3. **passage**：M2 再切；起点 512 token / 64 重叠 + 文档 max。不挡 M0。
4. **install**：不做，直到 M5（deferred）。
5. **embedding profile**：默认 bge-m3；qwen 对照在 M4 以后。

## Worked example — 一次 M0 再生

输入：本 DAG + BrowseComp-Plus parquet（revision 见 corpus）。

1. 读 README，范围 = corpus, token-batch, zvec-spike。
2. 三个子代理各持一份文档，写入 `../vgi-rs`。
3. `vgi ingest` 两次，`corpus_fp` 相等，`n_docs=100195`。
4. `vgi tokens` 填满 `token_count`。
5. `vgi spike-zvec` 写出报告。
6. `python3 docs/vgi/anchors/test_anchors.py` 绿。

**Expected:** 上列六步都发生；`deferred.md` 没有对应 crate。
