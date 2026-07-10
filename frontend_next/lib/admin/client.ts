import { request, requestEnvelope, requestText, type ApiEnvelope } from "../http/request";

export type AdminAccountRow = {
  id: string;
  name: string;
  plan: string;
  user_count: number;
  workspace_count: number;
  query_count: number;
  blocked: boolean;
  created_at: number;
};

export type AdminUserRow = {
  id: string;
  email: string;
  full_name: string;
  owner_user_id: string;
  role: string;
  created_at: number;
  last_active_at: number | null;
};

export type AdminUsageResponse = {
  total_requests: number;
  total_tokens: number;
  total_documents: number;
};

export type AdminHealthResponse = {
  status: string;
  service: string;
  version: string;
};

export type AdminBillingOverview = {
  active_subscriptions: number;
  past_due_subscriptions: number;
  unpaid_subscriptions: number;
  canceled_subscriptions: number;
};

export type AdminRagHealthStatus = {
  failed_documents: number;
  queued_tasks: number;
  processing_tasks: number;
  recent_guard_events: number;
};

export type AdminFeatureFlagEntry = {
  key: string;
  category: string;
  description: string;
  enabled: boolean;
  effective_enabled: boolean;
  config_ready: boolean;
  requires_config: boolean;
  source: string;
  updated_at: number | null;
  has_pending_request: boolean;
};

export type AdminFeatureFlagChangeRequest = {
  id: string;
  flag_key: string;
  current_enabled: boolean;
  requested_enabled: boolean;
  reason: string;
  status: string;
  requested_by: string;
  reviewed_by: string | null;
  review_note: string | null;
  created_at: number;
  reviewed_at: number | null;
  executed_at: number | null;
};

export type AdminWorkerStatusResponse = {
  runtime_mode: string;
  queued_tasks: number;
  processing_tasks: number;
  failed_documents: number;
};

export type AdminDegradationStatusResponse = {
  failed_documents: number;
  recent_guard_events: number;
  share_access_events: number;
};

export type AdminAuditLogEntry = {
  id: number;
  actor_id: string | null;
  action: string;
  resource_type: string;
  resource_id: string;
  owner_user_id: string | null;
  created_at: number;
};

export type AdminAuditLogQuery = {
  query?: string | null;
  action?: string | null;
  resource_type?: string | null;
  actor?: string | null;
  window?: string | null;
  page?: number | null;
  per_page?: number | null;
};

export type AdminAuditLogListResponse = {
  items: AdminAuditLogEntry[];
  total: number;
  page: number;
  per_page: number;
};

type RawAccountRow = {
  id: string;
  name: string;
  created_at: number;
  blocked: boolean;
  user_count: number;
  document_count: number;
  query_count: number;
};

type RawUserRow = {
  id: string;
  email: string;
  owner_user_id?: string;
  role: string;
  created_at: number;
};

type RawUsageResponse = {
  owner_user_id: string;
  period: string;
  query_count: number;
  document_count: number;
  chunk_count: number;
  storage_bytes: number;
};

type RawHealthResponse = {
  status: string;
  version: string;
  uptime_secs: number;
};

function buildQuery(query: AdminAuditLogQuery) {
  const params = new URLSearchParams();

  for (const [key, value] of Object.entries(query)) {
    if (value === null || value === undefined) {
      continue;
    }

    const normalizedValue = typeof value === "string" ? value.trim() : value.toString();

    if (!normalizedValue) {
      continue;
    }

    params.set(key, normalizedValue);
  }

  return params.toString();
}

function mapAccountRow(raw: RawAccountRow): AdminAccountRow {
  return {
    id: raw.id,
    name: raw.name,
    plan: "N/A",
    user_count: raw.user_count,
    workspace_count: raw.document_count,
    query_count: raw.query_count,
    blocked: raw.blocked,
    created_at: raw.created_at,
  };
}

function mapUserRow(raw: RawUserRow): AdminUserRow {
  return {
    id: raw.id,
    email: raw.email,
    full_name: "",
    owner_user_id: raw.owner_user_id ?? raw.id,
    role: raw.role,
    created_at: raw.created_at,
    last_active_at: null,
  };
}

export async function listAdminAccounts(token: string) {
  const rows = await requestEnvelope<RawAccountRow[]>("/api/v1/admin/accounts", { method: "GET" }, token, "Failed to load accounts");

  return rows.map(mapAccountRow);
}

export async function getAdminAccount(token: string, ownerUserId: string) {
  const row = await requestEnvelope<RawAccountRow>(`/api/v1/admin/accounts/${ownerUserId}`, { method: "GET" }, token, "Failed to load account");

  return mapAccountRow(row);
}

