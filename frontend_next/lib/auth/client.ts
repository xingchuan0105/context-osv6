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
  terms_version?: string;
  privacy_version?: string;
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

export {
  ApiError,
  buildApiUrl,
  fetchJsonRequest as request,
  getApiBaseUrl,
} from "../http/request";

import { request as apiRequest } from "../http/request";

export async function login(requestBody: LoginRequest) {
  return apiRequest<AuthEnvelope>("/api/auth/login", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function register(requestBody: RegisterRequest) {
  return apiRequest<AuthEnvelope>("/api/auth/register", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function me(token: string) {
  return apiRequest<AuthEnvelope>("/api/auth/me", { method: "GET" }, token);
}

export async function logout(token: string) {
  return apiRequest<AuthEnvelope>(
    "/api/auth/logout",
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}

export async function authRuntimeCapabilities() {
  return apiRequest<AuthRuntimeCapabilitiesResponse>("/api/auth/capabilities", {
    method: "GET",
  });
}

export async function changePassword(token: string, requestBody: ChangePasswordRequest) {
  return apiRequest<AuthEnvelope>(
    "/api/auth/change-password",
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function sendResetCode(requestBody: SendResetCodeRequest) {
  return apiRequest<AuthEnvelope>("/api/auth/reset/send-code", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function verifyResetCode(requestBody: VerifyResetCodeRequest) {
  return apiRequest<AuthEnvelope>("/api/auth/reset/verify-code", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}

export async function confirmResetPassword(requestBody: ConfirmResetPasswordRequest) {
  return apiRequest<AuthEnvelope>("/api/auth/reset/confirm", {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
}
