import { ApiError, request } from "../http/request";

export type UsageWindowBucket = {
  /** Internal usage_units (ledger). */
  used: number;
  /** Internal rolling limit in usage_units. */
  limit: number;
  /** Product-facing approx tokens (used / M * 1000). */
  used_tokens_approx?: number;
  /** Product-facing approx tokens (limit / M * 1000). */
  limit_tokens_approx?: number;
  percentage: number;
  reset_at: string; // ISO 8601
};

export type LimitHits = {
  rolling_5h: boolean;
  rolling_7d: boolean;
};

export type UsageWindowResponse = {
  plan_id: "free" | "plus" | "pro";
  /** Plan margin multiplier M (free 2.0 / plus 1.5 / pro 1.3). */
  margin_multiplier?: number;
  rolling_5h: UsageWindowBucket;
  rolling_7d: UsageWindowBucket;
  soft_limit_hit: LimitHits;
  hard_limit_hit: LimitHits;
};

/** Prefer server approx tokens; fall back to units when older API omits fields. */
export function bucketUsedTokensApprox(bucket: UsageWindowBucket): number {
  return bucket.used_tokens_approx ?? bucket.used;
}

export function bucketLimitTokensApprox(bucket: UsageWindowBucket): number {
  return bucket.limit_tokens_approx ?? bucket.limit;
}

export type DailyUsage = {
  date: string; // YYYY-MM-DD
  tokens: number;
};

export type UsageHistoryResponse = {
  daily: DailyUsage[];
};

export type UsageForecastResponse = {
  current_plan: string;
  avg_30d_tokens: number;
  projected_30d_tokens: number;
  current_limit_7d: number;
  upgrade_recommended: boolean;
  suggestion_zh: string;
  suggestion_en: string;
};

export type BillingPlanQuota = {
  metric_type: string;
  soft_limit: number | null;
  hard_limit: number | null;
};

export type BillingPlan = {
  plan_id: string;
  name: string;
  description: string;
  price_label: string;
  price_label_cny: string;
  price_label_usd: string;
  interval: string;
  checkout_available: boolean;
  current: boolean;
  quotas: BillingPlanQuota[];
};

export type BillingPlansResponse = {
  plans: BillingPlan[];
  current_plan_id: string;
};

type Envelope<T> = {
  ok?: boolean;
  data?: T;
  error?: { code?: string; message?: string } | null;
};

function unwrap<T>(envelope: Envelope<T>, fallback: string, status = 200): T {
  if (envelope.ok && envelope.data) {
    return envelope.data;
  }
  const message = envelope.error?.message ?? fallback;
  const code = envelope.error?.code ?? null;
  throw new ApiError(status, code, message);
}

export const billingApi = {
  async getPlans(token?: string | null) {
    return unwrap<BillingPlansResponse>(
      await request<Envelope<BillingPlansResponse>>(
        "/api/v1/billing/plans",
        {
          method: "GET",
          credentials: "include",
        },
        token ?? undefined,
      ),
      "Failed to load billing plans",
    );
  },

  async getUsageWindow(token?: string | null) {
    return unwrap<UsageWindowResponse>(
      await request<Envelope<UsageWindowResponse>>(
        "/api/v1/billing/usage/window",
        {
          method: "GET",
          credentials: "include",
        },
        token ?? undefined,
      ),
      "Failed to load usage window",
    );
  },

  async getUsageHistory(days = 7, token?: string | null) {
    return unwrap<UsageHistoryResponse>(
      await request<Envelope<UsageHistoryResponse>>(
        `/api/v1/billing/usage/history?days=${days}`,
        { method: "GET", credentials: "include" },
        token ?? undefined,
      ),
      "Failed to load usage history",
    );
  },

  async getUsageForecast(token?: string | null) {
    return unwrap<UsageForecastResponse>(
      await request<Envelope<UsageForecastResponse>>(
        "/api/v1/billing/usage/forecast",
        {
          method: "GET",
          credentials: "include",
        },
        token ?? undefined,
      ),
      "Failed to load usage forecast",
    );
  },
};
