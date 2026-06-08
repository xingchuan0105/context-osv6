import { ApiError, buildApiUrl } from "../auth/client";

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

async function decodeError(response: Response): Promise<ApiError> {
  const raw = await response.text();

  if (!raw.trim()) {
    return new ApiError(response.status, null, `Request failed with status ${response.status}`);
  }

  try {
    const parsed = JSON.parse(raw) as { error?: string; message?: string };
    return new ApiError(response.status, parsed.error ?? null, parsed.message ?? raw);
  } catch {
    return new ApiError(response.status, null, raw);
  }
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (!headers.has("Accept")) {
    headers.set("Accept", "application/json");
  }

  const response = await fetch(buildApiUrl(path), {
    ...init,
    cache: "no-store",
    credentials: "include",
    headers,
  });

  if (!response.ok) {
    throw await decodeError(response);
  }

  return (await response.json()) as T;
}

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
      await request<Envelope<BillingPlansResponse>>("/api/v1/billing/plans", { method: "GET" }),
      "Failed to load billing plans",
    );
  },

  async getUsageWindow() {
    return unwrap<UsageWindowResponse>(
      await request<Envelope<UsageWindowResponse>>("/api/v1/billing/usage/window", {
        method: "GET",
      }),
      "Failed to load usage window",
    );
  },

  async getUsageHistory(days = 7) {
    return unwrap<UsageHistoryResponse>(
      await request<Envelope<UsageHistoryResponse>>(
        `/api/v1/billing/usage/history?days=${days}`,
        { method: "GET" },
      ),
      "Failed to load usage history",
    );
  },

  async getUsageForecast() {
    return unwrap<UsageForecastResponse>(
      await request<Envelope<UsageForecastResponse>>("/api/v1/billing/usage/forecast", {
        method: "GET",
      }),
      "Failed to load usage forecast",
    );
  },
};
