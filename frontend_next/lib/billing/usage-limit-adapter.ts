import type { UsageLimitResponse, UsageWindow } from "../settings/client";
import type { LimitHits, UsageWindowBucket } from "./api";
import type { UsageMeterProps } from "./types";
import type { UiLocale } from "../i18n/config";

const PLAN_IDS = new Set<UsageMeterProps["planId"]>(["free", "plus", "pro"]);

function toBucket(window: UsageWindow): UsageWindowBucket {
  return {
    used: window.used_units,
    limit: window.limit_units,
    percentage: window.percent_used,
    reset_at: window.next_relief_at ?? window.blocked_until ?? new Date().toISOString(),
  };
}

function deriveLimitHits(window: UsageWindow): { soft: boolean; hard: boolean } {
  const hard = window.blocked || window.percent_used >= 100;
  const soft = !hard && window.percent_used >= 70;
  return { soft, hard };
}

function toLimitHits(windows: UsageLimitResponse["windows"]): {
  softLimitHit: LimitHits;
  hardLimitHit: LimitHits;
} {
  const hit5h = deriveLimitHits(windows.rolling_5h);
  const hit7d = deriveLimitHits(windows.rolling_7d);
  return {
    softLimitHit: { rolling_5h: hit5h.soft, rolling_7d: hit7d.soft },
    hardLimitHit: { rolling_5h: hit5h.hard, rolling_7d: hit7d.hard },
  };
}

function resolvePlanId(data: UsageLimitResponse): UsageMeterProps["planId"] {
  if ("plan_default" in data.scope) {
    const planId = data.scope.plan_default.plan_id;
    if (PLAN_IDS.has(planId as UsageMeterProps["planId"])) {
      return planId as UsageMeterProps["planId"];
    }
  }
  return "free";
}

export function usageLimitToMeterProps(
  data: UsageLimitResponse,
  locale: UiLocale,
  options?: { variant?: UsageMeterProps["variant"] },
): UsageMeterProps {
  const { softLimitHit, hardLimitHit } = toLimitHits(data.windows);
  return {
    variant: options?.variant ?? "full",
    locale,
    planId: resolvePlanId(data),
    rolling5h: toBucket(data.windows.rolling_5h),
    rolling7d: toBucket(data.windows.rolling_7d),
    softLimitHit,
    hardLimitHit,
  };
}
