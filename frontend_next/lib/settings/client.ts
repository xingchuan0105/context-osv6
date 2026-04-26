import { ApiError, buildApiUrl, type AuthEnvelope } from "../auth/client";

export type NotificationPreferences = {
  email_enabled: boolean;
  product_enabled: boolean;
  security_enabled: boolean;
  weekly_digest_enabled: boolean;
  quiet_hours_start: string | null;
  quiet_hours_end: string | null;
};

export type DashboardPreferences = {
  favorite_notebook_ids: string[];
  workspace_drafts: Array<Record<string, unknown>>;
  workspace_preferences: Array<Record<string, unknown>>;
  notebook_notes: Array<Record<string, unknown>>;
};

export type UserPreferences = {
  dashboard: DashboardPreferences;
  notifications: NotificationPreferences;
};

export type NotificationRow = {
  id: string;
  org_id: string;
  user_id: string;
  event_type: string;
  title: string;
  body: string;
  data: Record<string, unknown>;
  read_at: string | null;
  created_at: string;
  updated_at: string;
};

export type NotificationsResponse = {
  notifications: NotificationRow[];
};

export type UsageWindow = {
  used_units: number;
  limit_units: number;
  remaining_units: number;
  percent_used: number;
  blocked: boolean;
  next_relief_at: string | null;
  blocked_until: string | null;
};

export type UsageLimitResponse = {
  policy: {
    enabled: boolean;
    rolling_5h_limit_units: number;
    rolling_7d_limit_units: number;
  };
  windows: {
    rolling_5h: UsageWindow;
    rolling_7d: UsageWindow;
  };
  breakdown: Record<string, number>;
  scope:
    | {
        plan_default: {
          plan_id: string;
        };
      }
    | {
        user_override: Record<string, never>;
      };
  has_estimated_usage: boolean;
};

export type UsageResponse = {
  used_tokens: number;
  limit_tokens: number;
  used_documents: number;
  limit_documents: number;
};

export type PlanRow = {
  id: string;
  name: string;
  price: number;
  features: string[];
};

export type PlansResponse = {
  plans: PlanRow[];
};

export type SubscriptionResponse = {
  plan_id: string;
  status: string;
  current_period_end: string;
};

export type PortalSessionResponse = {
  url: string;
};

type ErrorEnvelope = {
  error?: string | null;
  message?: string;
};

type ApiEnvelope<T> = {
  ok?: boolean;
  data?: T | null;
  error?: {
    message?: string;
  } | null;
};

type RawPlanQuota = {
  metric_type: string;
  soft_limit?: number | null;
  hard_limit?: number | null;
};

type RawPlanRow = {
  plan_id: string;
  name: string;
  description: string;
  price_label: string;
  interval: string;
  checkout_available: boolean;
  current: boolean;
  quotas: RawPlanQuota[];
};

type RawPlansPayload = {
  plans: RawPlanRow[];
  current_plan_id: string;
};

type RawSubscriptionPayload = {
  subscription: {
    plan_id: string;
    status: string;
    current_period_end?: string | null;
  };
};

type RawUsagePayload = {
  usage: Record<string, number>;
};

async function decodeError(response: Response) {
  const raw = await response.text();

  if (!raw.trim()) {
    return new ApiError(response.status, null, `Request failed with status ${response.status}`);
  }

  try {
    const parsed = JSON.parse(raw) as ErrorEnvelope;
    return new ApiError(response.status, parsed.error ?? null, parsed.message ?? raw);
  } catch {
    return new ApiError(response.status, null, raw);
  }
}

async function request<T>(path: string, init: RequestInit = {}, token?: string) {
  const headers = new Headers(init.headers);

  if (!headers.has("Accept")) {
    headers.set("Accept", "application/json");
  }

  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  const response = await fetch(buildApiUrl(path), {
    ...init,
    cache: "no-store",
    headers,
  });

  if (!response.ok) {
    throw await decodeError(response);
  }

  return (await response.json()) as T;
}

