# UI Drift Prevention (Leptos)

This project uses a layered anti-drift model adapted for the existing Rust + Leptos + Tailwind stack.

## Layer 1: Design Tokens
- Canonical source: `crates/web-ui/src/styles/design_tokens.css`
- Runtime CSS carrier: `crates/web-ui/src/index.css` token section
- Sync command:
  - `python scripts/sync_design_tokens.py`

Notes:
- Keep token edits in `design_tokens.css` only.
- Do not hand-edit the generated token section in `index.css`.

## Layer 2: Styling Scope
- Current baseline uses semantic classes in `index.css` plus Tailwind utilities.
- Route/component code should prefer semantic classes and token references over ad-hoc values.
- Pixel-mapped preview pages (`routes/preview.rs`) are explicitly isolated from production route styling rules.

## Layer 3: Layout vs Business Logic
- Layout/container components should avoid reactive state.
- Signals and state transitions should live in route logic or leaf components.

## Layer 4: Component Reuse
- Reuse existing shared component modules under `crates/web-ui/src/components/`.
- New interactive widgets should be added to shared component areas before route-local duplication.

## Layer 5: Agent and CI Guardrails
- Agent rules: `frontend_rust/AGENTS.md`
- Drift guard script:
  - Report mode: `python scripts/ui_drift_guard.py`
  - Strict mode: `python scripts/ui_drift_guard.py --strict`
  - Baseline allowlist: `scripts/ui_drift_allowlist.txt`

## Recommended Daily Workflow
1. Update Figma.
2. Sync/adjust tokens in `design_tokens.css`.
3. Run `python scripts/sync_design_tokens.py`.
4. Implement components/pages.
5. Run `python scripts/ui_drift_guard.py`.
6. Build frontend bundle and visually verify.
