# UI Drift Prevention (Leptos)

This project uses a layered anti-drift model for Rust + Leptos with Plain CSS and Stylance CSS Modules as the target styling architecture.

## Layer 1: Design Tokens
- Canonical source: `crates/web-ui/src/styles/design_tokens.css`
- Runtime CSS carrier: `crates/web-ui/src/index.css` token section
- Sync command:
  - `python scripts/sync_design_tokens.py`

Notes:
- Keep token edits in `design_tokens.css` only.
- Do not hand-edit the generated token section in `index.css`.

## Layer 2: Scoped Styling
- Official styling stack: Plain CSS + Stylance CSS Modules.
- Route and component visuals should live in colocated `.module.css` files and reference token variables.
- `index.css` is reserved for resets, typography, shared primitives, token carriage, and temporary legacy compatibility.
- Pixel-mapped preview pages (`routes/preview.rs`) are explicitly isolated from production route styling rules.

## Layer 3: Legacy Tailwind Debt
- Tailwind utilities are legacy debt from the pre-Stylance phase.
- No new Tailwind utility strings should be added.
- Migration priority is: core approved pages first, shared components second, long-tail routes last.

## Layer 4: Layout vs Business Logic
- Layout/container components should avoid reactive state.
- Signals and state transitions should live in route logic or leaf components.

## Layer 5: Component Reuse
- Reuse existing shared component modules under `crates/web-ui/src/components/`.
- New interactive widgets should be added to shared component areas before route-local duplication.

## Layer 6: Agent and CI Guardrails
- Agent rules: `frontend_rust/AGENTS.md`
- Drift guard script:
  - Report mode: `python scripts/ui_drift_guard.py`
  - Strict mode: `python scripts/ui_drift_guard.py --strict`
  - Baseline allowlist: `scripts/ui_drift_allowlist.txt`

## Recommended Daily Workflow
1. Update Figma.
2. Sync or adjust tokens in `design_tokens.css`.
3. Run `python scripts/sync_design_tokens.py`.
4. Implement visuals in colocated `.module.css` files.
5. Run `python scripts/ui_drift_guard.py`.
6. Verify in the watch dev server and screenshot gates.
