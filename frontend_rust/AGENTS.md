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
- Official styling stack: `design_tokens.css` + Plain CSS + Stylance CSS Modules.
- Prefer token-driven CSS modules over route-level utility strings and global ad-hoc classes.
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
- Plain CSS + Stylance CSS Modules are the primary styling system.
- New UI work should:
  - map values to design tokens first,
  - place route/component visuals in `.module.css` files second,
  - keep route-level JSX free of ad-hoc visual constants.
- Tailwind utility classes are legacy migration debt and must not be introduced in new code.
