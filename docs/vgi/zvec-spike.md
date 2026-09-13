---
module: zvec-spike
provides: spike_zvec(n, dim) -> SpikeReport
depends_on: []
---

# zvec spike (M0)

## Interface

    SpikeReport = {
      ok: bool,
      n: int,
      dim: int,                 # 1024
      dtype: "fp16",
      p95_query_ms: float,
      rss_bytes: int,
      backend: "zvec-rust" | "hnsw_rs",
      error: str | null
    }

    spike_zvec(n=10000, dim=1024) -> SpikeReport

依赖：crates.io `zvec-rust`，feature `bundled`（预编译 `libzvec_c_api`）。Windows 目标 `x86_64-pc-windows-msvc`。

## Semantics

1. 建 collection，向量字段 fp16、1024 维、HNSW cosine。
2. 插入 `n` 条确定性伪向量（seed=0 的字节模式，可复现，不是语料）。
3. 用其中 32 条当查询，每条 top-10。
4. 写 `.vgi/jobs/zvec-spike.json`。

`ok=true` 当且仅当插入与 32 次查询都返回、进程未中止。p95 记入报告，**不是** M0 的失败门（机器会变）。

链接或 API 失败：同一接口换 `hnsw_rs` 再跑一次，`backend` 标明。上层检索仍按「进程内 ANN，uid=`doc_id#batch`」——本 spike 不实现 uid 映射。

本 spike **不** 读 corpus、不把 17 万语料向量灌进 zvec。那是 M1。

## Worked example — n=4, dim=2 (逻辑)

不是生产维度；用来钉「插入条数 = 查询可见条数」。

1. 插入 4 条。
2. 用第 0 条查询，`top_k=4`。
3. 返回 4 个 id，含被查询的那条。

真 spike 用 n=10000, dim=1024；本逻辑例不进 anchor 数值。

## Anchor

无闭式数值（吞吐随机器）。门是：

- 报告文件存在且 `ok=true`
- `n=10000`、`dim=1024`、`dtype=fp16`
- `backend` 为两个枚举之一

**Enforced by:** M0 手工/脚本检查 `.vgi/jobs/zvec-spike.json`（实现时生成）。本目录的 `test_anchors.py` 不覆盖本模块。
