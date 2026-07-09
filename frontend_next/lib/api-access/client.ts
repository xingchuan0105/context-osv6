import { getApiBaseUrl, request } from "../http/request";

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

export function getApiAccessBaseUrl() {
  return getApiBaseUrl();
}

export async function listApiKeys(token: string, workspaceId: string) {
  return request<ApiKeyListResponse>(`/api/v1/workspaces/${workspaceId}/api-keys`, { method: "GET" }, token);
}

export async function createApiKey(token: string, workspaceId: string, requestBody: CreateApiKeyRequest) {
  return request<CreateApiKeyResponse>(
    `/api/v1/workspaces/${workspaceId}/api-keys`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function revokeApiKey(token: string, workspaceId: string, keyId: string) {
  return request<Record<string, never>>(
    `/api/v1/workspaces/${workspaceId}/api-keys/${keyId}`,
    { method: "DELETE" },
    token,
  );
}
