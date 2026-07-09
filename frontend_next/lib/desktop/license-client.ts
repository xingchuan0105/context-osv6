import { requestEnvelope } from "@/lib/http/request";

export type LicenseSummary = {
  id: string;
  key: string;
  status: string;
  kind: string;
  max_machines?: number | null;
  machines_count?: number | null;
  metadata?: Record<string, unknown> | null;
  created_at?: string | null;
};

export type LicenseMachine = {
  id: string;
  fingerprint?: string | null;
  name?: string | null;
  platform?: string | null;
  last_heartbeat_at?: string | null;
  created_at?: string | null;
};

export type LicenseListResponse = {
  licenses: LicenseSummary[];
};

export type LicenseMachineListResponse = {
  machines: LicenseMachine[];
};

export type CreateLicenseCheckoutRequest = {
  plan_id: string;
  provider?: string;
  device_id?: string;
};

export type CreateLicenseCheckoutResponse = {
  checkout_url: string;
  session_id: string;
  plan_id: string;
};

export async function fetchMyLicenses(token: string): Promise<LicenseListResponse> {
  return requestEnvelope<LicenseListResponse>(
    "/api/v1/licenses/me",
    { method: "GET" },
    token,
    "加载授权列表失败",
  );
}

export async function fetchLicenseMachines(
  token: string,
  licenseId: string,
): Promise<LicenseMachineListResponse> {
  return requestEnvelope<LicenseMachineListResponse>(
    `/api/v1/licenses/${encodeURIComponent(licenseId)}/machines`,
    { method: "GET" },
    token,
    "加载设备列表失败",
  );
}

export async function deactivateLicenseMachine(
  token: string,
  licenseId: string,
  machineId: string,
): Promise<void> {
  await requestEnvelope<{ deactivated: boolean }>(
    `/api/v1/licenses/${encodeURIComponent(licenseId)}/machines/${encodeURIComponent(machineId)}`,
    { method: "DELETE" },
    token,
    "解绑设备失败",
  );
}

export async function createLicenseCheckout(
  token: string,
  request: CreateLicenseCheckoutRequest,
): Promise<CreateLicenseCheckoutResponse> {
  return requestEnvelope<CreateLicenseCheckoutResponse>(
    "/api/v1/licenses/checkout",
    {
      method: "POST",
      body: JSON.stringify(request),
    },
    token,
    "创建结账会话失败",
  );
}

export function licenseStatusLabel(status: string): string {
  const normalized = status.toLowerCase();
  if (normalized.includes("trial")) return "试用中";
  if (normalized.includes("expired")) return "已过期";
  if (normalized.includes("revoked") || normalized.includes("suspend")) return "已吊销";
  if (normalized.includes("active")) return "已激活";
  return status;
}

export function licenseKindDisplay(kind: string): string {
  const normalized = kind.toLowerCase();
  if (normalized.includes("pro")) return "AVRag Desktop Pro";
  if (normalized.includes("standard")) return "AVRag Desktop Standard";
  if (normalized.includes("trial")) return "AVRag Desktop Trial";
  return kind;
}

export function formatHeartbeatLabel(value?: string | null): string {
  if (!value) return "未知";
  const parsed = Date.parse(value);
  if (Number.isNaN(parsed)) return value;

  const diffMs = Date.now() - parsed;
  const diffHours = Math.floor(diffMs / (60 * 60 * 1000));
  if (diffHours < 1) return "1 小时内";
  if (diffHours < 24) return `${diffHours} 小时前`;
  const diffDays = Math.floor(diffHours / 24);
  return `${diffDays} 天前`;
}
