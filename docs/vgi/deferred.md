---
module: deferred
provides: (none in default DAG)
depends_on: []
---

# Deferred

默认不生成。写在这里是为了改口时只改一处，而不是让 M0 骨架长出空 crate。

## 词法（原 M2 自研倒排）

先在同一 Challenge-20 题集上跑官方预构建 BM25 当 oracle。默认引擎是**进程内库**（优先 tantivy 或已标定可用的 zvec FTS）；自研 postings 只有在字段 boost / BM25L 参数被测量为必要时才开。字段 boost 之前先统计 heading/url 非空率、金标答案是否出现在这些字段。

## 语义树 v2

在文档向量存在之后。形状门（叶容量 p50≥20、depth≤4）+ 标签噪声门 + **route(query) 的文档 recall@k**。19/20 题证据跨主题是硬划分的几何事实：最小覆盖节点不是验收门。Challenge-20 上不指望单节点收窄。

## 工具面

默认两个：`vgi_search`（hybrid，文档级）、`vgi_read`。`vgi_tree` / `vgi_rg` 不是默认 MCP 面；rg 原语按 [retrieval](retrieval.md) 必须带 `doc_ids`。JSON schema 在 M4 实现前锁一页。

zg 的「单一 search + 宿主原生 rg」是文件工作区形态；VGI 语料是 parquet 行。

## 代码模式

默认不做。若做：Windows 用 loopback TCP + 单次 token（本仓 `code-interpreter` 已如此），Unix 才是 fd3/4；独立 `vgi-rs` 重写，不链产品 crate。每次桥接 `search`/`rg`/`read`/`tree` 计一次 tool call，代码执行计 round。在此之前不写「同预算 A/B」。

## 云 / install

Milvus、`vgi-store`、无状态多副本、daemon「模型池」、`vgi install` 都不在 M0–M4。本机 zvec 不够用时再开一页规格，而不是预先做第二实现。
