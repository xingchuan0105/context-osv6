import { ApiError, buildApiUrl, getApiBaseUrl } from "../auth/client";

export type ApiKeyRow = {
  id: string;
  org_id: string;
  notebook_id: string;
  key_prefix: string;
  name: string;
  permissions: string[];
  rate_limit_rpm: number;
  expires_at: string | null;
  last_used_at: string | null;
  is_active: boolean;
  created_by: string;
  created_at: string;
  updated_at: string;
};

export type ApiKeyListResponse = {
  api_keys: ApiKeyRow[];
};

export type CreateApiKeyRequest = {
  name: string;
  permissions: string[];
  rate_limit_rpm?: number;
  expires_at?: string | null;
};

export type CreateApiKeyResponse = {
  api_key: ApiKeyRow;
  plaintext_key: string;
};

type ErrorEnvelope = {
  error?: string | null;
  message?: string;
};

export function getApiAccessBaseUrl() {
  return getApiBaseUrl();
}

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

async function request<T>(path: string, init: RequestInit, token: string) {
  const headers = new Headers(init.headers);

  if (!headers.has("Accept")) {
    headers.set("Accept", "application/json");
  }

  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  headers.set("Authorization", `Bearer ${token}`);

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

export async function listApiKeys(token: string, workspaceId: string) {
  return request<ApiKeyListResponse>(`/api/v1/notebooks/${workspaceId}/api-keys`, { method: "GET" }, token);
}

export async function createApiKey(token: string, workspaceId: string, requestBody: CreateApiKeyRequest) {
  return request<CreateApiKeyResponse>(
    `/api/v1/notebooks/${workspaceId}/api-keys`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function revokeApiKey(token: string, workspaceId: string, keyId: string) {
  return request<Record<string, never>>(
    `/api/v1/notebooks/${workspaceId}/api-keys/${keyId}`,
    { method: "DELETE" },
    token,
  );
}
