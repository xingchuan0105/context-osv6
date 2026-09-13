---
module: token-batch
provides: BatchWindow[], fill_token_counts(documents)
depends_on: [corpus]
---

# Token batching

## Interface

    LIMIT   = 7372          # floor(8192 * 0.90), bge-m3
    OVERLAP = 256
    STRIDE  = 7116          # LIMIT - OVERLAP
    CAP     = 64            # max windows per document

    BatchWindow = {start: int, end: int}   # token offsets, half-open [start, end)
    windows(n_tokens) -> (BatchWindow[], truncated: bool)

Tokenizer identity（填 `token_count` 时锁定）：HuggingFace `BAAI/bge-m3`（XLM-RoBERTa）。`manifest.tokenizer = "BAAI/bge-m3"`. 本页的窗口公式吃的是整数 `n_tokens`，不依赖某次 encode 的偶然分词。

## Semantics

`n_tokens = 0`：一个空窗 `[0, 0)`，`truncated=false`。

`n_tokens <= LIMIT`：`[0, n_tokens)`，`truncated=false`。

否则从 `start = 0, STRIDE, 2*STRIDE, …` 开窗，每窗长度为 `LIMIT`，直到覆盖 `n_tokens` 或达到 `CAP` 个窗：

- 若最后一个窗的 `end >= n_tokens`：把该 `end` 收成 `n_tokens`，`truncated=false`。
- 否则：保留 `CAP` 个满窗 `[i*STRIDE, i*STRIDE+LIMIT)`，丢掉 `>= CAP*覆盖` 的尾部，`truncated=true`。64 窗覆盖的最大 token 数是 `63*7116 + 7372 = 455680`。

超长文档（语料 max ≈ 9.96M 字符）按此截断并在 ingest 日志记 `truncated_docs`。M0 只计数与开窗，不调用 embedding。

## Worked example — 8000 tokens

`ceil` 无重叠会给出 `N=2`，但覆盖公式用 STRIDE：

1. `8000 > 7372` → 开窗。
2. 窗 0：`[0, 7372)`。
3. `7372 < 8000` → 窗 1 从 `7116` 起：`[7116, 7116+7372=14488)`，收成 `[7116, 8000)`。
4. `truncated=false`。重叠区 `[7116, 7372)` 长 256。

7373 token：窗 `[0,7372)` 与 `[7116,7373)`（第二窗长 257）。

455681 token：需要第 65 窗才能覆盖；发出 64 个满窗，最后 `[448308, 455680)`，`truncated=true`，下标 `455680` 被丢。

## Anchor

| n_tokens | windows | truncated |
|---:|---|:---:|
| 0 | `[0,0)` | false |
| 100 | `[0,100)` | false |
| 7372 | `[0,7372)` | false |
| 7373 | `[0,7372)`, `[7116,7373)` | false |
| 8000 | `[0,7372)`, `[7116,8000)` | false |
| 455680 | 64 窗，最后 `[448308,455680)` | false |
| 455681 | 64 窗，最后 `[448308,455680)` | true |

**Enforced by:** `docs/vgi/anchors/test_anchors.py::test_token_windows`

M0 全库：每个 `documents` 行有非空 `token_count`（含 0）。费用估算用 `sum(token_count)` 与窗数，在第一次 embedding 前报告，不沿用 176,438 的 4 字符启发式。
