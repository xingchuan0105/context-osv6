# HeavyTail Writer v2 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [`docs/superpowers/specs/2026-07-06-heavytail-writer-v2-design.md`](../specs/2026-07-06-heavytail-writer-v2-design.md). The v1 spec is referenced **only** by Task 12 (experiment arm C).

**Goal:** Implement the feedback-first HeavyTail Writer: deterministic Analyzer + sensitivity table, line-addressed draft workspace with patch grammar, MPC sectionwise drafting, evaluator-optimizer refinement loop, and `write` mode integration with research workers.

**Architecture:** New workspace crate `crates/heavytail` hosts everything that does not depend on app-chat: analyzer, scoring, sensitivity/op enumeration, workspace kernel, directive compiler, drafting/refinement stages, and two CLI bins (`heavytail-analyze`, `heavytail-experiment`). A thin integration layer in `crates/app-chat/src/writer/` hosts the orchestrator glue: mode dispatch, `SubagentInvoker` (research workers via the existing react loop), material cards, SSE progress. Dependency direction: `app-chat → heavytail`, never the reverse.

**Tech Stack:** Rust; `jieba-rs` (workspace dep, already at 0.7); `rand` + `rand_chacha` (crate-local, seeded reproducibility); no `statrs`/`clap` — inverse-normal CDF and arg parsing are hand-rolled (~30 lines each); `crates/llm` `LlmClient` for all LLM calls; `tokio` for bins and fan-out.

---

## Execution Order, Parallelism, Gates

| Group | Tasks | Depends on | Parallelizable within group |
|---|---|---|---|
| A — deterministic core | 1–9 | — (Task 1 first) | yes, after Task 1 |
| **GATE 1** | Task 6 step 5 | Tasks 2–6 | **M1 exit gate: if metrics do not separate human vs AI corpora, STOP and escalate to the user** |
| B — LLM stages | 10–13 | A + `.env` LLM config | 10 → {11, 12} → 13 |
| **GATE 2** | Task 13 step 4 | Task 13 | **M3 decision rules (spec §16): may retire arm-C code and/or deficit hints** |
| C — refinement | 14–15 | A, B | 14 → 15 |
| D — integration | 16–19 | C | 16 → {17, 18} → 19 |

LLM-dependent tests are env-gated: they read the existing `AGENT_LLM_*` / `E2E_LLM_*` variables from `avrag-rs/.env` (never ask the user; see Task 10 step 1) and are `#[ignore]`d or skip-with-message when unset.

---

## File Map

| File | Responsibility |
|---|---|
| `Cargo.toml` (root) | Add `crates/heavytail` to members |
| `crates/heavytail/Cargo.toml` | **NEW** crate manifest, two `[[bin]]` targets |
| `crates/heavytail/src/lib.rs` | **NEW** module tree, `StyleParams`, shared types |
| `crates/heavytail/src/math.rs` | **NEW** `inv_norm_cdf` (Acklam), `norm_cdf` (A&S 7.1.26), linear regression |
| `crates/heavytail/src/segment.rs` | **NEW** quote-aware sentence segmentation, char counting |
| `crates/heavytail/src/tokenize.rs` | **NEW** jieba wrapper, content-word filter, embedded stopword list |
| `crates/heavytail/src/metrics.rs` | **NEW** `FingerprintReport` + all fingerprint metrics |
| `crates/heavytail/src/score.rs` | **NEW** quantile targets, W1, band scores, composite S |
| `crates/heavytail/src/sensitivity.rs` | **NEW** length sensitivity table (exact enumeration) |
| `crates/heavytail/src/lexops.rs` | **NEW** promote/demote op enumeration |
| `crates/heavytail/src/placement.rs` | **NEW** target-to-position assignment (burstiness) |
| `crates/heavytail/src/workspace.rs` | **NEW** `DraftWorkspace`, sentence IDs, canonicalizer, render |
| `crates/heavytail/src/patch.rs` | **NEW** patch grammar parser, allow-set validation, splicer |
| `crates/heavytail/src/state.rs` | **NEW** `WriterState`, `RoundRecord`, file checkpoints |
| `crates/heavytail/src/llm.rs` | **NEW** `WriterLlm` wrapper over `avrag_llm::LlmClient` |
| `crates/heavytail/src/skeleton.rs` | **NEW** skeleton stage (JSON contract) |
| `crates/heavytail/src/draft.rs` | **NEW** MPC sectionwise drafting, deficit hints |
| `crates/heavytail/src/feedforward.rs` | **NEW** arm C only: AR(1) schedule + v1 §7 prompts |
| `crates/heavytail/src/compiler.rs` | **NEW** directive compiler + Chinese prompt rendering |
| `crates/heavytail/src/refine.rs` | **NEW** refinement round runner (two passes) |
| `crates/heavytail/src/validator.rs` | **NEW** tolerance bands, `ValidationReport` |
| `crates/heavytail/src/bin/analyze.rs` | **NEW** CLI: fingerprint + sensitivity + corpus compare |
| `crates/heavytail/src/bin/experiment.rs` | **NEW** CLI: M3 three-arm experiment runner |
| `crates/app-chat/Cargo.toml` | Add `heavytail` dependency |
| `crates/app-chat/src/agents/capability/schemas.rs` | Add `write_mode_schema()` |
| `crates/app-chat/src/writer/mod.rs` | **NEW** `WriterOrchestrator` glue |
| `crates/app-chat/src/writer/invoker.rs` | **NEW** `SubagentInvoker` + fan-out |
| `crates/app-chat/src/writer/cards.rs` | **NEW** MaterialCard extraction + guard pass |
| `crates/app-chat/src/chat/pipeline_steps.rs` | Route `agent_type == "write"` to orchestrator |

