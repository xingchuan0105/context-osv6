# HeavyTail Writer v2 — Feedback-First Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:writing-plans` after spec approval.

**Supersedes:** [`2026-07-06-heavytail-writer-design.md`](./2026-07-06-heavytail-writer-design.md) (v1, feedforward). v1 stays in-tree as the specification for experiment arm C (§16, M3). Do not implement v1's Phase B or Validator standalone.

**Amended by:** [`2026-07-07-write-refine-agent-loop.md`](../../plans/2026-07-07-write-refine-agent-loop.md) — §10 精修段改为 `AgentKind::WriteRefine` + 三 tool call + loop 内 research（≤5）+ 软结束。

**Goal:** Same statistical goal as v1 — generate Chinese articles whose fingerprint (log-normal sentence lengths with human-range CV, Zipf/hapax vocabulary, positive burstiness) matches human writing — achieved by a **feedback-first** pipeline: freely-drafted text is measured by a deterministic analyzer, repaired through localized sentence-anchored patch directives executed by the LLM, and grounded in retrieved material (corpus RAG + web search) that supplies rare vocabulary naturally.

**Architecture:** A writer orchestrator (deterministic state machine, ~7 nodes, 1 cycle) exposed to users as a `write` mode. Research runs the existing react loop as parallel subagents-as-tools returning compressed material cards. Drafting is skeleton-guided, section-by-section, in free prose, with running-fingerprint feedback shaping each next section brief (MPC style). Refinement is an evaluator-optimizer loop: the evaluator is the deterministic Analyzer + Directive Compiler (quantile-matching sensitivity analysis); the optimizer is the LLM applying patch-only anchored edits to a line-addressed draft workspace.

**Tech Stack:** Rust; jieba-rs (tokenization); statrs + rand (distributions); existing `crates/llm` (LLM client); existing `app-chat` agents react loop (as research subroutine); `contracts` (patch / tool schemas).

---

## Table of Contents

