# Visual Regression Testing (Playwright)

This workflow focuses on frontend UI layout, typography, and visual effects with screenshot comparison.

## Scope

Visual test spec: `e2e/visual-ui.spec.ts`

Covered pages:
- Login shell (`/login`)
- Settings appearance shell (`/settings`)
- Workspace shell (`/dashboard/:notebookId`)

Projects:
- `visual-desktop` (1440x900)
- `visual-mobile` (Pixel 5 emulation on Chromium)

## 1) Generate Baseline Snapshots

Run after UI is intentionally changed and approved:

```bash
npx playwright test --config=playwright.config.ts --project=visual-desktop --project=visual-mobile --update-snapshots
```

If you changed `frontend_rust` UI source and need fresh `/pkg` assets first, rebuild wasm in hydrate-only mode:

```bash
cd /home/chuan/context-osv6/frontend_rust
cargo build -p frontend-web-ui --target wasm32-unknown-unknown --release --no-default-features --features hydrate
wasm-bindgen target/wasm32-unknown-unknown/release/web_ui.wasm --target web --out-dir pkg --out-name web_ui
npx tailwindcss -c tailwind.config.js -i ./crates/web-ui/src/index.css -o ./pkg/index.css --minify
```

Why: `frontend-web-ui` defaults to `ssr`; building with only `--features hydrate` still enables `ssr` and can cause client-side router panic (`no RequestUrl provided`).

Snapshots are saved under:
- `e2e/visual-ui.spec.ts-snapshots/`

## 2) Run Visual Comparison

Run in normal mode to compare current UI against baseline:

```bash
npx playwright test --config=playwright.config.ts --project=visual-desktop --project=visual-mobile
```

When diff exists, Playwright writes artifacts into `test-results/`:
- `*-expected.png`
- `*-actual.png`
- `*-diff.png`

## 3) Generate Diff Analysis Report

Build a markdown report from visual artifacts:

```bash
python scripts/visual_diff_report.py --test-results-dir test-results --output test-results/visual-diff-report.md --show-images
```

Report output:
- `test-results/visual-diff-report.md`

## 4) Suggested Review Checklist

- UI structure: card/grid/spacing changed unexpectedly?
- Typography: heading size, line-height, wrapping, truncation changed?
- Visual style: color, border, shadow, state badges changed?
- Responsive behavior: desktop/mobile hierarchy still readable?

## Noise Reduction Rules

- Visual projects force `colorScheme: "light"` and `timezoneId: "UTC"`.
- Animations/caret are disabled during screenshot capture.
- Dynamic live areas are masked (`time`, `aria-live`, status/toast/spinner surfaces).

If snapshots are noisy:
- Prefer masking a dynamic region in `e2e/visual-ui.spec.ts`.
- Avoid broad mask that can hide real UI regressions.
- Keep baseline updates in dedicated commits with clear PR notes.