function unwrapApiData<T>(envelope: ApiEnvelope<T>, fallback: string) {
  if (envelope.ok && envelope.data) {
    return envelope.data;
  }

  throw new Error(envelope.error?.message ?? fallback);
}

function parsePriceToCents(label: string) {
  const amount = Number.parseFloat(
    label
      .split("")
      .filter((character) => /\d|\./.test(character))
      .join(""),
  );

  if (Number.isNaN(amount)) {
    return 0;
  }

  return Math.round(amount * 100);
}

function quotaFeature(quota: RawPlanQuota) {
  const limit = quota.hard_limit ?? quota.soft_limit;

  if (typeof limit === "number") {
    return `${quota.metric_type}: ${limit}`;
  }

  return `${quota.metric_type}: unlimited`;
}

function usageValue(usage: Record<string, number>, key: string) {
  const value = usage[key];
  return typeof value === "number" ? value : 0;
}

export function defaultNotificationPreferences(): NotificationPreferences {
  return {
    email_enabled: true,
    product_enabled: true,
    security_enabled: true,
    weekly_digest_enabled: false,
    quiet_hours_start: null,
    quiet_hours_end: null,
  };
}

export async function updateProfile(token: string, full_name: string | null) {
  return request<AuthEnvelope>(
    "/api/auth/profile",
    {
      method: "PUT",
      body: JSON.stringify({ full_name }),
    },
    token,
  );
}

export async function getUserPreferences(token: string) {
  return request<UserPreferences>("/api/auth/preferences", { method: "GET" }, token);
}

export async function updateUserPreferences(token: string, preferences: UserPreferences) {
  return request<UserPreferences>(
    "/api/auth/preferences",
    {
      method: "PUT",
      body: JSON.stringify(preferences),
    },
    token,
  );
}

export async function listNotifications(token: string) {
  return request<NotificationsResponse>("/api/v1/notifications", { method: "GET" }, token);
}

export async function markNotificationRead(token: string, notificationId: string) {
  return request<Record<string, never>>(
    `/api/v1/notifications/${notificationId}/read`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}

export async function getUsageLimit(token: string) {
  return request<UsageLimitResponse>("/api/auth/usage-limit", { method: "GET" }, token);
}

export async function listPlans(token: string) {
  const payload = unwrapApiData(
    await request<ApiEnvelope<RawPlansPayload>>("/api/v1/billing/plans", { method: "GET" }, token),
    "Failed to load billing plans",
  );

  return {
    plans: payload.plans.map((plan) => ({
      id: plan.plan_id,
      name: plan.name,
      price: parsePriceToCents(plan.price_label),
      features: plan.quotas.length > 0 ? plan.quotas.map(quotaFeature) : [plan.description],
    })),
  } satisfies PlansResponse;
}

export async function getUsage(token: string) {
  const payload = unwrapApiData(
    await request<ApiEnvelope<RawUsagePayload>>("/api/v1/billing/usage", { method: "GET" }, token),
    "Failed to load billing usage",
  );

  return {
    used_tokens:
      usageValue(payload.usage, "embedding_tokens") +
      usageValue(payload.usage, "llm_input_tokens") +
      usageValue(payload.usage, "llm_output_tokens"),
    limit_tokens: 0,
    used_documents: usageValue(payload.usage, "pages_processed"),
    limit_documents: 0,
  } satisfies UsageResponse;
}

export async function getSubscription(token: string) {
  const payload = unwrapApiData(
    await request<ApiEnvelope<RawSubscriptionPayload>>(
      "/api/v1/billing/subscription",
      { method: "GET" },
      token,
    ),
    "Failed to load billing subscription",
  );

  return {
    plan_id: payload.subscription.plan_id,
    status: payload.subscription.status,
    current_period_end: payload.subscription.current_period_end ?? "",
  } satisfies SubscriptionResponse;
}

export async function createPortalSession(token: string) {
  return unwrapApiData(
    await request<ApiEnvelope<PortalSessionResponse>>(
      "/api/v1/billing/portal-session",
      {
        method: "POST",
        body: JSON.stringify({}),
      },
      token,
    ),
    "Failed to create billing portal",
  );
}