export async function listAdminUsersForAccount(token: string, ownerUserId: string) {
  const rows = await requestEnvelope<RawUserRow[]>(`/api/v1/admin/users?owner_user_id=${encodeURIComponent(ownerUserId)}`, { method: "GET" }, token, "Failed to load users");

  return rows.map(mapUserRow);
}

export async function getAdminUsageForAccount(token: string, ownerUserId: string, period = "30d") {
  const usage = await requestEnvelope<RawUsageResponse>(
    `/api/v1/admin/usage?owner_user_id=${encodeURIComponent(ownerUserId)}&period=${encodeURIComponent(period)}`,
    { method: "GET" },
    token,
    "Failed to load usage",
  );

  return {
    total_requests: usage.query_count,
    total_tokens: usage.chunk_count,
    total_documents: usage.document_count,
  } satisfies AdminUsageResponse;
}

export async function updateAdminAccountBlocked(token: string, ownerUserId: string, blocked: boolean) {
  await request<ApiEnvelope<Record<string, never>>>(
    "/api/v1/admin/billing/block",
    {
      method: "POST",
      body: JSON.stringify({
        owner_user_id: ownerUserId,
        blocked,
      }),
    },
    token,
  );
}

export async function getAdminHealth(token: string) {
  const health = await requestEnvelope<RawHealthResponse>("/api/v1/admin/health", { method: "GET" }, token, "Failed to load health");

  return {
    status: health.status,
    service: "avrag-api",
    version: health.version,
  } satisfies AdminHealthResponse;
}

export async function getAdminBillingOverview(token: string) {
  return requestEnvelope<AdminBillingOverview>("/api/v1/admin/billing", { method: "GET" }, token, "Failed to load billing overview");
}

export async function getAdminRagHealth(token: string) {
  return requestEnvelope<AdminRagHealthStatus>("/api/v1/admin/rag-health", { method: "GET" }, token, "Failed to load rag health");
}

export async function listAdminFeatureFlags(token: string) {
  return requestEnvelope<AdminFeatureFlagEntry[]>("/api/v1/admin/feature-flags", { method: "GET" }, token, "Failed to load feature flags");
}

export async function requestAdminFeatureFlagChange(
  token: string,
  key: string,
  enabled: boolean,
  reason: string,
) {
  return requestEnvelope<AdminFeatureFlagChangeRequest>(
    `/api/v1/admin/feature-flags/${encodeURIComponent(key)}/change-requests`,
    {
      method: "POST",
      body: JSON.stringify({ enabled, reason }),
    },
    token,
    "Failed to request feature flag change",
  );
}

export async function reviewAdminFeatureFlagChange(
  token: string,
  requestId: string,
  approved: boolean,
  reviewNote?: string | null,
) {
  return requestEnvelope<AdminFeatureFlagChangeRequest>(
    `/api/v1/admin/feature-flags/change-requests/${encodeURIComponent(requestId)}/review`,
    {
      method: "POST",
      body: JSON.stringify({
        approved,
        review_note: reviewNote?.trim() ? reviewNote : undefined,
      }),
    },
    token,
    "Failed to review feature flag change",
  );
}

export async function listAdminFeatureFlagChangeRequests(token: string, status?: string | null) {
  const query = status?.trim() ? `?status=${encodeURIComponent(status)}` : "";

  return requestEnvelope<AdminFeatureFlagChangeRequest[]>(
    `/api/v1/admin/feature-flags/change-requests${query}`,
    { method: "GET" },
    token,
    "Failed to load feature flag change requests",
  );
}

export async function getAdminWorkerStatus(token: string) {
  return requestEnvelope<AdminWorkerStatusResponse>("/api/v1/admin/system/workers", { method: "GET" }, token, "Failed to load worker status");
}

export async function getAdminDegradationStatus(token: string) {
  return requestEnvelope<AdminDegradationStatusResponse>("/api/v1/admin/system/degradation", { method: "GET" }, token, "Failed to load degradation status");
}

export async function listAdminAuditLogs(token: string, query: AdminAuditLogQuery) {
  const suffix = buildQuery(query);

  return requestEnvelope<AdminAuditLogListResponse>(
    suffix ? `/api/v1/admin/audit-logs?${suffix}` : "/api/v1/admin/audit-logs",
    { method: "GET" },
    token,
    "Failed to load audit logs",
  );
}

export async function exportAdminAuditLogsCsv(token: string, query: AdminAuditLogQuery) {
  const suffix = buildQuery(query);
  const base = suffix ? `${suffix}&format=csv` : "format=csv";

  return requestText(`/api/v1/admin/audit-logs?${base}`, { method: "GET" }, token);
}
