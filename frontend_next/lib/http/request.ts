import { restRequest } from "../runtime/transport";

type ErrorEnvelope = {
  error?: string | { message?: string | null } | null;
  message?: string | null;
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

export async function decodeApiError(response: Response) {
  const raw = await response.text();

  if (!raw.trim()) {
    return new ApiError(response.status, null, `Request failed with status ${response.status}`);
  }

  try {
    const parsed = JSON.parse(raw) as ErrorEnvelope;
    const errorCode = typeof parsed.error === "string" ? parsed.error : null;
    const nestedMessage =
      parsed.error && typeof parsed.error === "object" ? parsed.error.message : null;

    return new ApiError(
      response.status,
      errorCode,
      parsed.message ?? nestedMessage ?? errorCode ?? raw,
    );
  } catch {
    return new ApiError(response.status, null, raw);
  }
}

function prepareJsonRequestInit(init: RequestInit = {}, token?: string): RequestInit {
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

  return {
    ...init,
    cache: "no-store",
    headers,
  };
}

/**
 * Web-only JSON fetch used by `restRequest` on browser runtimes.
 */
export async function fetchJsonRequest<T>(
  path: string,
  init: RequestInit = {},
  token?: string,
): Promise<T> {
  const response = await fetch(buildApiUrl(path), prepareJsonRequestInit(init, token));

  if (!response.ok) {
    throw await decodeApiError(response);
  }

  return (await response.json()) as T;
}

/**
 * Domain HTTP client: routes through runtime transport (IPC or web fetch).
 */
export async function request<T>(path: string, init: RequestInit = {}, token?: string) {
  return restRequest<T>(path, prepareJsonRequestInit(init, token), token);
}

function resolveRequestUrl(pathOrUrl: string) {
  if (pathOrUrl.startsWith("http://") || pathOrUrl.startsWith("https://")) {
    return pathOrUrl;
  }

  return buildApiUrl(pathOrUrl);
}

export async function fetchResponse(
  pathOrUrl: string,
  init: RequestInit = {},
  options?: { token?: string },
) {
  const headers = new Headers(init.headers);

  if (options?.token) {
    headers.set("Authorization", `Bearer ${options.token}`);
  }

  const response = await fetch(resolveRequestUrl(pathOrUrl), {
    ...init,
    cache: "no-store",
    headers,
  });

  if (!response.ok) {
    throw await decodeApiError(response);
  }

  return response;
}

export async function requestText(path: string, init: RequestInit = {}, token?: string) {
  const response = await fetchResponse(path, init, { token });
  return response.text();
}

export type ApiEnvelope<T> = {
  ok?: boolean;
  data?: T | null;
  error?: {
    message?: string;
  } | null;
};

export async function requestEnvelope<T>(
  path: string,
  init: RequestInit = {},
  token?: string,
  fallback = "Request failed",
): Promise<T> {
  const env = await request<ApiEnvelope<T>>(path, init, token);
  if (env.ok && env.data) {
    return env.data;
  }
  throw new Error(env.error?.message ?? fallback);
}