1. [Why v2: What Changed From v1](#1-why-v2-what-changed-from-v1)
2. [Goals and Non-Goals](#2-goals-and-non-goals)
3. [Statistical Foundation: Distribution Matching](#3-statistical-foundation-distribution-matching)
4. [Architecture Overview](#4-architecture-overview)
5. [Draft Workspace: Text as Code](#5-draft-workspace-text-as-code)
6. [Mode Integration](#6-mode-integration)
7. [Stage 1: Research](#7-stage-1-research)
8. [Stage 2: Skeleton](#8-stage-2-skeleton)
9. [Stage 3: Sectionwise Drafting (MPC)](#9-stage-3-sectionwise-drafting-mpc)
10. [Stage 4: Refinement Loop](#10-stage-4-refinement-loop)
11. [Validator and Exit Policy](#11-validator-and-exit-policy)
12. [Data Model](#12-data-model)
13. [Orchestration Patterns and Reuse Map](#13-orchestration-patterns-and-reuse-map)
14. [Default Parameters](#14-default-parameters)
15. [Risks and Mitigations](#15-risks-and-mitigations)
16. [Implementation Milestones](#16-implementation-milestones)
17. [Open Questions](#17-open-questions)

---

## 1. Why v2: What Changed From v1

v1 planned target distributions upfront (AR(1) per-sentence length schedule, per-sentence rare-word quotas) and enforced them with mechanical corrections (string-level split/merge, dictionary synonym substitution). Four findings from design review invalidate the feedforward emphasis:

### 1.1 The repair problem is low-dimensional

Heavy tails are made in the tails, not the bulk. Worked example: a 50-sentence AI draft with mean 20 chars, stddev 8 (CV ≈ 0.4). Introduce just 6 extremes — 4 sentences at ~5 chars, 2 at ~70 chars:

```
new mean  ≈ (44×20 + 4×5 + 2×70) / 50 = 20.8
new var   ≈ [4×(5−20.8)² + 2×(70−20.8)² + 44×~64.6] / 50 ≈ 174
new CV    ≈ 13.2 / 20.8 ≈ 0.63
```

A handful more edits reaches the 0.65–0.85 band. Scheduling all \(n\) sentences plans ~\(n\) degrees of freedom where fewer than 10 matter. Targeted post-hoc surgery is cheaper and equally effective for the length dimension.

### 1.2 Deterministic targeting, LLM actuation

Neither extreme of the feedback spectrum works:

- Scalar feedback ("raise CV to 0.75") is not executable — global metrics give the LLM no local gradient.
- Per-sentence numeric feedforward ("write exactly 23 chars") is not executable either — LLMs cannot count characters, and every free rewrite re-applies the model's mean-reversion bias.

The workable division: **deterministic code computes which sentence, what change, and the expected gain; the LLM executes the prose edit.** (12-Factor Agents: "own your control flow.")

### 1.3 Structure for reading and editing, not for writing

Forcing prose generation inside structured formats degrades fluency and, worse, nudges every sentence toward self-contained medium length — the exact pathology being fought. The asymmetric protocol: draft in free prose → system canonicalizes into a line-addressed, ID'd form → refinement rounds read the canonical form and emit **patch-only** output. Freeze of untouched text becomes structural: the model cannot modify sentences it does not emit.

### 1.4 Grounded material attacks the root cause of thin tails

Human writing is heavy-tailed partly because it is anchored in specifics — names, figures, terms, quotes — which are inherently low-frequency tokens. AI text is thin-tailed partly because it writes from parametric averages. Retrieval (user corpus + web) supplies specifics naturally; vocabulary steering then only handles residuals. This defuses v1 Risk 2 (uncanny-valley word injection) at the source and is why research integration is a statistical feature, not a bolt-on.

### Carried over from v1 unchanged

- Analyzer metric definitions (sentence segmentation, CV, lag-1 autocorrelation, log-normal KS, TTR, hapax ratio, Zipf exponent)
- `StyleParams` and its defaults
- Validator tolerance bands
- M1's human-vs-AI validation gate (if the metrics do not differentiate, stop)

---

## 2. Goals and Non-Goals

### Goals

- All v1 statistical goals: CV ∈ 0.65–0.85, hapax ≈ 0.40–0.50, positive burstiness, Zipf-consistent vocabulary; per-article fingerprint report.
- **Grounded output**: articles cite material gathered from the user's corpus (RAG) and the web, reusing the existing citation pipeline.
- **Bounded, observable iteration**: every phase checkpointed to artifacts; refinement budgeted per round and per job; best-version retention.
- **Reuse-first**: the react loop, capability registry, parse-retry, SSE progress, billing metering, citation filtering, and artifact conventions are reused, not reimplemented.

### Non-Goals

- Semantic quality engineering (argument depth, creativity) — this system shapes the statistical envelope and grounding, not rhetoric.
- AI-detector evasion as a primary goal (side effect only).
- Non-Chinese languages in the initial version.
- Real-time streaming generation of the final article (progress events stream; text is batch).
- Parallel section drafting — sections are sequential by design (running-fingerprint feedback requires it); only research parallelizes.
- Implementing the writer as a fourth react-loop mode (see §6.1).

---

## 3. Statistical Foundation: Distribution Matching

### 3.1 The pointwise-likelihood trap

The naive way to "localize" a distribution score — give each sentence the log-density \(\log f(l_i)\) under the target log-normal and greedily fix low scorers — **degenerates**: each term is maximized at the distribution's mode, so greedy optimization drives every sentence toward one length and CV to zero. Distribution match is a property of the ensemble, not of points. Any per-sentence attribution must be defined against an ensemble statistic. This also rules out "sentence \(i\) is improbable, fix it" heuristics.

### 3.2 Quantile matching (Wasserstein-1)

Sort the \(n\) sentence lengths and match them to the target log-normal's quantiles:

$$q_{(i)} = \exp\!\left(\mu_z + \sigma_z \, \Phi^{-1}\!\left(\tfrac{i-0.5}{n}\right)\right), \qquad \mu_z = \ln(\text{median\_length}), \quad \sigma_z = \sqrt{\ln(1+\text{CV}^2)}$$

The length-dimension distance is the empirical Wasserstein-1:

$$W_1 = \frac{1}{n}\sum_{i=1}^{n} \left| l_{(i)} - q_{(i)} \right|, \qquad \widehat{W_1} = \frac{W_1}{\mathbb{E}[l]} = \frac{W_1}{\text{median} \cdot e^{\sigma_z^2/2}}$$

This construction yields three things at once:

- **Per-sentence attribution**: the sentence at sorted position \(i\) contributes \(|l_{(i)} - q_{(i)}|\).
- **Constructive targets**: the optimal new length for that sentence is its matched quantile \(q_{(i)}\) — the compiler does not merely score a proposed \(X\), it derives the best \(X\).
- **Dense signal**: unlike the KS statistic (max-based; tail improvements register nothing until the argmax crosses), every improvement moves \(W_1\). KS is retained as a reporting metric only, never for targeting.

Worked targets (median = 20, CV = 0.75, n = 50, \(\sigma_z = 0.668\), clamped to [5, 100]):

| sorted position | 1 | 5 | 13 | 25 | 38 | 45 | 49 | 50 |
|---|---|---|---|---|---|---|---|---|
| target chars | 5 | 8 | 13 | 20 | 31 | 45 | 70 | 95 |

A typical AI draft's sorted lengths span ~12–35: the gaps concentrate at both ends, so the sensitivity table automatically points at the tails first.

### 3.3 Exact sensitivity analysis by enumeration

At this scale (\(n \le 200\) sentences, vocabulary \(\le\) ~1500 types) **every candidate edit's Δscore is computed by exact recomputation — no gradients, no approximations**. A full sensitivity table (each sentence × a log-spaced candidate-length grid `{5, 8, 12, 16, 20, 26, 34, 44, 56, 72, 90}`) costs \(O(n \cdot |grid| \cdot n\log n)\) — microseconds in Rust. The same holds for vocabulary operations against the full frequency table.

Batch planning per round: assign sorted lengths to sorted quantiles, select the K largest gaps as edit candidates, recompute everything after the round is applied. Sentence count changes (splits/merges) simply re-derive the quantile targets for the new \(n\).

### 3.4 Word frequency: operation enumeration

The actionable levers are discrete operations, each with an exactly recomputable effect:

**Promote-to-hapax.** Replace one occurrence of a content word with in-draft frequency exactly 2 by an unused reservoir word. Effect: the demoted word drops to frequency 1 (+1 hapax) and the new word enters at frequency 1 (+1 hapax); vocabulary +1. Net: hapax types +2 per operation — the cheapest lever. Frequency-3 words are the secondary pool (+1 hapax each).

Deficit math: at ~350 types with hapax ratio 0.31 (≈109 hapax), reaching 0.45 requires solving \((109+2k)/(350+k) = 0.45\) → \(k \approx 30\) operations. This is diffuse (dozens of small edits across the text), so it is executed as **one whole-text lexical pass** with an analyzer-computed word list — never as per-sentence quotas.

**Demote-overused.** Words whose in-draft frequency greatly exceeds the Zipf expectation for their rank (AI tics like 此外/总而言之 surface here) get a reduce-to-≤-target directive; the LLM chooses the replacement or elision.

**Reservoir priority.** Rare-word candidates come from material cards first (real terms from sources, §6.3), LLM topic expansion second. Grounded terms carry no uncanny-valley risk.

### 3.5 Burstiness as placement

Quantile matching fixes the **multiset** of target lengths; lag-1 autocorrelation is determined by **which position receives which length**. The compiler assigns targets to positions so that similar lengths cluster within paragraphs, minimizing the number of edited sentences subject to the planned sequence's autocorrelation landing in [0.2, 0.5]. Greedy/hill-climbing assignment suffices at this scale.

Burstiness is communicated to the LLM only as **per-paragraph rhythm modes** (`short-burst` / `long-flow` / `mixed`) plus per-sentence anchors — never as a correlation coefficient. Human burstiness arises from passage-level mode switching; the directive language mirrors that.

### 3.6 Composite score

$$S = w_1 \cdot s_{\text{len}} + w_2 \cdot s_{\text{burst}} + w_3 \cdot s_{\text{hapax}} + w_4 \cdot s_{\text{zipf}}$$

Each component \(s_m \in [0,1]\) is a band score: 1.0 inside the target band (§11), decaying linearly to 0 at the hard-fail bound. Default weights: 0.4 / 0.2 / 0.25 / 0.15. Uses:

- **Greedy op selection**: rank candidate operations by \(\Delta S\) per token cost.
- **Validator**: pass requires every metric inside its band (S is reported, not the pass criterion).
- **Best-version retention**: the round with the highest S is kept if the loop exhausts its budget.

Requirements on the functional form: monotone, bounded, exactly recomputable. The precise ramp shape is an implementation detail.

---

## 4. Architecture Overview

```
User input: topic + StyleParams (+ optional corpus scope, outline)
    │   mode: "write"  (ModeSchema: requires_internet — routed to the
    │                   orchestrator at the agent-dispatch seam, NOT the react loop)
    ▼
┌────────────────────────────────────────────────────────────────────┐
│ WRITER ORCHESTRATOR — deterministic state machine, checkpointed    │
│                                                                    │
│  [1] RESEARCH        parallel subagents-as-tools (existing react   │
│      │               loop, modes `rag` + `search`, unchanged)      │
│      │               → MaterialCards (compressed, cited)           │
│      ▼                                                             │
│  [2] SKELETON        1 LLM call → sections, key points,            │
│      │               rhythm modes, card assignments                │
│      ▼                                                             │
│  [3] DRAFT (per section, sequential)                               │
│      │   brief = skeleton[k] + cards + running stats deficit (MPC) │
│      │   free-prose LLM call → canonicalizer → DraftWorkspace      │
│      ▼                                                             │
│  [4] REFINE (loop ≤ max_rounds)                                    │
│      │   Analyzer → sensitivity table → Directive Compiler         │
│      │   → rhythm patch pass → lexical patch pass                  │
│      │   → parse/verify/splice → re-analyze                        │
│      ▼                                                             │
│  [5] VALIDATE        bands pass → done │ fail → loop │             │
│                      budget exhausted → best version + warning     │
└────────────────────────────────────────────────────────────────────┘
    ▼
WriteResult { text, fingerprint, citations, rounds, tokens }
```

Control flow is owned entirely by the orchestrator. LLM calls never decide the next phase; the LLM's only choices are prose. Phase transitions, budgets, and retries are `match` arms on `WriterState`, following the existing loop's `STATE_MACHINE.md` discipline.

---

## 5. Draft Workspace: Text as Code

The draft is a line-addressed artifact: sentences are the unit of addressing, edits are patches, the validator is CI.

### 5.1 Canonical form

```
# p1 | rhythm: short-burst
s01| 深夜的交易大厅安静得反常。
s02| 屏幕还亮着。
s03| 风险引擎在凌晨两点十七分弹出第一条告警，没有人注意到它。
# p2 | rhythm: long-flow
s04| ...
```

Rules:

- One sentence per line; line format `<id>| <text>`.
- Paragraph headers `# p<K> | rhythm: <mode>` carry paragraph identity and rhythm mode.
- Produced by the **canonicalizer** from free prose: deterministic segmentation on 。！？ (closing-quote aware; ； configurable), whitespace-stripped char counts. The LLM never writes this format during drafting.

### 5.2 ID stability — never renumber

- **Split**: `s07` → `s07a`, `s07b` (suffix inheritance; lexicographic order = document order among siblings; nesting allowed: `s07aa`). Parent ID is tombstoned.
- **Merge**: surviving sentence keeps the first ID; the merged-away ID is tombstoned (retained in state, never reused, must not reappear).
- Paragraph membership is a tag on the sentence record, not encoded in the ID.
- Cross-round references (directives, compliance records, patch history) therefore never dangle.

### 5.3 Patch grammar — structural freeze

Refinement passes emit **patch-only** output:

```
s03a| 没有人注意到它。
s03b| 风险引擎在凌晨两点十七分弹出第一条告警，所有人的注意力都被另一件事占据了。
s09| （改写后的 55–70 字复合长句……）
s12| （将其中一处"影响"替换为更低频表达后的原句……）
```

Parse rules (whole patch rejected on any violation → retry via the existing parse-retry path):

1. Every line matches `^(s[0-9]+[a-z]*)\| `.
2. Every emitted ID ∈ this round's **allow-set** = directive-named IDs ∪ declared split children. Tombstoned IDs and unlisted IDs are forbidden.
3. Each line contains exactly one sentence (single terminal punctuation, at end).

Splice semantics: replace by ID in place; split children insert at the parent's position; merge removes the tombstoned line. Untouched sentences are never re-emitted, so they **cannot** change — freeze is structural, not verified. (A byte-diff assert over untouched IDs runs anyway as belt-and-suspenders.)

Skipping a directive is tolerated (recorded as non-compliance, recompiled next round); emitting outside the allow-set is not.

Transport: fenced patch block is the v2 default (best prose fidelity, human-debuggable). A `contracts` tool-call variant (`[{id, text}]`) is the alternative; revisit after M4 (§17).

### 5.4 Files and checkpoints

Per-job artifact directory:

```
job-<id>/
  state.json                 # WriterState checkpoint (rewritten after each phase/round)
  material_cards.json
  skeleton.json
  article.draft              # canonical form, current best
  round-1.fingerprint.json
  round-1.directives.json
  round-1.patch.txt
  round-2....
```

Best-version is a pointer into round history. Any phase is resumable from `state.json`. Conventions follow the worker pipeline / e2e artifact style (`document_pipeline`, `test_context/artifacts.rs`).

---

## 6. Mode Integration

### 6.1 Two-level design: mode as entry, orchestrator as body

`write` is registered in the capability registry as a `ModeSchema` (`requires_internet: true`, `external_tools_used: ["web_search"]`) and appears to users as a peer of `chat`/`rag`/`search`. It is **not** a fourth react-loop `ModeConfig`: the react loop's shape (retrieve ≤ 4 iterations → evidence exit policy → single synthesis with citation contract) cannot host a multi-phase, stateful, 10–20-call pipeline whose control decisions must be deterministic. Dispatch happens at the same seam where `agent_type` currently selects the react-loop runner; `write` routes to `WriterOrchestrator` instead.

### 6.2 Research workers: reuse existing modes unchanged (MVP)

MVP runs two parallel workers-as-tools with the **existing `rag` and `search` ModeConfigs untouched**:

- corpus worker: react loop in `rag` mode over the user's notebooks/documents;
- web worker: react loop in `search` mode.

Each worker gets an isolated context, a per-worker token budget, focused queries from the orchestrator (§7), and returns its normal cited answer, which the orchestrator compresses into MaterialCards. This avoids new loop plumbing entirely.

Deferred to v2.1: a combined `modes/write-research.yaml` with a mixed tool pool (corpus retrieval bridge + `web_search`/`web_fetch`) enabling cross-source reasoning in one context ("web claims X — does my corpus confirm?"). Blocker: `auto_fallback` currently assumes a single fallback tool; extending it is the price of admission (§17).

### 6.3 Material cards

```rust
pub struct MaterialCard {
    pub id: String,                  // "m01"...
    pub kind: MaterialKind,          // Fact | Quote | Figure | Term | Inspiration
    pub content: String,             // compressed claim/quote, ≤ ~80 chars
    pub source: SourceRef,           // existing citation type
    pub section_hint: Option<String>,
    pub rare_terms: Vec<String>,     // low-frequency vocabulary extracted from this card
}
```

Cards are the only channel between research and writing (blackboard discipline — no transcript passing). They feed four consumers: skeleton (section evidence), drafting briefs, the vocabulary reservoir (`rare_terms` outrank LLM-invented candidates), and final citations (reusing the `filter_citations_for_mode` path). Web-origin card content passes `untrusted_input` / `content_guard` before entering any prompt.

### 6.4 Billing, telemetry, progress

- Metering: per-phase labels on the LLM usage exit metering pipeline (migration 0050); the writer is a token-metered category — per-iteration quotas of `ReactLoopAgentMode` do not apply to the orchestrator (workers meter under their own modes with caps).
- Progress: phase / section-k / round-N events over the existing SSE progressive sink.
- Style priming: the drafting system prompt is a `writing-style` skill in the capability registry's existing slot (`answer_writing_styles`), applicable-strategy `write`.

---

## 7. Stage 1: Research

**Input:** topic, optional corpus scope. **Output:** 10–30 MaterialCards + reservoir seed.

1. Query synthesis: 2–4 focused queries per worker from a template over the topic (one optional LLM call if the topic is broad).
2. Fan-out: `tokio::JoinSet`, both workers in parallel, per-worker timeout and token cap.
3. Join and compress: worker answers + citations → MaterialCards (one cheap LLM call per worker output, or rule-based extraction for MVP); `rare_terms` extracted per card (jieba + frequency-table lookup: keep tokens below a frequency-rank threshold).
4. Degradation: if one worker fails or returns nothing, proceed single-source with a `research_degraded` warning on the result. Research never hard-fails the job; with zero cards the pipeline runs ungrounded (statistically weaker, still valid).

Parallelism exists **only** here — the one phase with genuinely independent subtasks (multi-agent token economics: ~15× chat cost; spend it only where parallel value is real).

---

## 8. Stage 2: Skeleton

One LLM call (JSON contract, existing parse-retry) producing:

```rust
pub struct Skeleton {
    pub title: String,
    pub sections: Vec<SkeletonSection>,
}

pub struct SkeletonSection {
    pub heading: String,
    pub key_points: Vec<String>,        // 2–5
    pub card_refs: Vec<String>,         // MaterialCard ids
    pub target_chars: usize,            // section length budget
    pub paragraphs: Vec<ParagraphPlan>, // rhythm mode per paragraph
}

pub struct ParagraphPlan {
    pub rhythm: RhythmMode,             // ShortBurst | LongFlow | Mixed
}
```

A user-supplied outline skips the call; cards are still attached by similarity of `section_hint`/heading.

---

## 9. Stage 3: Sectionwise Drafting (MPC)

Sections are drafted sequentially in **free prose** — no IDs, no per-sentence constraints. Per-section brief:

```
[System]  writing-style priming skill (长短交错；不避 10 字以内的极短句；
          偶用 50 字以上的复合长句；少用高频套话；优先具体名词/数字/术语)

[Context] skeleton (always pinned) + last section verbatim
          + one-line summaries of older sections

[Material] assigned MaterialCards for this section (content + source tag)

[Task]    write section k: heading, key points, ~target_chars,
          paragraph rhythm modes
          + deficit hints (≤ 3, from running fingerprint), e.g.:
            - 目前全文短句偏少：本段安排至少两句 10 字以内的短句
            - 可自然使用的词：<3–5 reservoir terms>
```

After each section: canonicalize → append to DraftWorkspace → incremental fingerprint update → next brief's deficit hints. This is receding-horizon control at section granularity — cheap steering during generation so refinement handles residuals only. Whether deficit hints measurably help is an explicit M3 question; they are trivially removable.

Context strategy for long articles: skeleton pinned verbatim, previous section verbatim, older sections as one-line summaries. Cost: 4–8 calls for a typical article.

---

## 10. Stage 4: Refinement Loop

Each round (≤ `max_rounds`):

**1. Analyze.** Full fingerprint on the canonical draft.

**2. Compile directives (deterministic).**

- Batch quantile assignment (§3.2) → per-sentence gaps; placement pass (§3.5) decides which positions take which targets.
- **Rhythm ops** (≤ `max_rhythm_ops`, ranked by ΔS per token cost):
  - `SPLIT s | children: sA, sB | short side: <bin>` — preferred way to mint short sentences (content-preserving);
  - `MERGE s, t | conjunction hint` — preferred way to mint long sentences from adjacent shorts;
  - `EXTEND s | target bin | optional card/term to weave in` — grounded lengthening;
  - `REWRITE s | target bin` — last resort only.
- **Lexical ops** (≤ `max_lexical_ops`): promote list (freq-2 words × reservoir replacements, §3.4) + demote list, each op naming the sentence it lives in.
- **Overshoot**: directive targets push ~30% beyond the computed target toward the extreme (`overshoot = 1.3`) to compensate for the actuator's regression to the mean.
- Length targets are expressed as bins/ranges (约X字 / X字以内), never exact counts.

**3. Rhythm pass.** One LLM call: canonical draft + rhythm directives → patch → parse/verify/splice (§5.3).

**4. Lexical pass.** One LLM call: updated draft + lexical directives → patch → parse/verify/splice. (Two passes because heterogeneous constraint piles degrade compliance.)

**5. Re-analyze.** Record per-directive compliance (asked vs achieved). Non-complied directives are **recompiled from the new state** next round — re-target, don't argue. Retain best version by S.

Prompt layout keeps a stable prefix (system + canonical draft) and variable suffix (directives) — prefix-cache friendly across the round's two passes.

Cost: 2 calls × ≤ `max_rounds`, patch-sized outputs.

---

## 11. Validator and Exit Policy

Bands carried from v1:

| Metric | Target band | Hard fail |
|---|---|---|
| CV | [style.cv × 0.85, style.cv × 1.15] | outside [0.50, 1.00] |
| Hapax ratio | [0.35, 0.55] | < 0.30 |
| Burstiness (lag-1 autocorr) | [0.1, 0.6] | < 0.0 or > 0.8 |
| Zipf exponent | [0.8, 1.3] | < 0.6 or > 1.5 |

- Pass = all metrics in band → finalize (strip IDs, render paragraphs, attach citations).
- Fail → next refinement round with remaining budget.
- Rounds exhausted → best version by S, `validation_warning` set.
- Directive compliance rate is report-only, never a pass criterion.

Note: hapax/TTR are text-length dependent (Heaps' law); bands assume articles in the ~800–3000 char range. Outside that, bands need recalibration (§17).

---

## 12. Data Model

```rust
/// Carried from v1 unchanged.
pub struct StyleParams {
    pub cv: f64,              // default 0.75
    pub phi: f64,             // retained for arm-C experiments; unused by v2 runtime
    pub median_length: f64,   // default 20.0
    pub hapax_target: f64,    // default 0.45
    pub zipf_exponent: f64,   // default 1.0
}

pub struct WriteRequest {
    pub topic: String,
    pub target_word_count: usize,
    pub style: StyleParams,
    pub outline: Option<Skeleton>,
    pub corpus_scope: Option<CorpusScope>,   // notebook/document filter for the rag worker
    pub budget: WriterBudget,
}

pub struct WriterBudget {
    pub research_tokens_per_worker: usize,
    pub max_rounds: usize,
    pub max_rhythm_ops: usize,
    pub max_lexical_ops: usize,
    pub total_token_cap: usize,
}

pub enum WriterPhase {
    Research, Skeleton, Drafting { section: usize },
    Refining { round: usize }, Validating, Done, Failed,
}

pub struct WriterState {
    pub phase: WriterPhase,
    pub cards: Vec<MaterialCard>,
    pub skeleton: Option<Skeleton>,
    pub workspace: DraftWorkspace,
    pub rounds: Vec<RoundRecord>,           // fingerprint + directives + compliance
    pub best: Option<BestVersion>,          // round index + S
    pub tokens_used: usize,
}

pub struct DraftWorkspace {
    pub sentences: Vec<SentenceRecord>,
    pub paragraphs: Vec<ParagraphRecord>,   // id, rhythm mode
}

pub struct SentenceRecord {
    pub id: SentenceId,                     // "s07", "s07a", ...
    pub text: String,
    pub para: ParagraphId,
    pub tombstone: bool,
}

pub enum Directive {
    Split { id: SentenceId, children: (SentenceId, SentenceId), short_bin: LengthBin, expected_gain: f64 },
    Merge { keep: SentenceId, absorb: SentenceId, expected_gain: f64 },
    Extend { id: SentenceId, bin: LengthBin, weave: Option<String>, expected_gain: f64 },
    Rewrite { id: SentenceId, bin: LengthBin, expected_gain: f64 },
    Promote { id: SentenceId, replace: String, with_any_of: Vec<String>, expected_gain: f64 },
    Demote { word: String, max_count: usize, in_sentences: Vec<SentenceId>, expected_gain: f64 },
}

pub struct WriteResult {
    pub text: String,
    pub fingerprint: FingerprintReport,     // v1 §8, unchanged
    pub validation: ValidationReport,
    pub citations: Vec<Citation>,
    pub rounds_used: usize,
    pub total_tokens: usize,
    pub research_degraded: bool,
    pub validation_warning: bool,
}
```

`FingerprintReport`, `AnalyzedSentence`, `ValidationReport`, `MetricCheck` are carried from v1 §8/§10 verbatim, plus a `sensitivity: Vec<SensitivityRow>` extension on the report.

---

## 13. Orchestration Patterns and Reuse Map

### Pattern provenance (what each stage is, in industry vocabulary)

| Stage | Pattern | Source |
|---|---|---|
| Research | orchestrator-workers + parallel fan-out; subagent-as-tool (not handoff) | Anthropic "Building Effective Agents" + multi-agent research system; Claude Agent SDK subagents; OpenAI Agents SDK agent-as-tool |
| Skeleton → sections | prompt chaining with gates; receding-horizon feedback is our addition | Anthropic workflow taxonomy |
| Refinement | evaluator-optimizer, with a **deterministic** evaluator (no judge noise, no generator/evaluator deadlock) | Self-Refine / Reflexion lineage |
| Whole pipeline | blackboard (shared artifact, no transcript passing); typed state + reducers + checkpointer + conditional edges | classic blackboard; LangGraph state semantics (Rust references: juncture, metalcraft — semantics only, not adopted) |
| Discipline | own control flow; stateless-reducer agents; tools as structured outputs; small focused agents | 12-Factor Agents |

Worker contract rules stolen verbatim from the Anthropic research-system writeup: isolated context per worker, compressed structured artifact returns, per-worker token budget, parallelism only where subtasks are independent.

Explicit non-adoptions: conversational multi-agent (group chat / crews — nondeterministic topology, token-hungry, untestable here); A2A protocol (no cross-vendor interop in-process); external graph engines (the writer graph is ~7 nodes / 1 cycle; a hand-rolled state machine under the existing `STATE_MACHINE.md` discipline beats a framework dependency).

### Reuse map (existing code)

| Concern | Reused component |
|---|---|
| Research workers | `app-chat` react loop, `rag` + `search` ModeConfigs, unchanged |
| Tool allowlists | capability registry (`tool_pool`, `applicable_strategies`) |
| Patch/JSON parse retry | `agents/loop/parse.rs` + fallback path |
| Progress streaming | `agents/sse_sink.rs`, `agents/progressive/` |
| Replay/debugging | `agents/replay.rs` conventions |
| Citations | `agents/unified/helpers/citations.rs` |
| Web input hygiene | `agents/untrusted_input.rs`, `content_guard.rs` |
| Metering | LLM usage exit metering (migration 0050, usage observer) |
| Style priming | `writing-style` skill slot (`answer_writing_styles`) |
| Artifact/checkpoint style | worker `document_pipeline`, e2e `artifacts.rs` |

### New code (the actual gap, all small)

1. `WriterState` + reducers + file checkpointing (serde).
2. `SubagentInvoker`: run the react loop under a given mode id with budget/timeout/result schema.
3. Research fan-out/join (`tokio::JoinSet`).
4. Refinement loop runner (budgeted evaluator-optimizer with best-version retention).
5. Analyzer + sensitivity table + op enumerator (pure Rust; jieba-rs, statrs).
6. Workspace kernel: canonicalizer, ID rules, patch parser/splicer.

---

## 14. Default Parameters

| Parameter | Default | Rationale |
|---|---|---|
| `cv` | 0.75 | midpoint of human range |
| `median_length` | 20 chars | Chinese expository prose |
| `hapax_target` | 0.45 | midpoint of human range |
| `zipf_exponent` | 1.0 | canonical |
| `l_min` / `l_max` | 5 / 100 chars | carried from v1 |
| candidate length grid | {5, 8, 12, 16, 20, 26, 34, 44, 56, 72, 90} | log-spaced |
| `max_rounds` | 3 | diminishing returns |
| `max_rhythm_ops` / round | 8 | CV repair is low-dimensional (§1.1) |
| `max_lexical_ops` / round | 30 | hapax deficit ≈ 30 ops (§3.4) |
| `overshoot` | 1.3 | counter mean-reversion |
| research workers | 2 (corpus, web) | only independent subtasks |
| queries per worker | ≤ 3 | focused retrieval |
| cards | 10–30 | enough grounding, bounded prompts |
| score weights \(w_{1..4}\) | 0.4 / 0.2 / 0.25 / 0.15 | length dominates perceived rhythm |
| autocorr placement band | [0.2, 0.5] | planned-sequence target |

Expected cost per article: 1 (skeleton) + ~2 (research compression) + 4–8 (sections) + ≤6 (refinement) ≈ **10–17 LLM calls**, patch-sized outputs in refinement, stable prefixes for cache reuse. Plus 2 worker sub-loops under their own mode budgets.

---

## 15. Risks and Mitigations

### R1: Patch non-compliance or malformed patches
Parser rejects the whole patch → retry (existing path); repeated failure → recompile directives next round; persistent failure on one sentence → escalate op type (Extend → Rewrite). Compliance is measured per directive, so escalation is targeted.

### R2: Mean-reversion re-smoothing
Every free rewrite re-flattens the distribution. Mitigations are structural: patch-only output (untouched text cannot change), K bounded per round, overshoot on targets. There is no "polish the whole text" step anywhere in the pipeline after drafting.

### R3: Semantic drift in patched sentences
Bounded by K per round; op preference order (Split/Merge/Extend before Rewrite) is content-preserving; Extend ops weave material-card content rather than inviting invention.

### R4: Format tax in drafting
Drafting stays free-prose. M3 runs the A/B (free prose vs one-sentence-per-line) to quantify the tax before anyone is tempted to structure the drafting stage.

### R5: Engineered-sounding rhythm
Tolerance bands, not exact matching; length targets as bins; placement respects paragraph rhythm modes; extremes are minted by splitting/merging real content. Residual risk accepted — the validator measures statistics, humans should spot-check readability (M4 exit criterion includes a human read).

### R6: jieba tokenization instability (carried from v1)
Custom dictionary for domain terms extracted from material cards; relative comparisons remain valid.

### R7: Research failure or poisoned web content
Degrade to single source / ungrounded with warnings; web content passes `untrusted_input`/`content_guard`; cards are quotes-with-references, never instructions.

### R8: Cost blowup
Hard budgets at three levels (per worker, per round, per job); parallelism confined to research; refinement outputs are patches, not full texts; metering feeds billing.

---

## 16. Implementation Milestones

### M1: Analyzer + sensitivity table (validate the premise)
- v1 M1 scope (all metrics, CLI `heavytail-analyze`) **plus** the sensitivity table and op enumerator (§3.3–3.5).
- Property tests: attribution sums match totals; every table entry equals brute-force recomputation; quantile targets round-trip.
- Human-vs-AI validation dataset (10+ each). **Exit gate carried from v1: if the metrics do not differentiate human from AI text, stop.**

### M2: Workspace kernel (no LLM)
- Canonicalizer (segmentation, ID assignment), ID rules (split/merge/tombstone), patch parser + splicer, checkpoint round-trip.
- Fixture-tested pure Rust. Exit: fuzz segmentation edge cases (quotes, ellipses, ；); patch grammar rejects all malformed cases.

### M3: Drafting experiment (three arms decide the architecture empirically)
- Same 10 topics × 3 arms: (a) plain free-write, (b) skill-primed free-write + MPC deficit hints, (c) v1 feedforward Phase A per v1 §6–7.
- Measure all fingerprints with M1. Also run the free-prose vs line-per-sentence A/B (R4).
- Exit: quantify starting-point deficits per arm. Decision rules: if (b) ≈ (c), the feedforward scheduler stays retired; if (b) + one round of M4 refinement passes bands, MPC hints suffice; if (a) ≈ (b), drop deficit hints too.

### M4: Refinement loop end-to-end
- Directive compiler + two-pass patch rounds + validator, on arm-(b) drafts.
- Exit: ≥ 8/10 topics pass all bands within default budget; human spot-check finds no broken grammar from splices; compliance ≥ ~70% per round.

### M5: Mode integration (productization)
- `write` ModeSchema + orchestrator dispatch; research workers via `SubagentInvoker`; material cards + citations; SSE progress; metering labels; contract tests + e2e gate entry.
- Exit: end-to-end job from HTTP request to `WriteResult` with artifacts, on the standard e2e harness.

---

## 17. Open Questions

1. **Word-frequency table source** (carried from v1): jieba IDF as rank proxy vs building a table from BCC/BLCU corpus. Needed by M1 for promote/demote enumeration.
2. **Patch transport**: fenced block (default) vs `contracts` tool call. Decide after M4 compliance data.
3. **Combined `write-research` mode** (v2.1): requires extending single-tool `auto_fallback`; is cross-source reasoning worth it over parallel single-mode workers?
4. **MPC deficit hints**: keep or drop, per M3 decision rule.
5. **Band calibration vs article length**: hapax/TTR drift with length (Heaps' law); bands are calibrated for ~800–3000 chars. Longer forms need length-conditional bands.
6. **Secondary evaluation**: integrate an external AI-detector score as a report-only metric? (carried from v1)
7. **Very long articles**: section-context compression beyond the pinned-skeleton + sliding-window strategy (>5k chars).
