export type AuthUser = {
  id: string;
  email: string;
  full_name: string;
};

export type AuthPayload = {
  token: string;
  user: AuthUser;
  reset_ticket: string | null;
};

export type AuthEnvelope = {
  success: boolean;
  data: AuthPayload | null;
  error: string | null;
};

export type AuthRuntimeCapabilitiesResponse = {
  password_reset_enabled: boolean;
};

export type LoginRequest = {
  email: string;
  password: string;
};

export type RegisterRequest = {
  email: string;
  password: string;
  full_name?: string | null;
};

export type ChangePasswordRequest = {
  old_password: string;
  new_password: string;
};

export type SendResetCodeRequest = {
  email: string;
  lang: string;
};

export type VerifyResetCodeRequest = {
  email: string;
  code: string;
};

export type ConfirmResetPasswordRequest = {
  reset_ticket: string;
  new_password: string;
};

type ErrorEnvelope = {
  error: string;
  message: string;
};

const DEFAULT_API_BASE_URL = "";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string | null;

  constructor(status: number, code: string | null, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

export function getApiBaseUrl() {
  const configured = process.env.NEXT_PUBLIC_API_BASE_URL?.trim();

  return configured && configured.length > 0 ? configured : DEFAULT_API_BASE_URL;
}

export function buildApiUrl(path: string) {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  const configuredBaseUrl = getApiBaseUrl();

  if (configuredBaseUrl) {
    return `${configuredBaseUrl}${normalizedPath}`;
  }

  if (typeof window === "undefined") {
    return normalizedPath;
  }

  return new URL(normalizedPath, window.location.origin).toString();
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

export async function login(requestBody: LoginRequest) {
  return request<AuthEnvelope>("/api/auth/login", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function register(requestBody: RegisterRequest) {
  return request<AuthEnvelope>("/api/auth/register", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function me(token: string) {
  return request<AuthEnvelope>("/api/auth/me", { method: "GET" }, token);
}

export async function logout(token: string) {
  return request<AuthEnvelope>(
    "/api/auth/logout",
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}

export async function authRuntimeCapabilities() {
  return request<AuthRuntimeCapabilitiesResponse>("/api/auth/capabilities", {
    method: "GET",
  });
}

export async function changePassword(token: string, requestBody: ChangePasswordRequest) {
  return request<AuthEnvelope>(
    "/api/auth/change-password",
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function sendResetCode(requestBody: SendResetCodeRequest) {
  return request<AuthEnvelope>("/api/auth/reset/send-code", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function verifyResetCode(requestBody: VerifyResetCodeRequest) {
  return request<AuthEnvelope>("/api/auth/reset/verify-code", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function confirmResetPassword(requestBody: ConfirmResetPasswordRequest) {
  return request<AuthEnvelope>("/api/auth/reset/confirm", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}