---

## Task 1: Crate scaffold + math module

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `crates/heavytail/Cargo.toml`, `crates/heavytail/src/lib.rs`, `crates/heavytail/src/math.rs`

- [ ] **Step 1: Register workspace member**

Add `"crates/heavytail",` to `[workspace] members` in the root `Cargo.toml`.

- [ ] **Step 2: Crate manifest**

```toml
# crates/heavytail/Cargo.toml
[package]
name = "heavytail"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[[bin]]
name = "heavytail-analyze"
path = "src/bin/analyze.rs"

[[bin]]
name = "heavytail-experiment"
path = "src/bin/experiment.rs"

[dependencies]
avrag-llm = { path = "../llm" }
common = { path = "../common" }
jieba-rs = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tokio = { workspace = true }
rand = "0.8"
rand_chacha = "0.3"
once_cell = { workspace = true }
```

If any `{ workspace = true }` entry is missing from root `[workspace.dependencies]`, check how sibling crates (`crates/llm/Cargo.toml`) declare it and mirror that.

- [ ] **Step 3: Module tree in `lib.rs`**

```rust
pub mod math;
pub mod segment;
pub mod tokenize;
pub mod metrics;
pub mod score;
pub mod sensitivity;
pub mod lexops;
pub mod placement;
pub mod workspace;
pub mod patch;
pub mod state;
pub mod llm;
pub mod skeleton;
pub mod draft;
pub mod feedforward;
pub mod compiler;
pub mod refine;
pub mod validator;

/// Carried from v1 spec unchanged (spec §12).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StyleParams {
    pub cv: f64,
    pub phi: f64,            // arm-C experiments only
    pub median_length: f64,
    pub hapax_target: f64,
    pub zipf_exponent: f64,
}

impl Default for StyleParams {
    fn default() -> Self {
        Self { cv: 0.75, phi: 0.4, median_length: 20.0, hapax_target: 0.45, zipf_exponent: 1.0 }
    }
}
```

Create empty stub files for all modules so the crate compiles; subsequent tasks fill them.

- [ ] **Step 4: `math.rs`**

```rust
/// Acklam's rational approximation of the inverse normal CDF. |err| < 1.15e-9.
pub fn inv_norm_cdf(p: f64) -> f64 { /* standard Acklam coefficients, 3 branches */ }

/// Abramowitz & Stegun 7.1.26 erf approximation → Φ(x). |err| < 7.5e-8.
pub fn norm_cdf(x: f64) -> f64 { /* ... */ }

/// OLS slope+intercept for y ~ a + b·x.
pub fn linreg(xs: &[f64], ys: &[f64]) -> (f64, f64) { /* ... */ }
```

Tests: `inv_norm_cdf(0.5) == 0.0` (±1e-9); `inv_norm_cdf(0.99) ≈ 2.3263` (±1e-3); `norm_cdf(inv_norm_cdf(p)) ≈ p` over a grid; `linreg` against a hand-computed pair.

- [ ] **Step 5: Verify + commit**

```bash
cargo check -p heavytail && cargo test -p heavytail
git add Cargo.toml crates/heavytail/ && git commit -m "feat(heavytail): scaffold crate with math module"
```

---

## Task 2: Segmentation + tokenization

**Files:** `crates/heavytail/src/segment.rs`, `crates/heavytail/src/tokenize.rs`

- [ ] **Step 1: Sentence segmentation (spec §5.1)**

```rust
pub struct RawSentence { pub text: String, pub para_idx: usize }

/// Split free prose into sentences. Terminators: 。！？ (；only when
/// `semicolon_splits` — default false). Trailing closing quotes/brackets
/// （ ” ’ 」 』 ） follow their sentence. Paragraphs split on blank lines.
pub fn split_sentences(prose: &str, semicolon_splits: bool) -> Vec<RawSentence>;

/// Character count excluding whitespace (spec: length = non-whitespace chars).
pub fn char_len(s: &str) -> usize;
```

Edge-case tests (spec M2 exit): `他说："走吧。"然后离开了。` → 2 sentences with the quote attached; ellipsis `……` not a terminator; multiple blank lines; text without trailing terminator (final fragment kept as a sentence).

- [ ] **Step 2: Tokenization + content-word filter**

Own `OnceLock<Jieba>` (do not reuse `common::text_segment` — it returns FTS-joined strings, not token lists).

```rust
pub fn tokens(text: &str) -> Vec<String>;           // jieba cut, punctuation/whitespace dropped
pub fn is_content_word(w: &str) -> bool;            // char_len ≥ 2 && !STOPWORDS.contains(w)
pub const STOPWORDS: &[&str] = &[ /* ~200 function words: 的 了 是 在 我 有 和 就 不 人 都 一 ... */ ];
```

**MVP resolution of spec Open Question 1:** no global frequency table. "Rare" is defined relationally — demote ops use in-draft frequencies vs the draft's own Zipf fit; promote candidates come from the reservoir (material cards / topic terms), filtered to words not present in the draft. Record this in a module doc comment with the upgrade path (BCC/BLCU table).

