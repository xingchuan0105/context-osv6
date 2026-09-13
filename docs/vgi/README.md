# VGI-RAG design-doc DAG

日期：2026-09-14。现行规格。取代 [zg 对齐设计稿](../plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md)（评审：[needs revision](../reviews/2026-09-13-vgi-rag-rust-zg-aligned-design-review.md)）。

本目录是 VGI 的耐久产物：一组自包含设计文档，边由 `depends_on` / `provides` 组成 DAG。实现从文档再生，不在旧代码上打补丁。过程见 [AGENTS.md](AGENTS.md)。

## 范式缺口（本仓库）

| 层 | 状态 |
|---|---|
| 用户 skill `~/.agents/skills/spec-regeneration` | 有（worked example、anchor、DAG、`G(spec)`） |
| 产品 `AGENTS.md` / `docs/agent/` | 无此范式；只有通用设计原则与产品边界 |
| VGI | **本目录**把范式收进项目范围；根 `AGENTS.md` 增加一行路由 |

SMART 的符号 IR / TPU 调度与 VGI 无关。VGI 取用的是：文档为源、worked example 钉语义、anchor 可独立验、按文档派生子代理。

## DAG

```
overview          （意图与门，不生成代码）
   │
   ├─ corpus ───────────────┐
   │     provides: Document, corpus_fp, ingest
   │
   ├─ token-batch ──────────┤
   │     depends_on: corpus
   │     provides: BatchWindow[]
   │
   ├─ zvec-spike            │   M0 实现
   │     provides: spike report
   │
   ├─ retrieval ────────────┤   M1 起实现；M0 只读接口
   │     depends_on: corpus, token-batch
   │     provides: rrf_merge, read, rg
   │
   └─ eval                  │   口径；随 M1 指标跑
         depends_on: retrieval
```

[deferred](deferred.md) 不在默认 DAG 里（树、词法引擎、MCP、沙箱、云）。

## 里程碑（收窄后）

| 里程碑 | 实现哪些文档 | 门 |
|---|---|---|
| **M0** | corpus, token-batch, zvec-spike | 两次 ingest `corpus_fp` 相同；`n_docs = 100195`；全表 `token_count`；zvec spike 报告 |
| **M1** | retrieval 的向量路 + read + rg；eval 索引级 | Challenge-20 上报告绝对 recall；不拿官方 26.4%/59.7% 当及格线 |
| **M2** | retrieval 的词法路 + RRF | 先跑官方预构建 BM25 oracle；hybrid ≥ 本系统单路最优 |
| **M3** | deferred 中的树，若仍值得做 | 形状/标签 + route recall@k；不以「根覆盖」为门 |
| **M4** | MCP 工具面 + eval agent 级 | 默认 `vgi_search` + `vgi_read` |

## 代码落点

独立 workspace，默认路径：本仓库的 `../vgi-rs`。本目录只持有规格与 anchor 测试。

过夜无人值守 M0：[overnight-goal.md](overnight-goal.md)；workflow：`.grok/workflows/vgi-m0-overnight.rhai`（`/workflow runs` 里的 display name 是 `vgi-m0-overnight`）。
