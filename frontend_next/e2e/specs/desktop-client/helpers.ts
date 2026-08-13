import fs from "fs";
import path from "path";

export function desktopAppDataDir() {
  return (
    process.env.APPDATA ??
    path.join(process.env.USERPROFILE ?? process.env.HOME ?? "", "AppData", "Roaming")
  );
}

export function readLocalSessionToken() {
  const sessionPath = path.join(desktopAppDataDir(), "com.contextos.desktop", "local_session.json");
  const session = JSON.parse(fs.readFileSync(sessionPath, "utf8")) as { token?: string };
  if (!session.token) {
    throw new Error("desktop local_session.json has no JWT");
  }
  return session.token;
}

type ApiEnvelope<T> = {
  ok?: boolean;
  data?: T | null;
  error?: { message?: string } | null;
};

async function apiEnvelope<T>(path: string, init: RequestInit, token: string) {
  const response = await fetch(`http://127.0.0.1:18080${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
      ...(init.headers ?? {}),
    },
  });
  const envelope = (await response.json()) as ApiEnvelope<T>;
  if (!response.ok || !envelope.ok) {
    throw new Error(envelope.error?.message ?? `desktop API request failed: ${response.status}`);
  }
  if (!envelope.data) {
    throw new Error("desktop API response had no data");
  }
  return envelope.data;
}

export type ProviderSecret = {
  id: string;
  purpose: string;
  provider: string;
  base_url?: string | null;
  model_hint?: string | null;
  key_fingerprint: string;
  revoked_at?: string | null;
};

export async function upsertProviderSecret(
  token: string,
  body: {
    purpose: string;
    provider: string;
    api_key: string;
    base_url?: string;
    model_hint?: string;
    workspace_id?: string | null;
  },
) {
  return apiEnvelope<ProviderSecret>(
    "/api/v1/settings/provider-secrets",
    { method: "PUT", body: JSON.stringify(body) },
    token,
  );
}

export async function listProviderSecrets(token: string) {
  return apiEnvelope<{ secrets: ProviderSecret[] }>(
    "/api/v1/settings/provider-secrets",
    { method: "GET" },
    token,
  );
}

export async function revokeProviderSecret(token: string, id: string) {
  return apiEnvelope<ProviderSecret>(
    `/api/v1/settings/provider-secrets/${id}`,
    { method: "DELETE" },
    token,
  );
}

/** Upsert a resolvable but dead (`127.0.0.1:9`) llm secret — proves the local API
 *  constructs the client from the secret (G1) instead of the legacy llm-config path. */
export async function upsertDummyProviderSecret(token: string) {
  return upsertProviderSecret(token, {
    purpose: "llm",
    provider: "custom",
    api_key: "e2e-not-a-real-key",
    base_url: "http://127.0.0.1:9",
    model_hint: "e2e-dummy",
    workspace_id: null,
  });
}

export async function clearLlmSecrets(token: string) {
  const { secrets } = await listProviderSecrets(token);
  await Promise.all(
    secrets
      .filter((secret) => secret.purpose === "llm" && !secret.revoked_at)
      .map((secret) => revokeProviderSecret(token, secret.id)),
  );
}