- [ ] **Step 3: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/{segment,tokenize}.rs && git commit -m "feat(heavytail): quote-aware segmentation and jieba tokenization"
```

---

## Task 3: Fingerprint metrics

**Files:** `crates/heavytail/src/metrics.rs`

- [ ] **Step 1: Types (v1 spec §8 carried + v2 extensions)**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FingerprintReport {
    pub sentence_lengths: Vec<usize>,
    pub mean_length: f64,
    pub cv: f64,
    pub autocorr_lag1: f64,
    pub lognormal_ks_stat: f64,      // report-only, never for targeting
    pub total_tokens: usize,
    pub vocab_size: usize,
    pub ttr: f64,
    pub hapax_ratio: f64,
    pub zipf_exponent: f64,
    pub word_freq: std::collections::BTreeMap<String, usize>, // content words only
}

pub fn analyze_sentences(sentences: &[(String /*text*/, usize /*para*/)]) -> FingerprintReport;
```

- [ ] **Step 2: Metric implementations**

- `cv = stddev / mean` (population stddev); guard n < 2 → 0.0.
- `autocorr_lag1`: Pearson corr of `(l_1..l_{n-1})` vs `(l_2..l_n)`; guard zero variance → 0.0.
- `lognormal_ks_stat`: MLE fit `(μ̂, σ̂)` on `ln(l_i)`, KS = max |F_emp − Φ((ln x − μ̂)/σ̂)| using `math::norm_cdf`.
- `zipf_exponent`: `-slope` of `linreg(ln rank, ln freq)` over content-word rank/freq pairs.
- `hapax_ratio = |{w : freq(w)==1}| / vocab_size` over content words.

Tests: hand-computed 5-sentence fixture; constant lengths → cv 0, autocorr 0; alternating 10/30 lengths → negative autocorr; synthetic Zipf sample → exponent within ±0.15 of 1.0.

- [ ] **Step 3: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/metrics.rs && git commit -m "feat(heavytail): fingerprint metrics"
```

---

## Task 4: Quantile targets + composite score

**Files:** `crates/heavytail/src/score.rs`

- [ ] **Step 1: Targets and W1 (spec §3.2)**

```rust
pub const L_MIN: f64 = 5.0;
pub const L_MAX: f64 = 100.0;

/// q_(i) = exp(μ_z + σ_z · Φ⁻¹((i−0.5)/n)), clamped to [L_MIN, L_MAX].
pub fn quantile_targets(n: usize, style: &StyleParams) -> Vec<f64>;

/// W1 = mean |sorted(lengths) − targets|; normalized by E[l] = median·exp(σ_z²/2).
pub fn w1(lengths: &[usize], targets: &[f64]) -> f64;
pub fn w1_normalized(lengths: &[usize], style: &StyleParams) -> f64;
```

- [ ] **Step 2: Band scores + composite (spec §3.6, §11)**

```rust
pub struct Bands { pub target: (f64, f64), pub hard: (f64, f64) }

/// 1.0 inside target band; linear decay to 0.0 at the hard bound; 0.0 beyond.
pub fn band_score(x: f64, b: &Bands) -> f64;

pub struct Score { pub s: f64, pub len: f64, pub burst: f64, pub hapax: f64, pub zipf: f64 }

/// Weights 0.4/0.2/0.25/0.15. The length component maps Ŵ1 through a
/// decreasing ramp: len = clamp(1 − Ŵ1/0.5, 0, 1).
pub fn composite(fp: &FingerprintReport, style: &StyleParams) -> Score;

