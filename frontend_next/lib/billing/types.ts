import type { UsageWindowBucket, LimitHits } from "./api";
import type { UiLocale } from "../i18n/config";

export type UsageMeterProps = {
  variant: "full" | "compact";
  locale: UiLocale;
  planId: "free" | "plus" | "pro";
  /** Plan margin multiplier M; shown transparently under meters. */
  marginMultiplier?: number;
  rolling5h: UsageWindowBucket;
  rolling7d: UsageWindowBucket;
  softLimitHit: LimitHits;
  hardLimitHit: LimitHits;
};
