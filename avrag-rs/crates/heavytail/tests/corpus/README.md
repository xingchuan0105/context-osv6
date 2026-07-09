# HeavyTail validation corpora (GATE 1 / M1)

Local-only Chinese `.txt` files for human-vs-AI fingerprint separation checks.
Copyrighted real-world articles should stay on disk only; paths are gitignored via
`/crates/heavytail/tests/corpus/*/*.txt`.

## Layout

```
corpus/
  human/   # bursty, clustered sentence lengths, higher hapax
  ai/      # uniform sentence rhythm, closed vocabulary
```

## Provenance (MVP synthetic gate)

For the initial M1 gate, corpora were generated synthetically by
`crates/heavytail/scripts/generate_corpus.py` (12 topics × 2 classes). Human-like
files alternate short/long sentence **runs** within paragraphs; AI-like files use
fixed-length template sentences built from a small repeated vocabulary.

Replace with real essays / model outputs when available; re-run compare afterward.

## Regenerate synthetic corpus

```bash
python3 crates/heavytail/scripts/generate_corpus.py
```

## Run comparison

```bash
cargo run -p heavytail --bin heavytail-analyze -- \
  compare crates/heavytail/tests/corpus/human crates/heavytail/tests/corpus/ai
```

## GATE 1 results (2026-07-06, synthetic MVP)

Corpus sizes: **12 human**, **12 AI** files.

| Metric | Human μ | Human σ | AI μ | AI σ | Cohen's d | Verdict |
|--------|---------|---------|------|------|-----------|---------|
| CV | 0.8969 | 0.0242 | 0.0000 | 0.0000 | **52.4367** | SEPARATES |
| Hapax | 0.2456 | 0.0933 | 0.0565 | 0.0261 | **2.7608** | SEPARATES |
| Burstiness (autocorr lag-1) | 0.5054 | 0.0289 | 0.0000 | 0.0000 | **24.7136** | SEPARATES |
| Zipf | 0.5595 | 0.1128 | 0.8292 | 0.0344 | -3.2354 | SEPARATES |
| KS | 0.2523 | 0.0096 | 0.0000 | 0.0000 | 37.2915 | SEPARATES |

**GATE 1 verdict: PASS** — CV, hapax, and burstiness all reach |d| ≥ 0.8 on the
synthetic MVP corpora. Real-world corpora should be re-validated before production
claims.
