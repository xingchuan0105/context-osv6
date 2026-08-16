/**
 * 桌面云登录 IPC（官方模型走余额,2026-08-15 wave W3）。
 *
 * 与 `tauri-local` 分离：这里只装云会话（登录 / 登出 / 会话视图）。
 * 登录全程在 Rust 侧走 HTTPS（reqwest），不依赖 WebView fetch / CORS。
 */

export type CloudUser = {
  id: string;
  email: string;
  full_name: string;
};

export type CloudRelayConfig = {
  base_url: string;
  chat_model: string;
  embedding_model: string;
};

/** Redacted view — tokens never cross to the WebView. */
export type CloudSessionView = {
  logged_in: boolean;
  cloud_base: string;
  user?: CloudUser | null;
  relay?: CloudRelayConfig | null;
  message: string;
};

export type CloudLoginResult = {
  user: CloudUser;
  relay: CloudRelayConfig;
  env_updated: boolean;
  product_restarted: boolean;
  message: string;
};

export type CloudLogoutResult = {
  logged_out: boolean;
  env_updated: boolean;
  product_restarted: boolean;
  message: string;
};

/** Redacted wallet view — balance only; 401/403 surfaces as an Error whose
 *  message is the re-login prompt (IPC code `cloud_session_expired`). */
export type CloudWalletView = {
  logged_in: boolean;
  /** Spendable balance in fen (分); divide by 100 for ¥ (CNY-denominated). */
  balance_fen: number;
};

function mapIpcError(error: unknown): Error {
  if (error instanceof Error) {
    return error;
  }
  if (typeof error === "string") {
    try {
      const parsed: unknown = JSON.parse(error);
      if (
        parsed &&
        typeof parsed === "object" &&
        "message" in parsed &&
        typeof (parsed as { message: unknown }).message === "string"
      ) {
        return new Error((parsed as { message: string }).message);
      }
    } catch {
      return new Error(error);
    }
    return new Error(error);
  }
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return new Error(message);
    }
  }
  return new Error(String(error));
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw mapIpcError(error);
  }
}

export async function getCloudSession(): Promise<CloudSessionView> {
  return invoke<CloudSessionView>("get_cloud_session");
}

/** Cloud login → mint desktop token → relay config → client.env → restart product. */
export async function cloudLogin(email: string, password: string): Promise<CloudLoginResult> {
  return invoke<CloudLoginResult>("cloud_login", { email, password });
}

/** Revoke the desktop token cloud-side (best-effort) and drop local relay creds. */
export async function cloudLogout(): Promise<CloudLogoutResult> {
  return invoke<CloudLogoutResult>("cloud_logout");
}

/** E2E-only: true when the shell was launched with CONTEXT_OS_SKIP_CLOUD_GATE=1
 *  (scripts/desktop-e2e; no real cloud account exists there). */
export async function isCloudGateBypassed(): Promise<boolean> {
  return invoke<boolean>("cloud_gate_bypassed");
}

/** Wallet balance for the signed-in cloud account (官方模型 走余额 metering source). */
export async function getCloudWalletBalance(): Promise<CloudWalletView> {
  return invoke<CloudWalletView>("cloud_wallet_balance");
}