pub fn bands_for(style: &StyleParams) -> [(&'static str, Bands); 4]; // cv/hapax/burst/zipf per spec §11
```

- [ ] **Step 3: Spec worked-example test**

n=50, median=20, cv=0.75: assert targets at sorted positions {1,5,13,25,38,45,49,50} equal {5,8,13,20,31,45,70,95} within ±1 char (spec §3.2 table). Assert monotonicity and clamping.

- [ ] **Step 4: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/score.rs && git commit -m "feat(heavytail): quantile targets, W1, composite score"
```

---

## Task 5: Sensitivity table + lexical ops + placement

**Files:** `crates/heavytail/src/sensitivity.rs`, `crates/heavytail/src/lexops.rs`, `crates/heavytail/src/placement.rs`

- [ ] **Step 1: Length sensitivity by exact enumeration (spec §3.3)**

```rust
pub const CANDIDATE_GRID: &[usize] = &[5, 8, 12, 16, 20, 26, 34, 44, 56, 72, 90];

pub struct SensitivityRow {
    pub sentence_idx: usize,
    pub current_len: usize,
    pub candidate_len: usize,
    pub delta_s: f64,          // exact recompute of composite S with this one length swapped
}

pub fn length_sensitivity(fp: &FingerprintReport, style: &StyleParams) -> Vec<SensitivityRow>;
```

Property tests: (a) every `delta_s` equals an independent brute-force recomputation (recompute from scratch in the test — guards against incremental-update bugs later); (b) for a draft with uniform lengths, the best rows push toward extremes.

- [ ] **Step 2: Lexical op enumeration (spec §3.4)**

```rust
pub enum LexOp {
    Promote { word: String, replacement_pool: Vec<String>, delta_s: f64 },  // freq==2 first, freq==3 second
    Demote  { word: String, current: usize, max_count: usize, delta_s: f64 },
}

/// reservoir = candidate rare words (from material cards / topic terms), filtered
/// to words absent from the draft.
pub fn enumerate_lexops(fp: &FingerprintReport, reservoir: &[String], style: &StyleParams) -> Vec<LexOp>;
```

Demote candidates: content words whose freq exceeds `zipf_expected(rank) × 2.0`. Property test: applying one Promote to a synthetic freq map yields hapax types +2, vocab +1 (spec §3.4 identity).

- [ ] **Step 3: Placement (spec §3.5)**

```rust
pub struct PlacementPlan {
    pub edits: Vec<(usize /*sentence_idx*/, usize /*target_len*/)>, // ≤ K
    pub planned_autocorr: f64,
}

/// Choose ≤ K edit positions (largest |gap|), assign targets from the unmatched
/// quantiles, greedily permute assignments (within-paragraph clustering) until
/// planned lag-1 autocorr ∈ [0.2, 0.5] or local optimum. Deterministic given seed.
pub fn plan_placement(fp: &FingerprintReport, para_of: &[usize], style: &StyleParams, k: usize, seed: u64) -> PlacementPlan;
```

Test: synthetic uniform draft → plan contains both very-short and long targets, clustered by paragraph, planned autocorr in band.

- [ ] **Step 4: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/{sensitivity,lexops,placement}.rs && git commit -m "feat(heavytail): sensitivity table, lexical ops, placement planner"
```

---

## Task 6: `heavytail-analyze` CLI + M1 validation gate

**Files:** `crates/heavytail/src/bin/analyze.rs`

- [ ] **Step 1: CLI (manual arg parsing, no clap)**

```
heavytail-analyze <file.txt> [--json]        # fingerprint + top-20 sensitivity rows
heavytail-analyze compare <dir_human> <dir_ai>   # per-metric mean/std + Cohen's d + verdict
```

`compare` reads every `*.txt` in each dir, prints a table over {cv, hapax_ratio, autocorr_lag1, zipf_exponent, ks} with Cohen's d, and a verdict line per metric: `SEPARATES (|d| ≥ 0.8)` / `WEAK` / `NO`.

- [ ] **Step 2: Fixture smoke test**

Two tiny fixtures under `crates/heavytail/tests/fixtures/` (one bursty human-like, one uniform AI-like); integration test asserts the human-like fixture scores higher CV and the CLI runs end-to-end.

- [ ] **Step 3: Corpus directory conventions**

Create `crates/heavytail/tests/corpus/{human,ai}/.gitkeep` and add `crates/heavytail/tests/corpus/*/*.txt` to `.gitignore` (corpora may be copyrighted; they stay local).

- [ ] **Step 4: Collect corpus**

Collect 10+ human-written and 10+ AI-generated Chinese articles on comparable topics into the corpus dirs. Human sources: essays/blogs/news the user already has locally or public-domain texts; AI: generate with the configured production model via a throwaway script. Document provenance in `tests/corpus/README.md`.

- [ ] **Step 5: 🚦 GATE 1 — run the comparison**

```bash
cargo run -p heavytail --bin heavytail-analyze -- compare crates/heavytail/tests/corpus/human crates/heavytail/tests/corpus/ai
```

**Exit gate (spec §16 M1, carried from v1):** if CV, hapax, and burstiness do NOT separate (|d| < 0.8 on all three), STOP the plan and report to the user — the premise fails and v2 §1 needs re-evaluation. Record the comparison table in the commit message.

- [ ] **Step 6: Commit**

```bash
git add crates/heavytail/src/bin/analyze.rs crates/heavytail/tests/ .gitignore
git commit -m "feat(heavytail): analyze CLI + human-vs-AI validation gate"
```

---

## Task 7: Draft workspace + canonicalizer

**Files:** `crates/heavytail/src/workspace.rs`

- [ ] **Step 1: Types and ID scheme (spec §5.1–5.2)**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SentenceId(pub String);   // "s07", "s07a", "s07ab"

impl SentenceId {
    pub fn children(&self) -> (SentenceId, SentenceId);  // s07 → (s07a, s07b); s07a → (s07aa, s07ab)
    pub fn is_valid(s: &str) -> bool;                    // ^s[0-9]+[a-z]*$
}

pub struct SentenceRecord { pub id: SentenceId, pub text: String, pub para: usize, pub tombstone: bool }
pub struct ParagraphRecord { pub idx: usize, pub rhythm: RhythmMode }
pub enum RhythmMode { ShortBurst, LongFlow, Mixed }

pub struct DraftWorkspace { pub sentences: Vec<SentenceRecord>, pub paragraphs: Vec<ParagraphRecord> }

impl DraftWorkspace {
    /// Append a freshly drafted section (free prose) — segmentation + new IDs
    /// continuing the global counter. Never reuses tombstoned IDs.
    pub fn append_section(&mut self, prose: &str, rhythms: &[RhythmMode]);
    /// Canonical form for LLM consumption (spec §5.1): headers + id-prefixed lines.
    pub fn render_canonical(&self) -> String;
    /// Final text: strip IDs/tombstones, join by paragraph.
    pub fn render_plain(&self) -> String;
    /// Live (non-tombstoned) sentences in document order.
    pub fn live(&self) -> impl Iterator<Item = &SentenceRecord>;
}
```

Document-order invariant: `sentences` is kept in document order; split children are inserted at the parent's position; tombstones stay in place but render nowhere.

- [ ] **Step 2: Tests**

Canonical round-trip (prose → workspace → render_plain equals normalized prose); ID continuation across sections; children ordering; render_canonical exact-format fixture (matches spec §5.1 example shape).

- [ ] **Step 3: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/workspace.rs && git commit -m "feat(heavytail): draft workspace, ID scheme, canonicalizer"
```

---

## Task 8: Patch grammar + splicer

**Files:** `crates/heavytail/src/patch.rs`

- [ ] **Step 1: Parser (spec §5.3)**

```rust
pub struct AllowSet {
    pub replace: std::collections::BTreeSet<SentenceId>,           // EXTEND/REWRITE/PROMOTE targets + MERGE keep-id
    pub split_children: std::collections::BTreeMap<SentenceId, (SentenceId, SentenceId)>,
    pub tombstone_on_apply: std::collections::BTreeSet<SentenceId>, // MERGE absorb-ids
}

pub enum PatchError { BadLine(usize), UnknownId(SentenceId), TombstonedId(SentenceId),
                      NotSingleSentence(SentenceId), Empty }

pub struct Patch { pub lines: Vec<(SentenceId, String)> }

/// Reject-whole-patch semantics: any violation returns Err.
/// Line grammar: ^(s[0-9]+[a-z]*)\|\s?(.+)$
/// Sentence rule: exactly one terminator (。！？) at end, none mid-line
/// outside quoted spans (reuse segment.rs quote logic).
pub fn parse_patch(raw: &str, allow: &AllowSet) -> Result<Patch, PatchError>;

/// Splice into the workspace: replace by id / insert split children at parent /
/// tombstone parents and absorbed ids. Returns changed ids.
/// Debug-asserts byte-equality of every untouched live sentence (belt-and-suspenders).
pub fn apply_patch(ws: &mut DraftWorkspace, patch: &Patch, allow: &AllowSet) -> Vec<SentenceId>;
```

- [ ] **Step 2: Malformed-patch battery (spec M2 exit)**

Table-driven tests: unknown id; tombstoned id; missing `|`; two sentences on one line; mid-line 。 inside quotes (must PASS); empty patch; split child emitted without its sibling (decide: sibling optional — parent tombstones only when **both** children present, else reject `BadLine`; encode the chosen rule in a test); merge patch emitting the absorbed id (reject).

- [ ] **Step 3: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/patch.rs && git commit -m "feat(heavytail): patch grammar, allow-set validation, splicer"
```

---

## Task 9: WriterState + checkpoints

**Files:** `crates/heavytail/src/state.rs`

- [ ] **Step 1: State types (spec §12) + persistence**

`WriterPhase`, `WriterState`, `RoundRecord { fingerprint, directives_json, patch_raw, compliance, score }`, `BestVersion { round, score, canonical_text }`, `WriterBudget` — all serde. 

```rust
impl WriterState {
    pub fn checkpoint(&self, dir: &std::path::Path) -> anyhow::Result<()>;  // state.json + sidecars per spec §5.4
    pub fn restore(dir: &std::path::Path) -> anyhow::Result<Self>;
    pub fn record_round(&mut self, r: RoundRecord);   // updates best if score improves
}
```

- [ ] **Step 2: Round-trip test** (tempdir; checkpoint → restore → deep-equal; sidecar files exist per spec §5.4 naming).

- [ ] **Step 3: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/state.rs && git commit -m "feat(heavytail): writer state and file checkpoints"
```

---

## Task 10: LLM plumbing

**Files:** `crates/heavytail/src/llm.rs`

- [ ] **Step 1: Discover env config names**

```bash
rg -n "AGENT_LLM|E2E_LLM|ModelProviderConfig" .env.example crates/app-core/src/config.rs crates/app-bootstrap/src/config_helpers.rs | head -30
```

Reuse the existing variable names verbatim (rule: never invent new credential vars; map/alias if prefixes differ). Read from process env — bins source `.env` via the same mechanism sibling bins use (check `tests/rag_quality/src/bin/quality_runner.rs` for the pattern).

- [ ] **Step 2: Wrapper**

```rust
pub struct WriterLlm { client: avrag_llm::LlmClient }

impl WriterLlm {
    pub fn from_env() -> anyhow::Result<Self>;  // builds ModelProviderConfig from the discovered vars
    pub async fn prose(&self, system: &str, user: &str, temp: f32) -> anyhow::Result<String>;
    /// complete_json_mode + one reparse-retry with the parse error appended.
    pub async fn json<T: serde::de::DeserializeOwned>(&self, system: &str, user: &str) -> anyhow::Result<T>;
}
```

Env-gated smoke test (`#[ignore]` unless vars set): one tiny prose call round-trips.

- [ ] **Step 3: Verify + commit**

```bash
cargo check -p heavytail
git add crates/heavytail/src/llm.rs && git commit -m "feat(heavytail): LLM wrapper over crates/llm client"
```

---

## Task 11: Skeleton + MPC sectionwise drafting

**Files:** `crates/heavytail/src/skeleton.rs`, `crates/heavytail/src/draft.rs`

- [ ] **Step 1: Skeleton stage (spec §8)**

`Skeleton`/`SkeletonSection`/`ParagraphPlan` types (serde). `pub async fn plan_skeleton(llm, topic, target_chars, cards: &[MaterialCard]) -> Result<Skeleton>` via `WriterLlm::json`, prompt per spec §8. `MaterialCard` type lives here in `heavytail` with `source: serde_json::Value` (MVP; integration maps real `SourceRef` in Task 17).

- [ ] **Step 2: Deficit hints (spec §9)**

```rust
/// ≤ 3 hints from the running fingerprint vs style targets, rendered in Chinese:
/// short-sentence deficit, long-sentence deficit, reservoir suggestions.
pub fn deficit_hints(fp: &FingerprintReport, style: &StyleParams, reservoir: &[String]) -> Vec<String>;
```

Unit test: uniform-length running draft → contains a short-sentence hint; empty draft → empty hints.

- [ ] **Step 3: Section drafting loop**

`pub async fn draft_sections(llm, skeleton, style, cards, ws: &mut DraftWorkspace, mpc: bool) -> Result<()>` — per-section brief per spec §9 (priming block inlined as a const for now; Task 18 moves it to the skill slot), context = skeleton + last section verbatim + older one-line summaries, free-prose output, `append_section`, incremental fingerprint. `mpc: bool` toggles deficit hints (M3 arm a vs b).

- [ ] **Step 4: Verify + commit**

```bash
cargo check -p heavytail
git add crates/heavytail/src/{skeleton,draft}.rs && git commit -m "feat(heavytail): skeleton stage and MPC sectionwise drafting"
```

---

## Task 12: Arm C — v1 feedforward Phase A (experiment-only)

**Files:** `crates/heavytail/src/feedforward.rs`

- [ ] **Step 1: AR(1) schedule (v1 spec §3.1/§6, seeded)**

```rust
/// var_z = ln(1+cv²); σ_ε = sqrt(var_z(1−φ²)); μ_z = ln(median); z_0 = μ_z.
pub fn ar1_schedule(n: usize, style: &StyleParams, seed: u64) -> Vec<usize>; // clamped [5,100]
pub fn length_bin(chars: usize) -> &'static str; // v1 §7 bin descriptions (XShort..XLong)
```

Statistical test: 10k samples → empirical CV within ±0.1 of style.cv, lag-1 autocorr within ±0.1 of φ.

- [ ] **Step 2: v1 §7 per-paragraph prompt builder** (`pub fn feedforward_brief(...) -> String` with the 第N句/长度要求 format). No Phase B — arm C measures the open-loop draft only.

- [ ] **Step 3: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/feedforward.rs && git commit -m "feat(heavytail): arm-C feedforward generator (v1 Phase A)"
```

---

## Task 13: `heavytail-experiment` — M3 three-arm runner

**Files:** `crates/heavytail/src/bin/experiment.rs`, `crates/heavytail/experiment-topics.txt`

- [ ] **Step 1: Topics file** — 10 one-line Chinese topics of mixed genre (tech explainer, opinion, narrative, finance, howto...).

- [ ] **Step 2: Runner**

```
heavytail-experiment [--topics <file>] [--arms a,b,c] [--out heavytail-out]
```

Per topic × arm: arm **a** = plain free-write (skeleton + sections, no priming, no hints); arm **b** = priming + MPC hints; arm **c** = feedforward briefs (Task 12). Plus the prose-vs-lines A/B on arm b (spec R4): re-run 3 topics with "one sentence per line" instructed, compare fingerprints. Artifacts per spec §5.4 layout: `heavytail-out/<ts>/arm-<x>/topic-<n>.draft.txt` + `.fingerprint.json`, plus `summary.md` with per-arm metric means and the spec §16 M3 decision-rule evaluation rendered as explicit verdicts:

```
if mean|CV_b − CV_c| < 0.05 and hapax similar → "RETIRE feedforward (arm C)"
if arm b + 0 rounds already in bands → "MPC hints sufficient"
if |metrics_a − metrics_b| negligible → "DROP deficit hints"
```

- [ ] **Step 3: Run it** (needs `.env` LLM config; ~10 topics × 3 arms ≈ 150–250 LLM calls — confirm budget with a 2-topic dry run first, then full run).

- [ ] **Step 4: 🚦 GATE 2 — record decisions**

Paste `summary.md` verdicts into the plan-execution notes; apply the decisions: possibly delete `feedforward.rs` (keep the test), possibly hard-code `mpc` on/off. Update spec §17 open questions 4 accordingly.

- [ ] **Step 5: Commit**

```bash
git add crates/heavytail/src/bin/experiment.rs crates/heavytail/experiment-topics.txt
git commit -m "feat(heavytail): three-arm drafting experiment runner (M3)"
```

---

## Task 14: Directive compiler

**Files:** `crates/heavytail/src/compiler.rs`

- [ ] **Step 1: Directive types + compile (spec §10, §12)**

```rust
pub enum Directive {
    Split   { id: SentenceId, children: (SentenceId, SentenceId), short_bin: String, gain: f64 },
    Merge   { keep: SentenceId, absorb: SentenceId, gain: f64 },
    Extend  { id: SentenceId, bin: String, weave: Option<String>, gain: f64 },
    Rewrite { id: SentenceId, bin: String, gain: f64 },
    Promote { id: SentenceId, replace: String, with_any_of: Vec<String>, gain: f64 },
    Demote  { word: String, max_count: usize, in_sentences: Vec<SentenceId>, gain: f64 },
}

pub struct RoundDirectives { pub rhythm: Vec<Directive>, pub lexical: Vec<Directive>, pub allow: AllowSet }

pub fn compile(ws: &DraftWorkspace, fp: &FingerprintReport, style: &StyleParams,
               reservoir: &[String], budget: &WriterBudget, seed: u64) -> RoundDirectives;
```

Compile logic: placement plan (Task 5) → op-type selection per gap direction (need shorter: `Split` if current > 2×target else `Rewrite`; need longer: `Merge` if a short adjacent same-paragraph sentence exists else `Extend`); overshoot 1.3 applied when choosing the bin; caps `max_rhythm_ops`/`max_lexical_ops`; lexical ops located to their sentence ids; `AllowSet` derived from the chosen ops.

- [ ] **Step 2: Chinese prompt rendering (spec §5.3 directive block)**

`pub fn render_directives_zh(d: &[Directive]) -> String` — the `PATCH DIRECTIVES` block including the closing rule 未点名的句子不得出现在输出中, length targets as 约X字/X字以内 bins, never exact counts.

- [ ] **Step 3: Tests** — compile on a synthetic uniform workspace: produces splits+extends, respects caps, AllowSet consistent with directives (every directive id in allow, nothing else); render snapshot test.

- [ ] **Step 4: Verify + commit**

```bash
cargo test -p heavytail
git add crates/heavytail/src/compiler.rs && git commit -m "feat(heavytail): directive compiler and prompt rendering"
```

---

## Task 15: Refinement loop + validator

**Files:** `crates/heavytail/src/refine.rs`, `crates/heavytail/src/validator.rs`

- [ ] **Step 1: Validator (spec §11)** — `pub fn validate(fp, style) -> ValidationReport` with the band table, `passed = all in band`.

- [ ] **Step 2: Round runner (spec §10)**

```rust
pub async fn refine(llm: &WriterLlm, ws: &mut DraftWorkspace, style: &StyleParams,
                    reservoir: &[String], budget: &WriterBudget, state: &mut WriterState)
                    -> anyhow::Result<()>
```

Per round: analyze → compile → **rhythm pass** (canonical draft + rendered rhythm directives → `parse_patch`; on `PatchError` retry once with the error message appended; on second failure skip the pass, record) → apply → **lexical pass** (same protocol) → re-analyze → compliance per directive (Split/Extend/Rewrite: achieved length within asked bin ± 30%; Promote: replacement present and old freq reduced; Merge: absorb id gone) → `record_round` (best-version) → stop on validator pass or rounds exhausted. Non-complied directives are recompiled from the new state next round — never re-sent verbatim.

Prompt layout: stable prefix (system + canonical draft), variable suffix (directives) — prefix-cache friendly.

- [ ] **Step 3: Env-gated integration test** — one arm-b draft from Task 13 artifacts through 3 rounds; assert S non-decreasing across retained versions and ≥ 2 bands newly satisfied. `#[ignore]` without LLM env.

- [ ] **Step 4: M4 exit measurement** — run on the 10 arm-b drafts:

```bash
cargo run -p heavytail --bin heavytail-experiment -- --refine heavytail-out/<ts>
```

(add `--refine` subcommand to the experiment bin: loads drafts, runs refinement, appends results to `summary.md`). **Exit criteria (spec §16 M4):** ≥ 8/10 pass all bands within default budget; human spot-check of 3 outputs for splice grammar breaks; mean compliance ≥ ~70%. Record in summary.

- [ ] **Step 5: Commit**

```bash
git add crates/heavytail/src/{refine,validator}.rs crates/heavytail/src/bin/experiment.rs
git commit -m "feat(heavytail): refinement loop, validator, M4 measurement"
```

---

## Task 16: `write` mode schema + dispatch seam

**Files:** `crates/app-chat/Cargo.toml`, `crates/app-chat/src/agents/capability/schemas.rs`, dispatch site (located in step 2)

- [ ] **Step 1: Mode schema**

Add to `schemas.rs` (mirroring `search_mode_schema`):

```rust
pub fn write_mode_schema() -> ModeSchema {
    ModeSchema { id: "write".to_string(),
                 external_tools_used: vec!["web_search".to_string()],
                 requires_internet: true }
}
```

Include it in `standard_mode_schemas()`; fix any `mode_count`/`list_modes` assertions that hardcode 3.

- [ ] **Step 2: Locate the dispatch seam**

```bash
rg -n "agent_type" crates/app-chat/src/chat/pipeline_steps.rs | head
rg -n "AgentKind" crates/app-chat/src/agents/runtime.rs crates/app-chat/src/agents/unified/mod.rs | head -20
```

Identify where `agent_type == "rag" | "search"` selects the react-loop runner. Add a `"write"` arm that constructs `writer::WriterOrchestrator` and runs it instead of the loop. If `AgentKind` is an enum, add `Write`; update exhaustive matches (compiler-driven).

- [ ] **Step 3: Verify + commit**

```bash
cargo check -p app-chat && cargo test -p app-chat capability
git add crates/app-chat/ && git commit -m "feat(write-mode): register write mode schema and dispatch seam"
```

---

## Task 17: Research workers + material cards

**Files:** `crates/app-chat/src/writer/mod.rs`, `crates/app-chat/src/writer/invoker.rs`, `crates/app-chat/src/writer/cards.rs`

- [ ] **Step 1: `SubagentInvoker` (spec §6.2, §7)**

```rust
pub struct SubagentInvoker<'a> { /* AgentService handle or unified agent entry */ }

impl SubagentInvoker<'_> {
    /// Build an AgentRequest for the given kind (Rag | Search) and query,
    /// run it against a collecting AgentEventSink (no SSE), enforce timeout
    /// + token budget, return the AgentRunResult.
    pub async fn run_worker(&self, kind: AgentKind, query: &str, budget: usize,
                            timeout: std::time::Duration) -> anyhow::Result<AgentRunResult>;
}
```

Reuse the existing `AgentRequest` builder path (grep `build_agent_request`); the collecting sink pattern exists in loop tests — mirror it.

- [ ] **Step 2: Fan-out + cards (spec §6.3)**

`research(topic, scope)` — 2–3 template queries per worker, `tokio::JoinSet` over {rag, search} workers, per-worker timeout; each answer → `MaterialCard`s: MVP rule-based extraction (one card per citation: content = cited sentence trimmed to ≤80 chars, `rare_terms` = content tokens of the card absent from a small common-word set), web cards passed through `untrusted_input`/`content_guard` check (grep the existing `check_content` usage and mirror). Degradation: one worker failing → proceed, set `research_degraded`.

- [ ] **Step 3: Orchestrator glue**

`WriterOrchestrator::run(request, sink)`: research → `heavytail::skeleton` → `heavytail::draft_sections` → `heavytail::refine` → validate → map to `AgentRunResult` (answer = `render_plain`, citations from used cards, degrade flags in metadata). Checkpoint `WriterState` under the job artifact dir after each phase.

- [ ] **Step 4: Verify + commit**

```bash
cargo check -p app-chat
git add crates/app-chat/src/writer/ && git commit -m "feat(write-mode): research workers, material cards, orchestrator glue"
```

---

## Task 18: Progress events, metering, priming skill

**Files:** located by grep in each step

- [ ] **Step 1: SSE progress** — grep `AgentEventSink` variants (`agents/events.rs`); emit existing progress/status events at phase boundaries (`research`, `skeleton`, `drafting section k/N`, `refining round r`, `validating`). No new event types unless none fit.

- [ ] **Step 2: Metering** — grep the usage-observer API (`crates/llm/src/usage_observer.rs`, `crates/app-billing/src/usage_observer_impl.rs`). If calls carry a purpose/mode label, tag writer calls `write:<phase>`; otherwise record per-phase tokens in `WriterState` only and leave a `// TODO(metering)` with a pointer to the observer API.

