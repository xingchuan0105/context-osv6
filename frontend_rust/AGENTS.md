# frontend_rust AGENTS

## UI Drift Prevention Rules

### 1) Design Tokens Are the Source of Truth
- Canonical token file: `crates/web-ui/src/styles/design_tokens.css`
- Generated token section target: `crates/web-ui/src/index.css` between:
  - `/* DESIGN_TOKENS_START */`
  - `/* DESIGN_TOKENS_END */`
- Sync command:
  - `python scripts/sync_design_tokens.py`

### 2) Styling Boundaries
- Avoid hard-coded hex colors in Rust UI files.
- Prefer semantic classes and token-driven variables from `index.css`.
- Do not use inline `style=` in Rust components.
- Temporary exception: `crates/web-ui/src/routes/preview.rs` is allowed to use inline style and hex for pixel-mapped design review pages.

### 3) Layout vs State Responsibilities
- Layout/container components should be signal-free.
- Reactive state (`create_signal`, `signal`, derived state) belongs in leaf components or route-level logic, not layout wrappers.

### 4) Guard Commands
- Report mode:
  - `python scripts/ui_drift_guard.py`
- Enforcement mode:
  - `python scripts/ui_drift_guard.py --strict`
- Legacy allowlist:
  - `scripts/ui_drift_allowlist.txt` (existing debt baseline only; no new entries without explicit review)

### 5) Component Strategy
- Existing Tailwind + semantic component classes remain the primary styling system.
- New UI work should:
  - map values to design tokens first,
  - add or reuse semantic utility classes second,
  - keep route-level JSX free of ad-hoc visual constants.
