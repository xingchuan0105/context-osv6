# HeavyTail Writer Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:writing-plans` after spec approval.

**Goal:** Build a text generation pipeline that produces articles whose statistical fingerprint (sentence-length distribution, word-frequency distribution, burstiness) matches human writing patterns rather than AI defaults, by planning the target distributions upfront and enforcing them through a two-phase generate-then-correct loop.

**Architecture:** Two-phase hybrid control system — Phase A generates a draft in open-loop using a pre-planned sentence-length schedule (AR(1) log-space process) and vocabulary reservoir (Zipf-guided hapax injection); Phase B performs budget-driven greedy corrections (structural ops → synonym substitution → sentence rewrite) guided by a statistical fingerprint analyzer.

**Tech Stack:** Rust, jieba-rs (word tokenization), rand + statrs (distribution sampling), existing `crates/llm` (LLM client), existing `crates/common` (shared types).

---

## Table of Contents

1. [Context and Motivation](#1-context-and-motivation)
2. [Goals and Non-Goals](#2-goals-and-non-goals)
3. [Statistical Foundation](#3-statistical-foundation)
4. [Architecture Overview](#4-architecture-overview)
5. [Component: Content Planner](#5-component-content-planner)
6. [Component: Statistics Planner](#6-component-statistics-planner)
7. [Component: Phase A Generator](#7-component-phase-a-generator)
8. [Component: Analyzer](#8-component-analyzer)
9. [Component: Phase B Corrector](#9-component-phase-b-corrector)
10. [Component: Validator](#10-component-validator)
11. [Data Model](#11-data-model)
12. [Default Parameters](#12-default-parameters)
13. [Risks and Mitigations](#13-risks-and-mitigations)
14. [Implementation Milestones](#14-implementation-milestones)
15. [Open Questions](#15-open-questions)

---

## 1. Context and Motivation

### The Statistical Difference

Human writing and AI-generated text have fundamentally different statistical fingerprints:

| Feature | Human Writing | AI-Generated Text |
|---|---|---|
| Sentence length distribution | Log-normal (right-skewed, heavy-tailed) | Near-normal (symmetric, thin-tailed) |
| Coefficient of variation (CV) | 0.65–0.85 | 0.35–0.45 |
| Word frequency distribution | Zipf's law (power-law, heavy-tailed) | Flatter; rare words underused |
| Hapax legomena ratio | ~40–50% of word types appear exactly once | Significantly lower |
| Burstiness (length autocorrelation) | High (clusters of similar-length sentences) | Low (uniform alternation) |
| Perplexity | High (unpredictable word choices) | Low (predictable, regression to mean) |

### The Core Insight

AI's next-token prediction mechanism inherently regresses toward the mean — it selects statistically probable outputs, flattening the distribution. Human writing is a self-organizing system where thoughts, emotions, and rhythm interact, naturally producing heavy-tailed distributions.

### The Approach

This system does not attempt to change the LLM's generation mechanism. Instead, it **plans the target statistical shape upfront** (sentence lengths from a log-normal distribution, word frequencies from Zipf's law) and **enforces it through a two-phase control loop** — open-loop draft generation followed by budget-driven correction. This is analogous to algorithmic trading: a target trajectory is planned, executed open-loop, then rebalanced based on realized fills.

---

## 2. Goals and Non-Goals

### Goals

- Generate Chinese-language articles whose sentence-length distribution follows log-normal with human-range CV (0.65–0.85)
- Generate articles whose word-frequency distribution follows Zipf's law with human-range hapax ratio (~40–50%)
- Produce bursty sentence-length sequences (positive autocorrelation) matching human rhythm
- Controllable correction budget — Phase B cost is bounded
- Measurable output — every generated article gets a statistical fingerprint report

### Non-Goals

- Making the LLM write "better" content (argument quality, emotional depth, creativity) — this system shapes the statistical envelope, not the semantic quality
- Evading AI detectors as a primary goal — reducing statistical detectability is a side effect, not the purpose
- Supporting languages other than Chinese in the initial version (the tokenization layer is Chinese-specific)
- Real-time/streaming generation — the pipeline is batch-oriented (plan → generate → analyze → correct)

---

## 3. Statistical Foundation

### 3.1 Sentence Length: AR(1) Log-Space Process

Sentence lengths are modeled as a log-normal random variable with temporal autocorrelation, using an **AR(1) process in log-space** (discrete Ornstein-Uhlenbeck):

$$z_i = c + \varphi \cdot z_{i-1} + \varepsilon_i, \quad \varepsilon_i \sim \mathcal{N}(0, \sigma_\varepsilon^2)$$
$$l_i = \text{clamp}(\text{round}(e^{z_i}), \; l_{\min}, \; l_{\max})$$

where $z_i = \ln(l_i)$ is the log sentence length, $l_i$ is the target length in characters.

**Parameter derivation from human-range targets:**

The stationary distribution of $z$ is $\mathcal{N}(\mu_z, \text{Var}(z))$, giving:

- **CV control** (marginal distribution shape):
  $$\text{CV} = \sqrt{e^{\text{Var}(z)} - 1} \quad \Longrightarrow \quad \text{Var}(z) = \ln(1 + \text{CV}^2)$$

- **Burstiness control** (temporal autocorrelation):
  $$\text{Corr}(z_i, z_{i-1}) = \varphi$$

- **Innovation variance** (derived from the two above):
  $$\sigma_\varepsilon^2 = \text{Var}(z) \cdot (1 - \varphi^2)$$

- **Intercept and stationary mean**:
  $$\mu_z = \ln(\text{median\_length}), \quad c = \mu_z \cdot (1 - \varphi)$$

**Worked example** (CV=0.75, φ=0.4, median=20 chars):
```
Var(z)  = ln(1 + 0.75²) = 0.446
σ_ε²    = 0.446 × (1 − 0.16) = 0.375   →  σ_ε = 0.612
μ_z     = ln(20) = 3.0
c       = 3.0 × 0.6 = 1.8
```

**Burn-in:** Initialize $z_0 = \mu_z$ (stationary mean). No samples are discarded; the AR(1) process with this initialization is approximately stationary from step 1.

**Clamping:** $l_{\min} = 5$, $l_{\max} = 100$ characters. Clamped values remain in the sequence (they still contribute to the marginal distribution).

**Paragraph boundary handling:** The AR(1) process continues across paragraph boundaries (does not reset). This preserves cross-paragraph burstiness. Paragraph boundaries are determined by the Content Planner, not the Statistics Planner.

### 3.2 Word Frequency: Zipf's Law and Hapax Steering

Word frequencies in human text follow Zipf's law: $\text{freq}(r) \propto r^{-s}$, where $r$ is frequency rank and $s \approx 1$.

**The actionable metric is not the full Zipf curve but the hapax legomena ratio** — the proportion of word types that appear exactly once. In natural text, ~40–50% of distinct word types are hapax. AI text systematically underuses rare words, depressing this ratio.

**Steering mechanism:** Rather than attempting to control the frequency of every word (infeasible), the system maintains a reservoir of topic-appropriate rare words and injects them into sentence-level constraints when the running hapax ratio falls below target.

### 3.3 Burstiness Metric

Burstiness is measured as the lag-1 autocorrelation of sentence lengths:

$$B = \text{Corr}(l_i, l_{i-1})$$

Target: $B \in [0.2, 0.5]$ (moderate positive autocorrelation — clusters of similar-length sentences that eventually revert). The AR(1) parameter $\varphi$ directly controls this.

---

## 4. Architecture Overview

```
User Input: topic + target_word_count + StyleParams
    │
    ▼
┌──────────────────────────────────────────────────┐
│ 1. CONTENT PLANNER                                │
│    LLM generates article outline                  │
│    → ContentOutline                               │
└──────────────────┬───────────────────────────────┘
                   ▼
┌──────────────────────────────────────────────────┐
│ 2. STATISTICS PLANNER                             │
│    a. AR(1) sentence-length schedule              │
│    b. Vocabulary reservoir (Zipf-ranked)          │
│    → SentenceSchedule[] + VocabularyReservoir     │
└──────────────────┬───────────────────────────────┘
                   ▼
┌──────────────────────────────────────────────────┐
│ 3. PHASE A: Open-loop draft generation            │
│    Paragraph-by-paragraph, with per-sentence      │
│    length schedule + rare-word quota              │
│    → DraftText                                   │
└──────────────────┬───────────────────────────────┘
                   ▼
┌──────────────────────────────────────────────────┐
│ 4. ANALYZER                                       │
│    jieba tokenization → Zipf/hapax/TTR            │
│    char counting → length histogram/CV/autocorr   │
│    → FingerprintReport                            │
└──────────────────┬───────────────────────────────┘
                   ▼
┌──────────────────────────────────────────────────┐
│ 5. PHASE B: Budget-driven greedy correction       │
│    Layer 1 (zero cost): split/merge sentences     │
│    Layer 2 (low cost): synonym substitution       │
│    Layer 3 (high cost): sentence rewrite (≤ K)    │
│    → CorrectedText                                │
└──────────────────┬───────────────────────────────┘
                   ▼
┌──────────────────────────────────────────────────┐
│ 6. VALIDATOR                                      │
│    Re-analyze → compare to targets → pass/fail    │
│    fail → back to Phase B (≤ max_rounds)          │
└──────────────────────────────────────────────────┘
```

---

## 5. Component: Content Planner

### Responsibility

Generate the semantic structure of the article: sections, key points, argument flow. This is the content layer that the statistical plan is layered on top of.

### Interface

```rust
pub struct ContentOutline {
    pub title: String,
    pub sections: Vec<Section>,
}

pub struct Section {
    pub heading: String,
    pub key_points: Vec<String>,     // 2-5 key points per section
    pub paragraph_count: usize,       // suggested paragraphs
}

pub async fn plan_content(
    topic: &str,
    target_word_count: usize,
    llm_client: &LlmClient,
) -> Result<ContentOutline>;
```

### Process

1. Single LLM call with a structured prompt: topic, target word count, requested output format (JSON outline)
2. LLM produces section headings, key points, and suggested paragraph counts
3. Output is parsed into `ContentOutline`

### Notes

- The outline is purely semantic — no length or vocabulary constraints at this stage
- One LLM call total (cheap)
- User can optionally provide a pre-made outline, skipping this step

---

## 6. Component: Statistics Planner

### Responsibility

Generate the statistical plan: a per-sentence length schedule and a vocabulary reservoir, layered on top of the content outline.

### Interface

```rust
pub struct StatisticsPlan {
    pub schedule: Vec<SentenceSlot>,
    pub reservoir: VocabularyReservoir,
}

pub struct SentenceSlot {
    pub paragraph_idx: usize,
    pub content_intent: String,      // from ContentOutline
    pub target_length: usize,         // AR(1) sampled, in characters
    pub length_bin: LengthBin,        // for prompt translation
    pub rare_words: Vec<String>,      // words to inject this sentence (may be empty)
}

pub enum LengthBin {
    XShort,   // 5-10 chars
    Short,    // 10-20 chars
    Medium,   // 20-35 chars
    Long,     // 35-55 chars
    XLong,    // 55+ chars
}

pub struct VocabularyReservoir {
    pub rare_words: Vec<RankedWord>,  // topic words sorted by frequency rank
    pub hapax_target: f64,
}

pub struct RankedWord {
    pub word: String,
    pub freq_rank: usize,     // 1 = most common
}

pub fn plan_statistics(
    outline: &ContentOutline,
    style: &StyleParams,
) -> Result<StatisticsPlan>;
```

### Process

**Step 1: Sentence count estimation**

Estimate total sentence count from target word count and median sentence length:
```
N_sentences ≈ target_word_count / median_length
```

Distribute sentences across paragraphs based on the ContentOutline's paragraph_count per section.

**Step 2: AR(1) sentence-length sampling**

Using StyleParams, derive AR(1) parameters per Section 3.1:
```
var_z = ln(1 + cv²)
sigma_eps = sqrt(var_z * (1 - phi²))
mu_z = ln(median_length)
c = mu_z * (1 - phi)
z_0 = mu_z   // initialize at stationary mean
```

Iterate the AR(1) recurrence for N_sentences steps, producing `l_i = clamp(round(exp(z_i)), 5, 100)`.

Assign each length to a `LengthBin` based on character ranges.

**Step 3: Vocabulary reservoir construction**

1. Extract candidate words from the ContentOutline's headings and key points
2. Optionally: make one LLM call to expand the candidate list with topic-relevant terms (target: 100-200 words)
3. For each candidate word, look up its frequency rank using jieba-rs's dictionary (or a pre-built frequency table)
4. Sort by frequency rank; the tail (high rank = low frequency) forms the rare word pool

**Step 4: Rare word assignment**

Walk the sentence schedule. For each sentence, with probability proportional to how far the running hapax ratio is below target, assign 1-2 rare words from the reservoir (words not yet used).

```
hapax_deficit = max(0, hapax_target - running_hapax_ratio)
assignment_probability = min(1.0, hapax_deficit * k)   // k = sensitivity constant
if random() < assignment_probability:
    assign 1-2 unused rare words to this sentence
```

### Notes

- No LLM calls in steps 2-4 (pure computation). One optional LLM call in step 3 for vocabulary expansion.
- The AR(1) process is deterministic given a seed — results are reproducible.

---

## 7. Component: Phase A Generator

### Responsibility

Generate the article draft, paragraph by paragraph, following the statistical plan.

### Interface

```rust
pub async fn generate_draft(
    plan: &StatisticsPlan,
    outline: &ContentOutline,
    style: &StyleParams,
    llm_client: &LlmClient,
) -> Result<DraftText>;
```

### Process

For each paragraph (grouped by `paragraph_idx` in the schedule):

1. **Build the per-paragraph prompt:**
   ```
   [System] 你正在写一篇关于{topic}的文章。

   [Context] 已写内容：
   {accumulated_text_so_far}

   [Task] 请写第{paragraph_idx}段，共{n_sentences}句话。
   本段每句话的长度要求：
     第1句：{bin_description_1}（约{target_1}字）— 内容意图：{intent_1}
     第2句：{bin_description_2}（约{target_2}字）— 内容意图：{intent_2}
     ...

   {if any sentence has rare_words:}
   词汇要求：
     第{i}句请自然地包含以下词语：{rare_words_i}

   要求：句子之间语义连贯，内容自然流畅。
   ```

2. **`LengthBin` to description mapping:**
   | Bin | Range | Description |
   |-----|-------|-------------|
   | XShort | 5-10 | 极短，一个断句，干脆利落 |
   | Short | 10-20 | 短句，简洁有力 |
   | Medium | 20-35 | 中等长度，正常陈述 |
   | Long | 35-55 | 长句，包含从句或列举 |
   | XLong | 55+ | 极长，复合句，信息密集 |

3. **Generate** via LLM call, append to accumulated text.

4. **Post-process:** split the generated paragraph into sentences by Chinese sentence-ending punctuation (。！？), store as a list for the Analyzer.

### Cost model

- Number of LLM calls = number of paragraphs (~N_sentences / 5)
- Context grows linearly: paragraph $k$ receives paragraphs $1..k-1$ as context
- For a 50-sentence article: ~10 LLM calls

---

## 8. Component: Analyzer

### Responsibility

Compute the statistical fingerprint of a text. This is the measurement layer that drives Phase B corrections.

### Interface

```rust
pub struct FingerprintReport {
    // Sentence length dimension
    pub sentence_lengths: Vec<usize>,       // chars per sentence
    pub mean_length: f64,
    pub cv: f64,                              // coefficient of variation
    pub autocorr_lag1: f64,                   // burstiness
    pub lognormal_ks_stat: f64,              // KS test vs fitted log-normal

    // Word frequency dimension
    pub total_tokens: usize,                 // total word occurrences
    pub vocab_size: usize,                   // distinct word types
    pub ttr: f64,                             // type-token ratio
    pub hapax_ratio: f64,                    // proportion of types appearing exactly once
    pub zipf_exponent: f64,                  // fitted power-law exponent

    // Per-sentence detail (for Phase B targeting)
    pub sentences: Vec<AnalyzedSentence>,
}

pub struct AnalyzedSentence {
    pub text: String,
    pub char_count: usize,
    pub words: Vec<String>,                  // jieba tokenized
    pub planned_length: Option<usize>,       // from schedule, if available
    pub deviation: Option<f64>,              // (actual - planned) / planned
}

pub fn analyze(text: &str, plan: Option<&StatisticsPlan>) -> Result<FingerprintReport>;
```

### Process

**Sentence segmentation:** Split by Chinese sentence-ending punctuation (。！？；). Include the punctuation in the sentence. Count characters excluding whitespace.

**Word tokenization:** Use jieba-rs in default mode. Filter out punctuation tokens and whitespace.

**Metrics computation:**

- `cv = stddev(lengths) / mean(lengths)`
- `autocorr_lag1`: Pearson correlation between `(l_1, ..., l_{n-1})` and `(l_2, ..., l_n)`
- `lognormal_ks_stat`: Fit log-normal parameters via MLE on log(lengths), compute Kolmogorov-Smirnov statistic
- `ttr = vocab_size / total_tokens`
- `hapax_ratio = count(types with frequency == 1) / vocab_size`
- `zipf_exponent`: Fit `log(freq) ~ -s * log(rank)` via linear regression on the rank-frequency pairs

### Notes

- This component is fully deterministic (no LLM calls, no randomness)
- It is the first milestone (M1) — can be built and validated independently
- It can be used as a standalone CLI tool for analyzing any text

---

## 9. Component: Phase B Corrector

### Responsibility

Perform budget-driven greedy corrections on the draft to reduce deviations from the target distributions.

### Interface

```rust
pub struct CorrectionBudget {
    pub max_rewrites: usize,         // Layer 3 ceiling (sentence rewrites)
    pub max_token_cost: usize,       // total token budget for LLM calls
}

pub struct CorrectionResult {
    pub text: String,
    pub operations: Vec<CorrectionOp>,
    pub tokens_used: usize,
}

pub enum CorrectionOp {
    Split { sentence_idx: usize, at_clause: usize },
    Merge { sentence_idx: usize, with_next: usize },
    SynonymSub { sentence_idx: usize, original: String, replacement: String },
    Rewrite { sentence_idx: usize, old_length: usize, new_length: usize },
}

pub async fn correct(
    draft: &DraftText,
    report: &FingerprintReport,
    plan: &StatisticsPlan,
    style: &StyleParams,
    budget: &CorrectionBudget,
    llm_client: &LlmClient,
) -> Result<CorrectionResult>;
```

### Process: Three-Layer Greedy Correction

The corrector processes layers in order of cost. Each layer has its own deviation check and only executes if the relevant metric is still out of tolerance.

**Layer 1: Structural operations (zero LLM cost)**

Target: CV and burstiness deviations.

- If CV is too low (too uniform): identify the most "average" sentences (closest to mean) and split them at clause boundaries (，、；) into two shorter sentences. This injects short sentences into the distribution.
- If CV is too high: identify the shortest adjacent sentence pairs and merge them (join with appropriate conjunction).
- If burstiness (autocorr) is too low: within a paragraph, locally reorder sentences to create clusters of similar lengths (only if semantic flow permits — conservative heuristic: only reorder within a paragraph, never across).

Operations: `Split`, `Merge`. No LLM calls.

**Layer 2: Synonym substitution (low LLM cost)**

Target: hapax ratio and Zipf exponent deviations.

- If hapax ratio is below target: identify high-frequency common words (e.g., "的", "是", "可以", "这个" — but these are function words; target content words instead). Find content words that appear 2+ times. For each, look up a rarer synonym from the vocabulary reservoir or a synonym dictionary.
- Substitute one occurrence (not all) to create a new hapax.
- Context check: only substitute if the synonym is semantically valid in context (heuristic: jieba word similarity or a lightweight LLM check).

Operations: `SynonymSub`. Minimal LLM cost (context validation only).

**Layer 3: Sentence rewrite (high LLM cost, budget-bounded)**

Target: any remaining deviations, prioritized by severity.

- Rank sentences by their deviation from planned length, weighted by how much correcting them improves the global distribution fit.
- For the top-K sentences (within budget): regenerate with an adjusted length target (if too short, instruct "longer"; if too long, instruct "shorter") and a rare-word injection if hapax is still low.
- Each rewrite is one LLM call.

Operations: `Rewrite`. Cost = K LLM calls, bounded by `max_rewrites` and `max_token_cost`.

**Greedy selection:**

At each step, compute the marginal distribution improvement per unit cost for all available operations across all layers. Execute the highest-benefit-per-cost operation. Repeat until budget is exhausted or all metrics are within tolerance.

### Notes

- Phase B is the only component that requires iterative LLM calls
- The three-layer structure ensures free improvements are always exhausted before costly ones
- Corrections can cascade (a split introduces new words that may affect Zipf fit) — the Validator re-analysis catches this

---

## 10. Component: Validator

### Responsibility

Re-analyze the corrected text and compare against targets. Decide pass/fail.

### Interface

```rust
pub struct ValidationReport {
    pub fingerprint: FingerprintReport,
    pub passed: bool,
    pub metric_results: Vec<MetricCheck>,
}

pub struct MetricCheck {
    pub metric: String,            // e.g. "cv", "hapax_ratio"
    pub actual: f64,
    pub target: (f64, f64),        // (lower_bound, upper_bound)
    pub passed: bool,
}

pub fn validate(
    text: &str,
    style: &StyleParams,
    plan: Option<&StatisticsPlan>,
) -> Result<ValidationReport>;
```

### Tolerance bands

| Metric | Target range | Hard fail if |
|--------|-------------|-------------|
| CV | [style.cv × 0.85, style.cv × 1.15] | Outside [0.50, 1.00] |
| Hapax ratio | [0.35, 0.55] | < 0.30 |
| Burstiness (autocorr) | [0.1, 0.6] | < 0.0 or > 0.8 |
| Zipf exponent | [0.8, 1.3] | < 0.6 or > 1.5 |

A pass requires all metrics within their target ranges.

### Retry loop

If validation fails, return to Phase B with the remaining budget (reduced by tokens already used). Maximum `max_rounds` iterations (default: 3). If still failing after max rounds, return the best version (lowest total deviation) with a warning.

---

## 11. Data Model

### Core types

```rust
/// User-facing configuration
pub struct StyleParams {
    pub cv: f64,              // target coefficient of variation. Default: 0.75
    pub phi: f64,             // AR(1) autocorrelation (burstiness). Default: 0.4
    pub median_length: f64,   // target median sentence length in chars. Default: 20.0
    pub hapax_target: f64,    // target hapax legomena ratio. Default: 0.45
    pub zipf_exponent: f64,   // target Zipf power-law exponent. Default: 1.0
}

impl Default for StyleParams {
    fn default() -> Self {
        Self {
            cv: 0.75,
            phi: 0.4,
            median_length: 20.0,
            hapax_target: 0.45,
            zipf_exponent: 1.0,
        }
    }
}

/// Top-level pipeline input
pub struct WriteRequest {
    pub topic: String,
    pub target_word_count: usize,
    pub style: StyleParams,
    pub outline: Option<ContentOutline>,  // if None, Content Planner generates one
    pub budget: CorrectionBudget,
}

/// Top-level pipeline output
pub struct WriteResult {
    pub text: String,
    pub fingerprint: FingerprintReport,
    pub validation: ValidationReport,
    pub corrections: Vec<CorrectionOp>,
    pub total_llm_calls: usize,
    pub total_tokens: usize,
}
```

---

## 12. Default Parameters

| Parameter | Default | Rationale |
|---|---|---|
| `cv` | 0.75 | Midpoint of human range (0.65–0.85) |
| `phi` | 0.4 | Moderate burstiness; noticeable but not extreme clustering |
| `median_length` | 20 chars | Typical for Chinese expository prose |
| `hapax_target` | 0.45 | Midpoint of human range (0.40–0.50) |
| `zipf_exponent` | 1.0 | Canonical Zipf's law value |
| `l_min` | 5 chars | Minimum meaningful Chinese sentence |
| `l_max` | 100 chars | Upper bound for readable sentences |
| `max_rewrites` (Layer 3) | 30% of sentences | Balance correction power vs cost |
| `max_rounds` (Validator) | 3 | Diminishing returns beyond 3 rounds |

---

## 13. Risks and Mitigations

### Risk 1: Statistical fingerprint ≠ human quality

**Description:** Forcing log-normal lengths and Zipf frequencies treats statistical symptoms, not semantic causes. The text may pass statistical checks but still feel artificial in argument quality, emotional depth, or logical coherence.

**Mitigation:** This system is explicitly scoped as a "statistical envelope shaper," not a quality engine. The Content Planner handles semantic structure independently. Document this limitation clearly in user-facing output.

### Risk 2: Rare word injection creates uncanny valley

**Description:** Injecting low-frequency words into sentences where they don't naturally belong produces a different kind of unnaturalness — forced vocabulary.

**Mitigation:**
- Vocabulary reservoir only contains topic-appropriate words (extracted from the outline, not random)
- Layer 2 synonym substitution requires context validation before applying
- Rare word quotas are probabilistic, not deterministic (some sentences get them, most don't)
- Phase A instructions say "自然地包含" (naturally include), not "必须使用" (must use)

### Risk 3: Phase B corrections cascade

**Description:** Splitting a sentence changes word frequencies. Rewriting a sentence may introduce new common words. Corrections interact non-linearly.

**Mitigation:**
- Three-layer greedy approach processes free operations first, minimizing cascade surface
- Validator re-analyzes after each round, catching cascading effects
- Max rounds bound prevents infinite oscillation
- Best-version retention: if correction makes things worse, revert to the previous version

### Risk 4: AR(1) clamping distorts the distribution

**Description:** Clamping lengths to [5, 100] truncates the log-normal tails, slightly reducing the CV from the theoretical value.

**Mitigation:** With median=20 and CV=0.75, the probability of exceeding 100 chars is < 2%. Clamping has negligible effect on the distribution shape. If median_length is set much higher (>40), widen l_max accordingly.

### Risk 5: jieba tokenization instability for new/technical terms

**Description:** jieba may split domain-specific terms inconsistently (e.g., "区块链" as one token vs "区块" + "链"), affecting word frequency statistics.

**Mitigation:** Add a custom dictionary for domain terms when available. The Analyzer should report tokenization confidence. This is a known limitation — absolute Zipf values may shift but relative comparisons (human vs AI) remain valid.

---

## 14. Implementation Milestones

### M1: Analyzer (validate hypothesis)

**Goal:** Build the Analyzer component as a standalone tool. Verify that human and AI texts actually differ in the expected statistical dimensions.

**Deliverables:**
- `analyze()` function: jieba tokenization, sentence segmentation, all metrics
- CLI tool: `heavytail-analyze <input.txt>` → prints FingerprintReport
- Validation dataset: collect 10+ human-written articles and 10+ AI-generated articles on similar topics
- Comparison report: do CV, hapax ratio, burstiness, and Zipf exponent actually differentiate human from AI?

**Exit criterion:** If the metrics do not differentiate human from AI text as expected, stop and re-evaluate the premise before proceeding.

### M2: Statistics Planner + Phase A Generator (minimum closed loop)

**Goal:** Plan a statistical schedule and generate a draft. Measure how far the open-loop draft deviates from targets.

**Deliverables:**
- AR(1) sentence-length scheduler
- Vocabulary reservoir builder
- Phase A paragraph-level generator (using existing LLM client)
- End-to-end: topic → plan → draft → analyze → report deviations

**Exit criterion:** Understand the magnitude of Phase A deviations. This determines how heavy Phase B needs to be.

### M3: Phase B Corrector + Validator (full pipeline)

**Goal:** Complete the correction loop. End-to-end pipeline with measurable distribution shaping.

**Deliverables:**
- Three-layer greedy corrector (split/merge → synonym sub → rewrite)
- Validator with tolerance bands and retry loop
- End-to-end: topic → full pipeline → corrected text with fingerprint report
- A/B comparison: same topic, with and without HeavyTail Writer, compare fingerprints

**Exit criterion:** Pipeline consistently produces text with CV, hapax ratio, and burstiness within human-range tolerance bands.

---

## 15. Open Questions

1. **Vocabulary reservoir word frequency source:** jieba-rs's built-in dictionary has IDF values but not absolute frequency counts. Need to determine: use jieba IDF as a proxy for frequency rank, or build a separate frequency table from a reference corpus (e.g., BLCU corpus, BCC corpus)?

2. **Synonym source for Layer 2:** Options include: (a) a Chinese synonym dictionary (cilin / 同义词词林), (b) LLM-generated synonyms per call (accurate but costly), (c) pre-computed embeddings similarity. Trade-off between accuracy and cost.

3. **Sentence reordering for burstiness:** Phase B Layer 1 includes conservative within-paragraph reordering to improve burstiness. How aggressive should this be? Need to balance statistical improvement against semantic flow disruption. Potentially: disable by default, enable as an option.

4. **Paragraph-level context management:** As the article grows, passing the full accumulated text as context for each paragraph may exceed context windows for long articles (>3000 chars). Need a context summarization or sliding-window strategy for Phase A.

5. **Evaluation framework:** How to quantitatively evaluate whether the output "feels" more human? Beyond statistical metrics, should we integrate an external AI-detector score as a secondary validation metric?