- [ ] **Step 3: Priming skill** — grep how `writing-style` category skills are loaded (`agents/progressive/prompt_registry.rs`, `capability/registry.rs::answer_writing_styles`). Add the heavytail priming skill (content = the const from Task 11 step 3) with `applicable_strategies: ["write"]`; switch `draft.rs` to accept the priming text as a parameter supplied by the orchestrator from the skill slot (keep the const as fallback for the CLI bins).

- [ ] **Step 4: Verify + commit**

```bash
cargo check -p app-chat && cargo test -p app-chat
git add -A && git commit -m "feat(write-mode): progress events, metering labels, priming skill"
```

---

## Task 19: Contract test + workspace green

**Files:** `crates/app/tests/` (pattern from `unified_agent_contract.rs`), root workspace

- [ ] **Step 1: Contract test** — mirror `unified_agent_contract.rs` conventions for the write mode: request with `agent_type: "write"` and a stub/env-gated LLM produces a `WriteResult`-shaped response (non-empty text, fingerprint attached, citations well-formed when research ran). Env-gate the LLM-real variant; the stub variant must run in CI.

- [ ] **Step 2: e2e gate entry** — add the write-mode case to the product_e2e suite registry (grep `E2eSuite` in `crates/app/tests/product_e2e/e2e_gate.rs`) under the LLM-real mode, with artifacts (fingerprint + rounds) written per `test_context/artifacts.rs` conventions. Skip-by-default like other llm_real suites.

