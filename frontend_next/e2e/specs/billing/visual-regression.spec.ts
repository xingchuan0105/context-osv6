import { test } from "../../fixtures/run-context";

/**
 * Billing visual baselines — deferred until UsageMeter is wired into settings (PR-3)
 * and PRICING_REVAMP_ROLLOUT gate is stable in E2E env.
 */
test.describe("Billing Visual Regression", () => {
  test.skip("B00: settings billing tab snapshot", async () => {
    // TODO(PR-3): navigate to /settings?tab=billing, mask dynamic dates/countdowns,
    // then capture toHaveScreenshot("billing-settings.png").
  });
});
