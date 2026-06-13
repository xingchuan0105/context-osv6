import { ApiError, request } from "../http/request";

export type UsageWindowBucket = {
  used: number;
  limit: number;
  percentage: number;
  reset_at: string; // ISO 8601
};

export type LimitHits = {
  rolling_5h: boolean;
  rolling_7d: boolean;
};

export type UsageWindowResponse = {
  plan_id: "free" | "plus" | "pro";
  rolling_5h: UsageWindowBucket;
  rolling_7d: UsageWindowBucket;
  soft_limit_hit: LimitHits;
  hard_limit_hit: LimitHits;
};

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
  async getPlans() {
    return unwrap<BillingPlansResponse>(
      await request<Envelope<BillingPlansResponse>>("/api/v1/billing/plans", {
        method: "GET",
        credentials: "include",
      }),
      "Failed to load billing plans",
    );
  },

  async getUsageWindow() {
    return unwrap<UsageWindowResponse>(
      await request<Envelope<UsageWindowResponse>>("/api/v1/billing/usage/window", {
        method: "GET",
        credentials: "include",
      }),
      "Failed to load usage window",
    );
  },

  async getUsageHistory(days = 7) {
    return unwrap<UsageHistoryResponse>(
      await request<Envelope<UsageHistoryResponse>>(
        `/api/v1/billing/usage/history?days=${days}`,
        { method: "GET", credentials: "include" },
      ),
      "Failed to load usage history",
    );
  },

  async getUsageForecast() {
    return unwrap<UsageForecastResponse>(
      await request<Envelope<UsageForecastResponse>>("/api/v1/billing/usage/forecast", {
        method: "GET",
        credentials: "include",
      }),
      "Failed to load usage forecast",
    );
  },
};