- [ ] **Step 3: Workspace green**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Fix fallout (mode-count assertions, exhaustive matches on `AgentKind`).

- [ ] **Step 4: Final commit**

```bash
git add -A && git commit -m "test(write-mode): contract test and e2e gate entry"
```

---

## Self-Review Checklist

- [x] **Spec coverage:**
  - §3 statistical foundation → Tasks 3, 4, 5 (trap avoided: no pointwise-likelihood API exists; all scoring goes through ensemble metrics)
  - §5 workspace/patch → Tasks 7, 8, 9
  - §6 mode integration → Tasks 16, 17, 18
  - §7–§10 stages → Tasks 11, 12 (arm C only), 14, 15
  - §11 validator → Task 15
  - §16 milestones → GATE 1 (Task 6), GATE 2 (Task 13), M4 exit (Task 15 step 4)
- [x] **Dependency direction:** `heavytail` depends only on `avrag-llm` + `common`; app-chat depends on `heavytail`; no cycle.
- [x] **Open questions resolved for MVP:** OQ1 (no global frequency table — relational rarity, documented in Task 2); OQ2 (fenced patch transport; tool-call variant deferred); OQ4 (decided empirically at GATE 2).
- [x] **YAGNI:** no statrs (2 hand-rolled approximations with error-bound tests), no clap, no external graph engine, no new event types unless required, arm-C code deletable at GATE 2.
- [x] **Placeholder scan:** every step has concrete files, commands, or a grep that locates the target; no TBD.
- [x] **Test strategy:** pure-Rust core is fully unit/property-tested and CI-safe; every LLM-touching test is env-gated; two human gates are explicit stop points.
