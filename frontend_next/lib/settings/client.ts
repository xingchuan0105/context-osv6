import type { AuthEnvelope } from "../auth/client";
import { request, requestEnvelope } from "../http/request";

export type NotificationPreferences = {
  email_enabled: boolean;
  product_enabled: boolean;
  security_enabled: boolean;
  weekly_digest_enabled: boolean;
  quiet_hours_start: string | null;
  quiet_hours_end: string | null;
};

export type DashboardPreferences = {
  favorite_workspace_ids: string[];
  workspace_drafts: Array<Record<string, unknown>>;
  workspace_preferences: Array<Record<string, unknown>>;
  workspace_notes: Array<Record<string, unknown>>;
};

export type UserPreferences = {
  dashboard: DashboardPreferences;
  notifications: NotificationPreferences;
};

export type NotificationRow = {
  id: string;
  owner_user_id: string;
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

export type UpdateProfileInput = {
  full_name?: string | null;
  bio?: string | null;
  contact_url?: string | null;
};

export async function updateProfile(token: string, input: UpdateProfileInput | string | null) {
  // Accept legacy string full_name for callers mid-migration; prefer object form.
  const body: UpdateProfileInput =
    typeof input === "string" || input === null
      ? { full_name: input }
      : input;
  return request<AuthEnvelope>(
    "/api/auth/profile",
    {
      method: "PUT",
      body: JSON.stringify({
        full_name: body.full_name ?? null,
        bio: body.bio ?? null,
        contact_url: body.contact_url ?? null,
      }),
    },
    token,
  );
}

export async function uploadProfileMedia(
  token: string,
  kind: "avatar" | "banner",
  file: Blob,
  contentType: string,
) {
  return request<AuthEnvelope>(
    `/api/auth/profile/media/${kind}`,
    {
      method: "PUT",
      headers: {
        "Content-Type": contentType,
      },
      body: file,
    },
    token,
  );
}

export async function deleteProfileMedia(token: string, kind: "avatar" | "banner") {
  return request<AuthEnvelope>(
    `/api/auth/profile/media/${kind}`,
    {
      method: "DELETE",
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
  const payload = await requestEnvelope<RawPlansPayload>("/api/v1/billing/plans", { method: "GET" }, token, "Failed to load billing plans");

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
  const payload = await requestEnvelope<RawUsagePayload>("/api/v1/billing/usage", { method: "GET" }, token, "Failed to load billing usage");

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
  const payload = await requestEnvelope<RawSubscriptionPayload>(
    "/api/v1/billing/subscription",
    { method: "GET" },
    token,
    "Failed to load billing subscription",
  );

  return {
    plan_id: payload.subscription.plan_id,
    status: payload.subscription.status,
    current_period_end: payload.subscription.current_period_end ?? "",
  } satisfies SubscriptionResponse;
}

export async function createPortalSession(token: string) {
  return requestEnvelope<PortalSessionResponse>(
    "/api/v1/billing/portal-session",
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
    "Failed to create billing portal",
  );
}

export type CheckoutRequest = {
  plan_id?: string;
  /** Product checkout providers only (Stripe removed 2026-07-13). */
  provider?: "creem" | "alipay";
  /** `subscription` (default) or `wallet_topup` (ADR-0010 PR5). */
  kind?: "subscription" | "wallet_topup";
  /** Required when kind is wallet_topup (`topup_50` / `topup_100` / `topup_200`). */
  topup_pack_id?: string;
};

export type CheckoutResponse = {
  url: string;
  session_id: string;
  qr_code?: string | null;
  order_id?: string | null;
};

export type WalletBalanceResponse = {
  user_id: string;
  /** Spendable balance in fen (分). 2000 = ¥20. */
  balance_fen: number;
  /** Lifetime paid top-ups in fen (excludes gifts). */
  lifetime_paid_topup_fen: number;
};

export type TopupPack = {
  pack_id: string;
  amount_fen: number;
  amount_yuan: number;
  label_cny: string;
};

export async function createCheckoutSession(token: string, requestPayload: CheckoutRequest) {
  return requestEnvelope<CheckoutResponse>(
    "/api/v1/billing/checkout-session",
    {
      method: "POST",
      body: JSON.stringify(requestPayload),
    },
    token,
    "Failed to create checkout session",
  );
}

export async function getWalletBalance(token: string) {
  return requestEnvelope<WalletBalanceResponse>(
    "/api/v1/billing/wallet",
    { method: "GET" },
    token,
    "Failed to load wallet balance",
  );
}

export async function listTopupPacks(token: string) {
  return requestEnvelope<TopupPack[]>(
    "/api/v1/billing/wallet/topup-packs",
    { method: "GET" },
    token,
    "Failed to load top-up packs",
  );
}

export type BillingOrderStatusResponse = {
  order_id: string;
  status: "pending" | "paid";
  plan_id: string;
};

export async function getBillingOrderStatus(token: string, orderId: string) {
  return requestEnvelope<BillingOrderStatusResponse>(
    `/api/v1/billing/orders/${orderId}`,
    { method: "GET" },
    token,
    "Failed to load billing order",
  );
}

export type ReferralStats = {
  code: string;
  rewarded_count: number;
  quota: number;
  remaining: number;
};

export async function getReferralStats(token: string) {
  return requestEnvelope<ReferralStats>(
    "/api/v1/billing/referral",
    { method: "GET" },
    token,
    "Failed to load referral stats",
  );
}

export type ProviderSecretRow = {
  id: string;
  purpose: string;
  provider: string;
  base_url?: string | null;
  model_hint?: string | null;
  key_fingerprint: string;
  revoked_at?: string | null;
};

export async function listProviderSecrets(token: string) {
  return requestEnvelope<{ secrets: ProviderSecretRow[] }>(
    "/api/v1/settings/provider-secrets",
    { method: "GET" },
    token,
    "Failed to load provider secrets",
  );
}

export async function upsertProviderSecret(
  token: string,
  body: {
    purpose: "llm" | "embedding" | "rerank";
    provider: string;
    api_key: string;
    base_url?: string;
    model_hint?: string;
    workspace_id?: string | null;
  },
) {
  return requestEnvelope<ProviderSecretRow>(
    "/api/v1/settings/provider-secrets",
    { method: "PUT", body: JSON.stringify(body) },
    token,
    "Failed to save provider secret",
  );
}

export async function revokeProviderSecret(token: string, id: string) {
  return requestEnvelope<ProviderSecretRow>(
    `/api/v1/settings/provider-secrets/${id}`,
    { method: "DELETE" },
    token,
    "Failed to revoke provider secret",
  );
}

