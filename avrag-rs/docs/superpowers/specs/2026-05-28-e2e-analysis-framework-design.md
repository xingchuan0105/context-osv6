# E2E Analysis Framework Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:writing-plans` after spec approval.

**Goal:** Build a cross-run regression detection and failure attribution framework for agent E2E tests, with coverage governance and stability trending as extensions.

**Architecture:** Four-phase layered analyzer (P0 regression + attribution, P1 coverage governance, P2 stability trends) that consumes `e2e_output/{run_id}/` artifacts and produces actionable reports without modifying test runtimes.

**Tech Stack:** Rust CLI, serde_json, Markdown reports, optional perceptual hashing for screenshot diff.

---

## Table of Contents

1. [Context](#1-context)
2. [Goals and Non-Goals](#2-goals-and-non-goals)
3. [Data Model](#3-data-model)
4. [Phase 1: Cross-Run Regression Detection (P0)](#4-phase-1-cross-run-regression-detection-p0)
5. [Phase 2: Failure Attribution Diagnosis (P0)](#5-phase-2-failure-attribution-diagnosis-p0)
6. [Phase 3: Coverage Governance Matrix (P1)](#6-phase-3-coverage-governance-matrix-p1)
7. [Phase 4: Stability and Trending (P2)](#7-phase-4-stability-and-trending-p2)
8. [CLI Interface](#8-cli-interface)
9. [Phase Dependencies and Milestones](#9-phase-dependencies-and-milestones)
10. [Open Questions](#10-open-questions)

---

## 1. Context

The project has 16+ E2E tests across Chat, RAG, and Search strategies. Each run persists:

- `meta.json` — `TestResult` with status, answer, LLM calls, tool calls, duration, token usage
- `llm_calls.jsonl` — captured system prompts, user messages, responses
- `tool_calls.jsonl` — tool inputs/outputs
- `answer.txt` / `answer.html` — generated outputs
- `screenshot.png` — rendered HTML output (when applicable)

The gap: results are saved per-run but never compared across runs. There is no systematic way to detect prompt drift, behavior changes, cost regression, or classify failures into actionable layers.

---

## 2. Goals and Non-Goals

### Goals

- Detect regression across runs for prompt assembly, state-machine behavior, output structure, and cost/performance
- Attribute failures to specific layers (FSM, prompt, tool, model, perf) with first-anomaly localization
- Generate risk-prioritized coverage gaps with concrete test-pattern recommendations
- Track flaky tests and performance drift over time (when sufficient history exists)

### Non-Goals

- Replace existing test runners or modify E2E test code
- Real-time monitoring during test execution (post-processing only)
- Full screenshot pixel-diff as a mandatory gate (optional, default off)
- Automatic test generation from coverage gaps (recommend only)

---

## 3. Data Model

### 3.1 Run Record

```rust
pub struct RunRecord {
    pub run_id: String,
    pub branch: String,
    pub commit: String,
    pub timestamp: String,
    pub results: Vec<TestResult>,
}
```

### 3.2 Test Fingerprint

```rust
pub struct TestFingerprint {
    pub test_name: String,
    pub source_file: String,
    pub source_hash: String, // hash of test function body
    pub case_version: String,
}
```

Fingerprint match determines diagnostic depth:
- `true`: full strict diff and attribution
- `false`: informational compare only, no hard conclusions

### 3.3 Diff Entry (Phase 1)

```rust
pub struct DiffEntry {
    pub dimension: DiffDimension, // Prompt | Behavior | Output | CostPerf
    pub severity: DiffSeverity,   // Hard | Soft | Info
    pub category: DiffCategory,   // Functional | NonFunctional
    pub raw_diff: String,
    pub normalized_signal: String, // e.g. "duration_ms: +37%"
    pub baseline_value: serde_json::Value,
    pub current_value: serde_json::Value,
}
```

### 3.4 Attribution Report (Phase 2)

```rust
pub struct AttributionReport {
    pub test_name: String,
    pub fingerprint_match: bool,
    pub category: FailureCategory,
    pub severity: DiffSeverity,
    pub confidence: ConfidenceLevel, // High | Medium | Low
    pub suspected_layers: Vec<SuspectedLayer>, // fsm | prompt_assembly | tool_dispatch | llm_output | perf_budget
    pub first_anomaly: FirstAnomaly,
    pub related_diffs: Vec<DiffEntry>,
    pub suggested_action: String,
    pub diagnostic_notes: Option<String>,
}

pub struct FirstAnomaly {
    pub stage: String,
    pub iteration: u32,
    pub expected_next: Vec<String>,
    pub actual_next: String,
    pub reasoning: String,
}
```

### 3.5 Coverage Gap (Phase 3)

```rust
pub struct CoverageGap {
    pub priority: GapPriority, // High | Medium | Info
    pub risk_score: f32, // impact * likelihood * blast_radius
    pub dimensions: HashMap<String, String>,
    pub related_tests: Vec<String>,
    pub evidence: String,
    pub recommended_test_pattern: String,
}
```

### 3.6 Stability Record (Phase 4)

```rust
pub struct StabilityRecord {
    pub test_name: String,
    pub fingerprint_hash: String,
    pub flaky_rate: f32,
    pub runs_analyzed: u32,
    pub consecutive_failures: u32,
    pub category_history: Vec<CategorySnapshot>,
    pub perf_trend: PerfTrend,
}

pub struct PerfTrend {
    pub hard_regressions: Vec<PerfRegression>, // single-run over threshold
    pub drift_warnings: Vec<DriftWarning>,     // N-run slope positive
}
```

---

## 4. Phase 1: Cross-Run Regression Detection (P0)

### 4.1 Baseline Management

Three-tier fallback:
1. `.e2e_baseline` file (explicit persistent baseline)
2. `--baseline-run-id` CLI flag
3. Default: latest successful run on same branch

Baseline update is **explicit only** (`--promote-baseline` or manual command). Never implicit drift.

### 4.2 Diff Dimensions

#### Prompt Diff (Functional)

| Sub-dimension | Comparison | Severity Rule |
|--------------|-----------|--------------|
| system prompt hash | SHA-256 of full text | Info if hash changes but assertions pass; Hard if skill body missing |
| skills injected set | Set difference of skill IDs | Hard if required skill missing |
| per-skill body hash | Per-skill SHA-256 | Hard if body changed AND assertion fails |
| tool catalog set | Set difference of tool names | Hard if plan tool missing |
| message count | Number of user/assistant messages | Soft if > 20% change |

#### Behavior Diff (Functional)

| Sub-dimension | Comparison | Severity Rule |
|--------------|-----------|--------------|
| state sequence | Canonical path summary | Hard if illegal transition |
| replan count | Integer delta | Soft if increases |
| final decision | Synthesized / Degraded / Error | Hard if changes from Synthesized to Error |
| degrade reason | Category of degradation | Soft if reason changes |

#### Output Diff (Functional)

| Output Type | Comparison |
|------------|-----------|
| text | Length delta, structure pattern presence |
| html | DOM node count, heading depth, resource refs |
| ppt | Slide count, title presence, notes count |

#### Cost/Perf Diff (Non-Functional)

| Metric | Relative Threshold | Absolute Threshold |
|--------|-------------------|-------------------|
| total_input_tokens | +30% | > 20k |
| total_output_tokens | +30% | > 10k |
| llm_call_count | +50% | > 10 |
| tool_call_count | +50% | — |
| wall_clock_duration_ms | +30% | > 20s |

### 4.3 Output Format

**Markdown report** (human-readable, PR comment friendly):

```markdown
# E2E Regression Report

## Summary
- Baseline: `e2e_20260520-143022-a1b2c3d4`
- Current: `e2e_20260528-091122-e5f6g7h8`
- 12 passed, 2 soft drift, 1 hard regression

## Hard Regressions

### chat_content_guard_redacts_injection
- **Category:** Functional
- **Dimension:** Prompt
- **Signal:** `tool_catalog_count: 12 -> 9`
- **Detail:** Missing `conversation_history_tag` in tool catalog
```

**JSON summary** (CI gate parseable):

```json
{
  "baseline_run_id": "e2e_20260520-143022-a1b2c3d4",
  "current_run_id": "e2e_20260528-091122-e5f6g7h8",
  "summary": {
    "hard": 1,
    "soft": 2,
    "info": 5
  },
  "gate_status": "blocked"
}
```

---

## 5. Phase 2: Failure Attribution Diagnosis (P0)

### 5.1 Attribution Categories

Priority chain (first match wins):

1. **StateMachineFailure** — illegal state transition, budget/replan assertion failure, unexpected termination state
2. **PromptAssemblyFailure** — missing skill body, missing tool catalog, malformed history context
3. **ToolExecutionFailure** — wrong tool called, tool returned Error, tool sequence anomaly (e.g., tag before load), PolicyEnforcer denial
4. **ModelBehaviorFailure** — output format illegal, semantic drift, policy drift
   - Sub-labels: `FormatFailure`, `SemanticDrift`, `PolicyDrift`
5. **PerformanceRegression** — duration/token/call count over threshold (function passes)

### 5.2 First Anomaly Localization

```rust
pub struct FirstAnomaly {
    pub stage: String,           // e.g. "evaluate"
    pub iteration: u32,          // e.g. 1
    pub expected_next: Vec<String>, // e.g. ["answer", "degrade"]
    pub actual_next: String,     // e.g. "parallel_search"
    pub reasoning: String,       // human-readable explanation
}
```

### 5.3 Confidence Levels

| Level | Evidence | CI Action |
|-------|---------|-----------|
| `high` | State sequence mismatch, tool Error status, explicit parse failure | Block gate |
| `medium` | Prompt length mutation, catalog change, call count anomaly | Require review |
| `low` | Semantic drift inferred from output only | Log trend, no alert |

### 5.4 Fingerprint-Aware Diagnostics

- `fingerprint_match: true` → full attribution with hard conclusions
- `fingerprint_match: false` → informational compare + light diagnosis, no hard conclusions

### 5.5 Mapping from Phase 1 Diff

| Phase 1 Diff | Attribution Category |
|-------------|---------------------|
| skill/tool catalog missing | PromptAssemblyFailure |
| system prompt hash change + assertion fail | PromptAssemblyFailure |
| illegal state transition | StateMachineFailure |
| replan count increase | StateMachineFailure or ToolExecutionFailure |
| tool status = Error | ToolExecutionFailure |
| model output parse failure | ModelBehaviorFailure::FormatFailure |
| answer structure drift | ModelBehaviorFailure::SemanticDrift |
| duration/token over threshold | PerformanceRegression |

---

## 6. Phase 3: Coverage Governance Matrix (P1)

### 6.1 Two-Layer Architecture

**Governance Layer** (human view):
- High-priority gap list
- Risk-weighted coverage score per strategy
- New gaps in last 30 days
- Flaky hotspots

**Detail Layer** (machine consumption):
- Per-test dimension hit records
- Full cross-dimensional indexing

### 6.2 Core Dimensions

| Dimension | Role |
|-----------|------|
| Strategy (Chat/RAG/Search) | Core |
| State Boundary (plan/execute/evaluate/answer + cancel/budget_exhausted/empty_results/security_guard/fallback) | Core |
| Output Format (text/html/ppt/teach) | Core |
| Risk Category (injection/empty_input/no_results/history_load/format_constraint) | Core |
| Skill (chat-plan/retrieval-planner/html-renderer/etc.) | Side (filter) |
| Atomic Tool (calculator/web_search/history_load/etc.) | Side (filter) |

### 6.3 Coverage Status

| Status | Meaning |
|--------|---------|
| `covered_and_passing` | Hit by test, recent runs pass |
| `covered_but_flaky` | Hit by test, flaky_rate > 20% |
| `covered_but_recently_failing` | Hit by test, consecutive failures ≥ 2 |
| `only_ignored` | Only `#[ignored]` tests cover this |
| `never_tested` | Zero hits |

### 6.4 Risk Score

```
risk_score = impact × likelihood × blast_radius
```

Each gap includes:
- `risk_score`: float for sorting
- `evidence`: why this gap matters
- `recommended_test_pattern`: concrete combination template (e.g., `RAG-empty-results + html-renderer + security_guard`)

---

## 7. Phase 4: Stability and Trending (P2)

### 7.1 Flaky Detection

**Prerequisite:** fingerprint consistent across runs.

| Metric | Threshold | Action |
|--------|-----------|--------|
| `flaky_rate` | > 20% | Mark as flaky |
| `consecutive_failures` | ≥ 3 | Alert |
| `category_history_variance` | category jumps across runs | Flag instability |

### 7.2 Performance Trends

Two-tier alerting:

| Tier | Trigger | Example |
|------|---------|---------|
| `hard_regression` | Single run over threshold | duration_ms > 20s |
| `drift_warning` | N-run linear slope positive | tokens increased 15% per run over last 5 runs |

### 7.3 Screenshot Diff (Optional Gate)

- Default: record only, do not block
- Enable via `--enable-screenshot-diff`
- Uses perceptual hash or SSIM
- Configurable threshold

---

## 8. CLI Interface

```bash
# Phase 1: Regression diff
e2e-analyzer diff --baseline-run-id e2e_20260520 --current-run-id e2e_20260528

# Phase 2: Attribution (requires diff first, or auto-runs diff)
e2e-analyzer diagnose --run-id e2e_20260528

# Phase 3: Coverage matrix
e2e-analyzer coverage --runs 30

# Phase 4: Stability trends
e2e-analyzer trends --test-name search_budget_exhaustion_degrades --runs 20

# Combined report (runs all applicable phases)
e2e-analyzer report --current-run-id e2e_20260528 --baseline-run-id e2e_20260520

# Baseline management
e2e-analyzer baseline promote --run-id e2e_20260528
e2e-analyzer baseline show
```

---

## 9. Phase Dependencies and Milestones

### Milestone 1: Phase 1 + Phase 2 (P0)

- [ ] `e2e-analyzer diff` command with all four dimensions
- [ ] Baseline selection and `.e2e_baseline` management
- [ ] `e2e-analyzer diagnose` with five attribution categories
- [ ] First-anomaly localization
- [ ] Markdown + JSON dual output
- [ ] CI gate integration (exit code based on hard regression count)

### Milestone 2: Phase 3 (P1)

- [ ] Coverage matrix scanner
- [ ] Risk score calculation
- [ ] Gap detection with `recommended_test_pattern`
- [ ] Governance-layer markdown report

### Milestone 3: Phase 4 (P2)

- [ ] Flaky detection with fingerprint awareness
- [ ] Performance trend analysis (hard regression + drift warning)
- [ ] Optional screenshot diff gate

---

## 10. Open Questions

1. Should the analyzer be a separate crate (`crates/e2e-analyzer`) or live within `app/tests/`?
2. What is the minimum N for drift_warning detection (5 runs? 10 runs?)?
3. Should risk score weights (impact/likelihood/blast_radius) be hardcoded or configurable per strategy?
4. Do we need a `baseline promote` web UI or is CLI sufficient?
