#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

git add frontend_next/components/billing/UsageMeter.tsx \
  frontend_next/components/billing/UsageMeter.module.css \
  frontend_next/tests/billing/UsageMeter.test.tsx
git commit -m "feat(frontend): add UsageMeter component (full/compact variants)"

git add frontend_next/components/billing/PricingCards.tsx \
  frontend_next/components/billing/PricingCards.module.css \
  frontend_next/tests/billing/PricingCards.test.tsx
git commit -m "feat(frontend): add PricingCards component (full/compact variants)"

git add frontend_next/components/billing/UsageWarningToast.tsx \
  frontend_next/components/billing/UsageWarningToast.module.css \
  frontend_next/tests/billing/UsageWarningToast.test.tsx
git commit -m "feat(frontend): add UsageWarningToast (80%/95% thresholds)"

git add frontend_next/components/billing/UsageForecastCard.tsx \
  frontend_next/components/billing/UsageForecastCard.module.css \
  frontend_next/tests/billing/UsageForecastCard.test.tsx
git commit -m "feat(frontend): add UsageForecastCard (upgrade recommendation)"

git add frontend_next/components/billing/UsageTrendChart.tsx \
  frontend_next/components/billing/UsageTrendChart.module.css \
  frontend_next/tests/billing/UsageTrendChart.test.tsx
git commit -m "feat(frontend): add UsageTrendChart (pure SVG line chart, 7-day default)"

git add frontend_next/components/billing/PaywallModal.tsx \
  frontend_next/components/billing/PaywallModal.module.css \
  frontend_next/tests/billing/PaywallModal.test.tsx
git commit -m "feat(frontend): add PaywallModal (reuses UsageMeter compact + PricingCards compact)"

git add frontend_next/app/\(marketing\)/pricing/ \
  frontend_next/tests/billing/pricing-page.test.tsx
git commit -m "feat(frontend): add /pricing page with 3-tier cards + FAQ"

git add frontend_next/app/\(app\)/settings/usage/ \
  frontend_next/tests/billing/usage-page.test.tsx
git commit -m "feat(frontend): add /settings/usage dashboard"

git add frontend_next/app/\(app\)/upgrade/paywall/
git commit -m "feat(frontend): add /upgrade/paywall page (full-screen paywall)"

git add frontend_next/app/\(app\)/upgrade/success/
git commit -m "feat(frontend): add /upgrade/success post-checkout landing page"

git add frontend_next/lib/i18n/messages.ts
git commit -m "feat(frontend): add i18n entries for pricing/usage/paywall/toast"

git add frontend_next/components/workspace/workspace-surface.tsx \
  frontend_next/tests/workspace/workspace-surface.test.tsx
git commit -m "feat(workspace): show 80%/95% usage warning toast"

git add frontend_next/e2e/pom/billing-page.ts
git commit -m "test(e2e): add BillingPage POM (PricingPage/UsagePage/PaywallPage)"

git add frontend_next/e2e/specs/billing/pricing-page.spec.ts \
  frontend_next/e2e/specs/billing/usage-dashboard.spec.ts \
  frontend_next/e2e/specs/billing/paywall-flow.spec.ts \
  frontend_next/playwright.config.ts
git commit -m "test(e2e): add billing E2E for pricing/usage/paywall"

git add frontend_next/e2e/specs/billing/dark-mode.spec.ts \
  frontend_next/e2e/specs/billing/visual-regression.spec.ts
git commit -m "test(e2e): dark mode + visual regression for billing pages"

git add avrag-rs/crates/billing/src/feature_flag.rs \
  avrag-rs/crates/billing/src/lib.rs \
  avrag-rs/crates/transport-http/src/routes/billing.rs \
  frontend_next/lib/billing/featureFlag.ts \
  docs/ops/2026-06-14-pricing-revamp-rollout.md
git commit -m "feat(billing): add rollout flag for pricing revamp"

echo "All pricing revamp commits created."
