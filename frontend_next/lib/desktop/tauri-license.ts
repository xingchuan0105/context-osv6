import { isTauri } from "@/lib/runtime/tauri-ipc";

export type LicenseStatusKind =
  | "unactivated"
  | "trial"
  | "active"
  | "expired"
  | "revoked"
  | "offline_grace";

export type LicenseKind = "trial" | "standard" | "pro";

export type LicenseStatus = {
  kind: LicenseStatusKind;
  days_remaining?: number | null;
  offline_grace_days?: number | null;
  license_kind?: LicenseKind | null;
  expires_at?: number | null;
  dev_mode: boolean;
};

export type ActivationResult = {
  license_id: string;
  kind: LicenseKind;
  status: LicenseStatusKind;
};

export type TrialResult = {
  expires_at: number;
  days_remaining: number;
};

export type LicenseErrorPayload = {
  code: string;
  message: string;
};

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(command, args);
}

export function formatLicenseError(error: unknown): string {
  if (error && typeof error === "object") {
    const payload = error as Partial<LicenseErrorPayload>;
    if (typeof payload.message === "string" && payload.message.length > 0) {
      return payload.message;
    }
  }

  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error) as Partial<LicenseErrorPayload>;
      if (typeof parsed.message === "string" && parsed.message.length > 0) {
        return parsed.message;
      }
    } catch {
      return error;
    }
    return error;
  }

  return error instanceof Error ? error.message : "授权操作失败";
}

export async function getDeviceId(): Promise<string> {
  return invoke<string>("get_device_id");
}

export async function startTrial(): Promise<TrialResult> {
  return invoke<TrialResult>("start_trial");
}

export async function activateLicense(licenseKey: string): Promise<ActivationResult> {
  return invoke<ActivationResult>("activate_license", { license_key: licenseKey });
}

export async function getLicenseStatus(): Promise<LicenseStatus> {
  return invoke<LicenseStatus>("get_license_status");
}

export async function openInBrowser(url: string): Promise<void> {
  await invoke("open_in_browser", { url });
}

export async function listenDeepLinkActivate(onKey: (key: string) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<string>("deep-link-activate", (event) => {
    onKey(event.payload);
  });
  return unlisten;
}

export function licenseKindLabel(kind: LicenseKind): string {
  if (kind === "pro") return "AVRag Desktop Pro";
  if (kind === "standard") return "AVRag Desktop Standard";
  return "AVRag Desktop Trial";
}

export function licenseSeatsMax(kind: LicenseKind): number {
  return kind === "pro" ? 3 : 1;
}

export function licenseTypeLabel(kind: LicenseKind, daysRemaining?: number | null): string {
  if (kind === "trial") {
    return `试用（剩余 ${daysRemaining ?? 7} 天）`;
  }
  return "永久（v1.x 终身免费升级）";
}

export function isDesktopRuntime(): boolean {
  return isTauri();
}
