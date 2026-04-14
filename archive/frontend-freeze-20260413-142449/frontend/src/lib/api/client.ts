import axios, { AxiosError, type AxiosProgressEvent } from "axios";
import { BUILTIN_CHAT_AGENTS } from "@/lib/agents";
import { normalizeDocumentStatusValue } from "@/lib/document-status";
import type {
  Agent,
  AuthResponse,
  BillingPlan,
  BillingPlanQuota,
  BillingSubscription,
  BillingUsage,
  BillingUsageMetric,
  ChatMessage,
  ChatSession,
  Citation,
  CitationLookupResponse,
  Document,
  FavoriteKnowledgeBase,
  KnowledgeBase,
  Notification,
  NotebookAPIKey,
  Note,
  SearchResult,
  ShareMember,
  ShareSettings,
  ShareTokenInfo,
  User,
} from "@/types";

type ApiResult<T> =
  | {
      success: true;
      data: T;
      error?: string;
      error_code?: string;
      message?: string;
    }
  | {
      success: false;
      error: string;
      error_code?: string;
      message?: string;
      data?: undefined;
    };

const API_BASE_URL = "";
const API_TIMEOUT_MS = Number(process.env.NEXT_PUBLIC_API_TIMEOUT || 30000);

const STORAGE_AUTH_TOKEN = "token";
const STORAGE_RUNTIME_ORG_ID = "runtime_org_id";
const STORAGE_RUNTIME_USER_ID = "runtime_user_id";
const STORAGE_RUNTIME_EMAIL = "runtime_user_email";
const STORAGE_RUNTIME_NAME = "runtime_user_name";
const STORAGE_LOCAL_NOTEBOOKS = "local_notebooks_v1";
const STORAGE_LOCAL_NOTES = "local_notes_v1";
const STORAGE_LOCAL_DOCUMENTS = "local_documents_v1";
const STORAGE_LOCAL_CHAT_MESSAGES = "local_chat_messages_v1";
const STORAGE_LOCAL_DELETED_SESSIONS = "local_deleted_sessions_v1";
const STORAGE_LOCAL_FAVORITES = "local_favorites_v1";
const BILLING_USAGE_METRICS: BillingUsageMetric[] = [
  "pages_processed",
  "embedding_tokens",
  "llm_input_tokens",
  "llm_output_tokens",
  "storage_bytes",
];

const reUUID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export const api = axios.create({
  baseURL: API_BASE_URL,
  timeout: API_TIMEOUT_MS,
  headers: {
    "Content-Type": "application/json",
  },
});

function isBrowser(): boolean {
  return typeof window !== "undefined";
}

function readStorage(key: string): string {
  if (!isBrowser()) return "";
  return localStorage.getItem(key) || "";
}

function writeStorage(key: string, value: string): void {
  if (!isBrowser()) return;
  if (value) {
    localStorage.setItem(key, value);
    return;
  }
  localStorage.removeItem(key);
}

function readJSON<T>(key: string, fallback: T): T {
  if (!isBrowser()) return fallback;
  const raw = localStorage.getItem(key);
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function writeJSON<T>(key: string, value: T): void {
  if (!isBrowser()) return;
  localStorage.setItem(key, JSON.stringify(value));
}

function ok<T>(data: T): ApiResult<T> {
  return { success: true, data };
}

function fail<T = never>(error: string, errorCode?: string): ApiResult<T> {
  return {
    success: false,
    error,
    ...(errorCode ? { error_code: errorCode } : {}),
  };
}

function nowISO(): string {
  return new Date().toISOString();
}

function randomID(prefix: string): string {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    return crypto.randomUUID();
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function statusCodeOf(error: unknown): number {
  const axiosErr = error as AxiosError<{ error?: string; message?: string }>;
  return Number(axiosErr?.response?.status || 0);
}

function errorMessageOf(error: unknown, fallback = "Request failed"): string {
  const axiosErr = error as AxiosError<{ error?: string; message?: string }>;
  if (axiosErr?.response?.data?.error) return axiosErr.response.data.error;
  if (axiosErr?.response?.data?.message) return axiosErr.response.data.message;
  if (axiosErr?.message) return axiosErr.message;
  return fallback;
}

function normalizeNoteType(
  noteType?: string,
): "draft" | "committed" | undefined {
  if (noteType === "draft" || noteType === "committed") return noteType;
  return undefined;
}

function normalizeNullableString(value: unknown): string | undefined {
  if (typeof value === "string") return value;
  if (value && typeof value === "object") {
    const candidate = value as { String?: unknown; Valid?: unknown };
    if (candidate.Valid === true && typeof candidate.String === "string") {
      return candidate.String;
    }
  }
  return undefined;
}

function normalizeNotePayload<T extends Record<string, any>>(note: T): T {
  const normalizedTitle = normalizeNullableString(note.title);
  const normalizedNoteType = normalizeNullableString(note.note_type);
  return {
    ...note,
    title: normalizedTitle,
    note_type: normalizeNoteType(normalizedNoteType),
  };
}

function clearRuntimeIdentity(): void {
  writeStorage(STORAGE_RUNTIME_ORG_ID, "");
  writeStorage(STORAGE_RUNTIME_USER_ID, "");
  writeStorage(STORAGE_RUNTIME_EMAIL, "");
  writeStorage(STORAGE_RUNTIME_NAME, "");
}

function rememberAuthenticatedUser(
  user?: User,
  tokenHint?: string | null,
): void {
  const token = (tokenHint || getStoredAuthToken()).trim();
  const claims = token ? decodeJWTPayload(token) : null;

  const orgID = typeof claims?.org_id === "string" ? claims.org_id.trim() : "";
  const userIDFromClaims =
    typeof claims?.user_id === "string"
      ? claims.user_id.trim()
      : typeof claims?.sub === "string"
        ? claims.sub.trim()
        : "";

  if (reUUID.test(orgID)) {
    writeStorage(STORAGE_RUNTIME_ORG_ID, orgID);
  }
  if (user?.id && reUUID.test(user.id)) {
    writeStorage(STORAGE_RUNTIME_USER_ID, user.id);
  } else if (reUUID.test(userIDFromClaims)) {
    writeStorage(STORAGE_RUNTIME_USER_ID, userIDFromClaims);
  }
  if (user?.email) {
    writeStorage(STORAGE_RUNTIME_EMAIL, user.email);
  }
  if (user?.full_name) {
    writeStorage(STORAGE_RUNTIME_NAME, user.full_name);
  }
}

function getRuntimeUserID(): string {
  const userID = readStorage(STORAGE_RUNTIME_USER_ID);
  return reUUID.test(userID) ? userID : "";
}

function getCachedRuntimeUser(): User | null {
  const id = getRuntimeUserID();
  const email = readStorage(STORAGE_RUNTIME_EMAIL).trim();
  const fullName = readStorage(STORAGE_RUNTIME_NAME).trim();
  if (!id || !email) {
    return null;
  }
  return {
    id,
    email,
    ...(fullName ? { full_name: fullName } : {}),
  };
}

function mapNotebookFromV5(raw: Record<string, any>): KnowledgeBase {
  return {
    id: String(raw.id || ""),
    user_id: String(raw.owner_id || getRuntimeUserID()),
    title: String(raw.name || raw.title || "Untitled Notebook"),
    icon: typeof raw.icon === "string" ? raw.icon : undefined,
    description: typeof raw.description === "string" ? raw.description : "",
    created_at: typeof raw.created_at === "string" ? raw.created_at : nowISO(),
  };
}

function mapNotebookAPIKey(raw: Record<string, any>): NotebookAPIKey {
  return {
    id: String(raw.id || ""),
    org_id: String(raw.org_id || ""),
    notebook_id:
      typeof raw.notebook_id === "string" ? raw.notebook_id : undefined,
    key_prefix: String(raw.key_prefix || ""),
    name: String(raw.name || ""),
    permissions: Array.isArray(raw.permissions)
      ? raw.permissions.map((item) => String(item))
      : [],
    rate_limit_rpm: Number(raw.rate_limit_rpm || 0) || 60,
    expires_at:
      typeof raw.expires_at === "string" ? raw.expires_at : undefined,
    last_used_at:
      typeof raw.last_used_at === "string" ? raw.last_used_at : undefined,
    is_active: Boolean(raw.is_active),
    created_by: String(raw.created_by || ""),
    created_at: typeof raw.created_at === "string" ? raw.created_at : nowISO(),
    updated_at: typeof raw.updated_at === "string" ? raw.updated_at : nowISO(),
  };
}

function mapNotification(raw: Record<string, any>): Notification {
  const payload =
    raw.data && typeof raw.data === "object" && !Array.isArray(raw.data)
      ? (raw.data as Record<string, unknown>)
      : {};

  return {
    id: String(raw.id || ""),
    org_id: String(raw.org_id || ""),
    user_id: String(raw.user_id || ""),
    event_type: String(raw.event_type || ""),
    title: String(raw.title || ""),
    body: String(raw.body || ""),
    data: payload,
    read_at: typeof raw.read_at === "string" ? raw.read_at : undefined,
    created_at: typeof raw.created_at === "string" ? raw.created_at : nowISO(),
    updated_at: typeof raw.updated_at === "string" ? raw.updated_at : nowISO(),
  };
}

function mapShareTokenInfo(raw: Record<string, any>): ShareTokenInfo {
  return {
    token: String(raw.token || ""),
    access_level: String(raw.access_level || "read"),
    expires_at:
      typeof raw.expires_at === "string" ? raw.expires_at : undefined,
    revoked_at:
      typeof raw.revoked_at === "string" ? raw.revoked_at : undefined,
    access_count: Number(raw.access_count || 0) || 0,
  };
}

function mapShareMember(raw: Record<string, any>): ShareMember {
  return {
    id: String(raw.id || ""),
    notebook_id: String(raw.notebook_id || ""),
    user_id: typeof raw.user_id === "string" ? raw.user_id : undefined,
    email: typeof raw.email === "string" ? raw.email : undefined,
    access_level: String(raw.access_level || "Read"),
    invite_status: String(raw.invite_status || "accepted"),
    invited_by:
      typeof raw.invited_by === "string" ? raw.invited_by : undefined,
    invited_at: Number(raw.invited_at || 0) || 0,
    accepted_at:
      typeof raw.accepted_at === "number" ? raw.accepted_at : undefined,
  };
}

function mapShareSettings(raw: Record<string, any>): ShareSettings {
  const shareTokens = Array.isArray(raw.share_tokens)
    ? raw.share_tokens
        .filter((item) => item && typeof item === "object")
        .map((item) => mapShareTokenInfo(item as Record<string, any>))
    : [];
  const members = Array.isArray(raw.members)
    ? raw.members
        .filter((item) => item && typeof item === "object")
        .map((item) => mapShareMember(item as Record<string, any>))
    : [];
  return {
    access_level: String(raw.access_level || "private"),
    share_tokens: shareTokens,
    members,
  };
}

function mapBillingPlanQuota(raw: Record<string, any>): BillingPlanQuota {
  return {
    metric_type: String(raw.metric_type || "") as BillingUsageMetric,
    soft_limit:
      typeof raw.soft_limit === "number" ? raw.soft_limit : undefined,
    hard_limit:
      typeof raw.hard_limit === "number" ? raw.hard_limit : undefined,
  };
}

function mapBillingPlan(raw: Record<string, any>): BillingPlan {
  const quotas = Array.isArray(raw.quotas)
    ? raw.quotas
        .filter((item) => item && typeof item === "object")
        .map((item) => mapBillingPlanQuota(item as Record<string, any>))
    : [];

  return {
    plan_id: String(raw.plan_id || ""),
    name: String(raw.name || ""),
    description: String(raw.description || ""),
    price_label: String(raw.price_label || ""),
    interval: String(raw.interval || ""),
    checkout_available: Boolean(raw.checkout_available),
    current: Boolean(raw.current),
    quotas,
  };
}

function mapBillingSubscription(raw: Record<string, any>): BillingSubscription {
  return {
    id: String(raw.id || raw.ID || ""),
    org_id: String(raw.org_id || raw.OrgID || ""),
    stripe_subscription_id:
      typeof raw.stripe_subscription_id === "string"
        ? raw.stripe_subscription_id
        : typeof raw.StripeSubscriptionID === "string"
          ? raw.StripeSubscriptionID
          : undefined,
    stripe_price_id:
      typeof raw.stripe_price_id === "string"
        ? raw.stripe_price_id
        : typeof raw.StripePriceID === "string"
          ? raw.StripePriceID
          : undefined,
    plan_id: String(raw.plan_id || raw.PlanID || "free"),
    status: String(raw.status || raw.Status || "active"),
    current_period_start:
      typeof raw.current_period_start === "string"
        ? raw.current_period_start
        : typeof raw.CurrentPeriodStart === "string"
          ? raw.CurrentPeriodStart
          : undefined,
    current_period_end:
      typeof raw.current_period_end === "string"
        ? raw.current_period_end
        : typeof raw.CurrentPeriodEnd === "string"
          ? raw.CurrentPeriodEnd
          : undefined,
    cancel_at_period_end:
      typeof raw.cancel_at_period_end === "boolean"
        ? raw.cancel_at_period_end
        : Boolean(raw.CancelAtPeriodEnd),
    created_at:
      typeof raw.created_at === "string"
        ? raw.created_at
        : typeof raw.CreatedAt === "string"
          ? raw.CreatedAt
          : undefined,
    updated_at:
      typeof raw.updated_at === "string"
        ? raw.updated_at
        : typeof raw.UpdatedAt === "string"
          ? raw.UpdatedAt
          : undefined,
  };
}

function emptyBillingUsage(): BillingUsage {
  return {
    pages_processed: 0,
    embedding_tokens: 0,
    llm_input_tokens: 0,
    llm_output_tokens: 0,
    storage_bytes: 0,
  };
}

function mapBillingUsage(raw: Record<string, any>): BillingUsage {
  const usage = emptyBillingUsage();
  for (const metric of BILLING_USAGE_METRICS) {
    usage[metric] = Number(raw[metric] || 0) || 0;
  }
  return usage;
}

function mapSessionFromV5(raw: Record<string, any>): ChatSession {
  const notebookID = typeof raw.notebook_id === "string" ? raw.notebook_id : "";
  return {
    id: String(raw.id || ""),
    kb_id: notebookID,
    user_id: getRuntimeUserID(),
    title: typeof raw.title === "string" ? raw.title : undefined,
    summary: typeof raw.summary === "string" ? raw.summary : undefined,
    source_type: "owner",
    source_token: undefined,
    created_at: typeof raw.created_at === "string" ? raw.created_at : nowISO(),
    updated_at:
      typeof raw.updated_at === "string"
        ? raw.updated_at
        : typeof raw.created_at === "string"
          ? raw.created_at
          : nowISO(),
  };
}

function normalizeDocumentStatus(status: unknown): Document["status"] {
  return normalizeDocumentStatusValue(status) ?? "pending";
}

function mapDocumentFromV5(raw: Record<string, any>): Document {
  const displayName =
    (typeof raw.file_name === "string" && raw.file_name) ||
    (typeof raw.title === "string" && raw.title) ||
    (typeof raw.filename === "string" && raw.filename) ||
    "Untitled Document";
  return {
    id: String(raw.id || raw.document_id || randomID("doc")),
    kb_id: String(raw.notebook_id || raw.kb_id || ""),
    user_id: String(raw.user_id || getRuntimeUserID()),
    file_name: String(displayName),
    storage_path: typeof raw.file_path === "string" ? raw.file_path : undefined,
    mime_type: typeof raw.mime_type === "string" ? raw.mime_type : undefined,
    file_size: Number(raw.file_size || 0) || 0,
    status: normalizeDocumentStatus(raw.status),
    summary_global:
      typeof raw.summary_global === "string" ? raw.summary_global : undefined,
    content: typeof raw.content === "string" ? raw.content : undefined,
    chunk_count: Number(raw.chunk_count || 0) || 0,
    created_at: typeof raw.created_at === "string" ? raw.created_at : nowISO(),
  };
}

function readLocalNotes(): Note[] {
  return readJSON<Note[]>(STORAGE_LOCAL_NOTES, []);
}

function writeLocalNotes(notes: Note[]): void {
  writeJSON(STORAGE_LOCAL_NOTES, notes);
}

function readLocalDocuments(): Document[] {
  return readJSON<Document[]>(STORAGE_LOCAL_DOCUMENTS, []);
}

function writeLocalDocuments(docs: Document[]): void {
  writeJSON(STORAGE_LOCAL_DOCUMENTS, docs);
}

function readLocalChatMessages(): Record<string, ChatMessage[]> {
  return readJSON<Record<string, ChatMessage[]>>(
    STORAGE_LOCAL_CHAT_MESSAGES,
    {},
  );
}

function writeLocalChatMessages(payload: Record<string, ChatMessage[]>): void {
  writeJSON(STORAGE_LOCAL_CHAT_MESSAGES, payload);
}

function setLocalSessionMessages(
  sessionID: string,
  messages: ChatMessage[],
): void {
  const all = readLocalChatMessages();
  all[sessionID] = messages;
  writeLocalChatMessages(all);
}

function clearLocalSessionMessages(sessionID: string): void {
  const all = readLocalChatMessages();
  delete all[sessionID];
  writeLocalChatMessages(all);
}

function readDeletedSessions(): string[] {
  return readJSON<string[]>(STORAGE_LOCAL_DELETED_SESSIONS, []);
}

function writeDeletedSessions(ids: string[]): void {
  writeJSON(STORAGE_LOCAL_DELETED_SESSIONS, ids);
}

function readFavorites(): FavoriteKnowledgeBase[] {
  return readJSON<FavoriteKnowledgeBase[]>(STORAGE_LOCAL_FAVORITES, []);
}

function writeFavorites(favorites: FavoriteKnowledgeBase[]): void {
  writeJSON(STORAGE_LOCAL_FAVORITES, favorites);
}

function readLocalNotebooks(): KnowledgeBase[] {
  return readJSON<KnowledgeBase[]>(STORAGE_LOCAL_NOTEBOOKS, []);
}

function writeLocalNotebooks(items: KnowledgeBase[]): void {
  writeJSON(STORAGE_LOCAL_NOTEBOOKS, items);
}

function getStoredAuthToken(): string {
  return readStorage(STORAGE_AUTH_TOKEN).trim();
}

function decodeJWTPayload(token: string): Record<string, unknown> | null {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  try {
    const segment = parts[1];
    const normalized = segment + "=".repeat((4 - (segment.length % 4)) % 4);
    const json = atob(normalized.replace(/-/g, "+").replace(/_/g, "/"));
    return JSON.parse(json) as Record<string, unknown>;
  } catch {
    return null;
  }
}

function isJWTTokenUsable(token: string): boolean {
  if (!token) return false;
  const payload = decodeJWTPayload(token);
  if (!payload) return true;

  const exp = Number(payload.exp || 0);
  if (!exp) return true;

  const now = Math.floor(Date.now() / 1000);
  return exp > now + 30;
}

api.interceptors.request.use(
  (config) => {
    const token = getAuthToken();

    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => Promise.reject(error),
);

api.interceptors.response.use(
  (response) => response,
  (error) => Promise.reject(error),
);

export function setAuthToken(token: string): void {
  writeStorage(STORAGE_AUTH_TOKEN, token.trim());
}

export function clearAuthToken(): void {
  writeStorage(STORAGE_AUTH_TOKEN, "");
  clearRuntimeIdentity();
}

export function getAuthToken(): string | null {
  const storedToken = getStoredAuthToken();
  if (storedToken) {
    if (isJWTTokenUsable(storedToken)) {
      return storedToken;
    }
    clearAuthToken();
  }
  return null;
}

export function hasUsableAuthToken(): boolean {
  return Boolean(getAuthToken());
}

export function getCachedAuthUser(): User | null {
  if (!hasUsableAuthToken()) {
    return null;
  }
  return getCachedRuntimeUser();
}

function normalizeAuthResponse(payload: any): AuthResponse {
  if (
    payload &&
    typeof payload === "object" &&
    typeof payload.success === "boolean"
  ) {
    return payload as AuthResponse;
  }
  return fail<AuthResponse["data"]>("invalid auth response") as AuthResponse;
}

export const authApi = {
  login: async (email: string, password: string): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>("/api/auth/login", {
        email,
        password,
      });
      const payload = normalizeAuthResponse(response.data);
      if (payload.success && payload.data?.token) {
        setAuthToken(payload.data.token);
        rememberAuthenticatedUser(payload.data.user, payload.data.token);
      }
      return payload;
    } catch (error) {
      clearAuthToken();
      return fail(errorMessageOf(error)) as AuthResponse;
    }
  },

  register: async (
    email: string,
    password: string,
    fullName?: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>("/api/auth/register", {
        email,
        password,
        full_name: fullName,
      });
      const payload = normalizeAuthResponse(response.data);
      if (payload.success && payload.data?.token) {
        setAuthToken(payload.data.token);
        rememberAuthenticatedUser(payload.data.user, payload.data.token);
      }
      return payload;
    } catch (error) {
      clearAuthToken();
      return fail(errorMessageOf(error)) as AuthResponse;
    }
  },

  logout: async (): Promise<void> => {
    try {
      await api.post("/api/auth/logout");
    } catch {
      // Ignore legacy endpoint failure.
    } finally {
      clearAuthToken();
    }
  },

  me: async (): Promise<AuthResponse> => {
    try {
      const response = await api.get<AuthResponse>("/api/auth/me");
      const payload = normalizeAuthResponse(response.data);
      if (payload.success && payload.data?.user) {
        rememberAuthenticatedUser(payload.data.user);
        return payload;
      }
    } catch (error) {
      const status = statusCodeOf(error);
      if (status === 401 || status === 403) {
        clearAuthToken();
        return fail("unauthorized", "UNAUTHORIZED") as AuthResponse;
      }
      return fail(errorMessageOf(error)) as AuthResponse;
    }

    const cachedUser = getCachedAuthUser();
    if (cachedUser) {
      return ok({
        token: "",
        user: cachedUser,
      }) as AuthResponse;
    }
    return fail("unauthorized", "UNAUTHORIZED") as AuthResponse;
  },

  updateProfile: async (data: {
    full_name?: string;
  }): Promise<AuthResponse> => {
    try {
      const response = await api.put<AuthResponse>("/api/auth/profile", data);
      const payload = normalizeAuthResponse(response.data);
      if (payload.success && payload.data?.user) {
        rememberAuthenticatedUser(payload.data.user);
      }
      return payload;
    } catch (error) {
      return fail(errorMessageOf(error)) as AuthResponse;
    }
  },

  changePassword: async (
    _oldPassword: string,
    _newPassword: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/change-password",
        {
          old_password: _oldPassword,
          new_password: _newPassword,
        },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "password change is not available"),
      ) as AuthResponse;
    }
  },

  forgotPassword: async (email: string): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>("/api/auth/reset-request", {
        email,
      });
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "password reset is not enabled"),
      ) as AuthResponse;
    }
  },

  verifyResetToken: async (token: string): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/verify-reset-token",
        { token },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "password reset is not enabled"),
      ) as AuthResponse;
    }
  },

  resetPassword: async (
    token: string,
    newPassword: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/reset-password",
        {
          token,
          new_password: newPassword,
        },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "password reset is not enabled"),
      ) as AuthResponse;
    }
  },

  sendResetCode: async (
    email: string,
    lang?: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/reset/send-code",
        { email, lang },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "verification code API is not enabled"),
      ) as AuthResponse;
    }
  },

  verifyResetCode: async (
    email: string,
    code: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/reset/verify-code",
        { email, code },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "verification code API is not enabled"),
      ) as AuthResponse;
    }
  },

  confirmResetPassword: async (
    resetTicket: string,
    newPassword: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>("/api/auth/reset/confirm", {
        reset_ticket: resetTicket,
        new_password: newPassword,
      });
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "password reset is not enabled"),
      ) as AuthResponse;
    }
  },

  wechatStartQR: async (): Promise<{
    success: boolean;
    data?: { login_id: string; qr_url: string };
    error?: string;
  }> => {
    try {
      const response = await api.post("/api/auth/wechat/qr/start");
      return response.data;
    } catch (error) {
      return fail(errorMessageOf(error, "wechat login is not enabled"));
    }
  },

  wechatGetStatus: async (
    loginId: string,
  ): Promise<{
    success: boolean;
    data?: { status: string; bind_ticket?: string; login_code?: string };
    error?: string;
  }> => {
    try {
      const response = await api.get("/api/auth/wechat/qr/status", {
        params: { login_id: loginId },
      });
      return response.data;
    } catch (error) {
      return fail(errorMessageOf(error, "wechat login is not enabled"));
    }
  },

  wechatExchange: async (
    loginId: string,
    loginCode: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/wechat/exchange",
        {
          login_id: loginId,
          login_code: loginCode,
        },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "wechat login is not enabled"),
      ) as AuthResponse;
    }
  },

  wechatBindExisting: async (
    bindTicket: string,
    email: string,
    password: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/wechat/bind-existing",
        {
          bind_ticket: bindTicket,
          email,
          password,
        },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "wechat login is not enabled"),
      ) as AuthResponse;
    }
  },

  wechatBindCreate: async (
    bindTicket: string,
    email: string,
    password: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/wechat/bind-create",
        {
          bind_ticket: bindTicket,
          email,
          password,
        },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "wechat login is not enabled"),
      ) as AuthResponse;
    }
  },

  accountMerge: async (
    targetUserId: string,
    verification: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>("/api/auth/account/merge", {
        target_user_id: targetUserId,
        verification,
      });
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "account merge is not enabled"),
      ) as AuthResponse;
    }
  },

  accountCancel: async (payload: {
    password?: string;
    cancel_ticket?: string;
    reset_ticket?: string;
  }): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/account/cancel",
        payload,
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "account cancellation is not enabled"),
      ) as AuthResponse;
    }
  },

  accountCancelSendCode: async (
    email: string,
    lang?: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/account/cancel/send-code",
        {
          email,
          lang,
        },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "account cancellation is not enabled"),
      ) as AuthResponse;
    }
  },

  accountCancelVerifyCode: async (
    email: string,
    code: string,
  ): Promise<AuthResponse> => {
    try {
      const response = await api.post<AuthResponse>(
        "/api/auth/account/cancel/verify-code",
        {
          email,
          code,
        },
      );
      return normalizeAuthResponse(response.data);
    } catch (error) {
      return fail(
        errorMessageOf(error, "account cancellation is not enabled"),
      ) as AuthResponse;
    }
  },
};

export const kbApi = {
  list: async (): Promise<ApiResult<KnowledgeBase[]>> => {
    try {
      const response = await api.get("/api/v1/notebooks");
      const notebooksRaw = Array.isArray(response.data?.notebooks)
        ? response.data.notebooks
        : [];
      const mapped = notebooksRaw.map((item: Record<string, any>) =>
        mapNotebookFromV5(item),
      );
      writeLocalNotebooks(mapped);
      return ok(mapped);
    } catch (error) {
      const status = statusCodeOf(error);
      if (status !== 401 && status !== 403) {
        const cached = readLocalNotebooks();
        if (cached.length > 0) {
          return ok(cached);
        }
      }
      return fail(errorMessageOf(error, "failed to load notebooks"));
    }
  },

  listFavorites: async (): Promise<ApiResult<FavoriteKnowledgeBase[]>> => {
    return ok(readFavorites());
  },

  get: async (id: string): Promise<ApiResult<KnowledgeBase>> => {
    try {
      const response = await api.get(`/api/v1/notebooks/${id}`);
      const notebook = response.data?.notebook;
      if (!notebook) {
        return fail("notebook not found");
      }
      return ok(mapNotebookFromV5(notebook));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load notebook"));
    }
  },

  create: async (
    title: string,
    description?: string,
    icon?: string,
  ): Promise<ApiResult<KnowledgeBase>> => {
    try {
      const response = await api.post("/api/v1/notebooks", {
        name: title,
        description: description || "",
      });
      const notebook = response.data?.notebook;
      if (!notebook) {
        return fail("create notebook failed");
      }
      const mapped = mapNotebookFromV5({ ...notebook, icon });
      return ok(mapped);
    } catch (error) {
      return fail(errorMessageOf(error, "create notebook failed"));
    }
  },

  update: async (
    id: string,
    data: { title?: string; description?: string; icon?: string },
  ): Promise<ApiResult<KnowledgeBase>> => {
    try {
      let title = data.title;
      let description = data.description;
      if (title === undefined || description === undefined) {
        const current = await kbApi.get(id);
        if (current.success && current.data) {
          title = title ?? current.data.title;
          description = description ?? current.data.description;
        }
      }

      await api.put(`/api/v1/notebooks/${id}`, {
        name: title ?? "",
        description: description ?? "",
      });

      const mapped: KnowledgeBase = {
        id,
        user_id: getRuntimeUserID(),
        title: title ?? "Untitled Notebook",
        description: description ?? "",
        icon: data.icon,
        created_at: nowISO(),
      };
      return ok(mapped);
    } catch (error) {
      return fail(errorMessageOf(error, "update notebook failed"));
    }
  },

  delete: async (id: string): Promise<ApiResult<{ status: string }>> => {
    try {
      await api.delete(`/api/v1/notebooks/${id}`);
      return ok({ status: "deleted" });
    } catch (error) {
      return fail(errorMessageOf(error, "delete notebook failed"));
    }
  },

  createShare: async (
    id: string,
    data: {
      permission: "full" | "partial";
      expire_in_hours?: number;
    },
  ): Promise<ApiResult<{ share_token?: string; share_url: string }>> => {
    try {
      const expiresAt =
        data.expire_in_hours && data.expire_in_hours > 0
          ? new Date(
              Date.now() + data.expire_in_hours * 60 * 60 * 1000,
            ).toISOString()
          : undefined;
      const role = data.permission === "full" ? "editor" : "viewer";
      const response = await api.post(`/api/v1/notebooks/${id}/share`, {
        role,
        expires_at: expiresAt,
      });
      const rawToken = response.data?.share_token;
      const token =
        typeof rawToken === "string"
          ? rawToken
          : typeof rawToken?.Token === "string"
            ? rawToken.Token
            : typeof rawToken?.token === "string"
              ? rawToken.token
              : "";
      if (!token) {
        return fail("share token not returned");
      }
      return ok({
        share_token: token,
        share_url: `/shared/kb/${token}`,
      });
    } catch (error) {
      return fail(errorMessageOf(error, "create share link failed"));
    }
  },

  getShareSettings: async (id: string): Promise<ApiResult<ShareSettings>> => {
    try {
      const response = await api.get(`/api/v1/notebooks/${id}/share/settings`);
      return ok(mapShareSettings(response.data || {}));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load share settings"));
    }
  },

  updateAccessLevel: async (
    id: string,
    accessLevel: "private" | "link" | "public",
  ): Promise<ApiResult<{ access_level: string }>> => {
    try {
      const response = await api.post(`/api/v1/notebooks/${id}/access-level`, {
        access_level: accessLevel,
      });
      return ok({
        access_level: String(response.data?.access_level || accessLevel),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to update access level"));
    }
  },

  listMembers: async (id: string): Promise<ApiResult<ShareMember[]>> => {
    try {
      const response = await api.get(`/api/v1/notebooks/${id}/members`);
      const rows = Array.isArray(response.data?.members) ? response.data.members : [];
      return ok(rows.map((item: Record<string, any>) => mapShareMember(item)));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load notebook members"));
    }
  },

  inviteMember: async (
    id: string,
    email: string,
    role: "viewer" | "editor" | "owner" = "viewer",
  ): Promise<ApiResult<ShareMember>> => {
    try {
      const response = await api.post(`/api/v1/notebooks/${id}/members/invite`, {
        email,
        role,
      });
      return ok(mapShareMember(response.data?.member || {}));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to invite notebook member"));
    }
  },

  removeMember: async (
    id: string,
    memberId: string,
  ): Promise<ApiResult<{ status: string }>> => {
    try {
      const response = await api.delete(`/api/v1/notebooks/${id}/members/${memberId}`);
      return ok({
        status: String(response.data?.status || "removed"),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to remove notebook member"));
    }
  },

  favoriteByToken: async (
    token: string,
    displayName?: string,
  ): Promise<ApiResult<FavoriteKnowledgeBase>> => {
    const favorites = readFavorites();
    const existing = favorites.find((item) => item.share_token === token);
    if (existing) {
      return ok(existing);
    }

    const title = displayName || `Shared ${token.slice(0, 8)}`;
    const favorite: FavoriteKnowledgeBase = {
      id: randomID("fav"),
      user_id: getRuntimeUserID(),
      title,
      description: "",
      created_at: nowISO(),
      is_favorite: true,
      share_token: token,
      share_url: `/shared/kb/${token}`,
      favorite_id: randomID("favorite"),
      favorite_alias: title,
      origin_title: title,
      share_permission: "partial",
      share_expires_at: null,
      favorited_at: nowISO(),
    };
    favorites.push(favorite);
    writeFavorites(favorites);
    return ok(favorite);
  },

  unfavoriteByToken: async (
    token: string,
  ): Promise<ApiResult<{ status: string }>> => {
    const next = readFavorites().filter((item) => item.share_token !== token);
    writeFavorites(next);
    return ok({ status: "removed" });
  },

  listAPIKeys: async (id: string): Promise<ApiResult<NotebookAPIKey[]>> => {
    try {
      const response = await api.get(`/api/v1/notebooks/${id}/api-keys`);
      const rows = Array.isArray(response.data?.api_keys)
        ? response.data.api_keys
        : [];
      return ok(rows.map((item: Record<string, any>) => mapNotebookAPIKey(item)));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load notebook api keys"));
    }
  },

  createAPIKey: async (
    id: string,
    data: {
      name: string;
      permissions?: string[];
      rate_limit_rpm?: number;
      expires_at?: string;
    },
  ): Promise<ApiResult<{ api_key: NotebookAPIKey; plaintext_key: string }>> => {
    try {
      const response = await api.post(`/api/v1/notebooks/${id}/api-keys`, data);
      const apiKey = response.data?.api_key;
      const plaintextKey = response.data?.plaintext_key;
      if (!apiKey || typeof plaintextKey !== "string" || !plaintextKey) {
        return fail("api key payload missing");
      }
      return ok({
        api_key: mapNotebookAPIKey(apiKey),
        plaintext_key: plaintextKey,
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to create notebook api key"));
    }
  },

  revokeAPIKey: async (
    id: string,
    keyId: string,
  ): Promise<ApiResult<{ status: string }>> => {
    try {
      const response = await api.delete(`/api/v1/notebooks/${id}/api-keys/${keyId}`);
      return ok({
        status: String(response.data?.status || "revoked"),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to revoke notebook api key"));
    }
  },
};

export const billingApi = {
  listPlans: async (): Promise<
    ApiResult<{ plans: BillingPlan[]; current_plan_id: string }>
  > => {
    try {
      const response = await api.get("/api/v1/billing/plans");
      const rows = Array.isArray(response.data?.plans) ? response.data.plans : [];
      return ok({
        plans: rows.map((item: Record<string, any>) => mapBillingPlan(item)),
        current_plan_id: String(response.data?.current_plan_id || "free"),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load billing plans"));
    }
  },

  getSubscription: async (): Promise<ApiResult<BillingSubscription>> => {
    try {
      const response = await api.get("/api/v1/billing/subscription");
      const raw =
        response.data?.subscription &&
        typeof response.data.subscription === "object" &&
        !Array.isArray(response.data.subscription)
          ? (response.data.subscription as Record<string, any>)
          : {};
      return ok(mapBillingSubscription(raw));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load billing subscription"));
    }
  },

  getUsage: async (): Promise<ApiResult<BillingUsage>> => {
    try {
      const response = await api.get("/api/v1/billing/usage");
      const raw =
        response.data?.usage &&
        typeof response.data.usage === "object" &&
        !Array.isArray(response.data.usage)
          ? (response.data.usage as Record<string, any>)
          : {};
      return ok(mapBillingUsage(raw));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load billing usage"));
    }
  },

  createCheckoutSession: async (
    planID: string,
  ): Promise<ApiResult<{ url: string; session_id: string }>> => {
    try {
      const response = await api.post("/api/v1/billing/checkout-session", {
        plan_id: planID,
      });
      return ok({
        url: String(response.data?.url || ""),
        session_id: String(response.data?.session_id || ""),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to create checkout session"));
    }
  },

  createPortalSession: async (): Promise<ApiResult<{ url: string }>> => {
    try {
      const response = await api.post("/api/v1/billing/portal-session", {});
      return ok({
        url: String(response.data?.url || ""),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to create billing portal"));
    }
  },
};

export const notificationApi = {
  listNotifications: async (
    limit = 20,
    offset = 0,
  ): Promise<ApiResult<Notification[]>> => {
    try {
      const response = await api.get("/api/v1/notifications", {
        params: { limit, offset },
      });
      const rows = Array.isArray(response.data?.notifications)
        ? response.data.notifications
        : [];
      return ok(rows.map((item: Record<string, any>) => mapNotification(item)));
    } catch (error) {
      return fail(errorMessageOf(error, "failed to load notifications"));
    }
  },

  markNotificationRead: async (
    id: string,
  ): Promise<ApiResult<{ status: string }>> => {
    try {
      const response = await api.post(`/api/v1/notifications/${id}/read`);
      return ok({
        status: String(response.data?.status || "ok"),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "failed to mark notification read"));
    }
  },
};

export const notesApi = {
  list: async (kbId?: string): Promise<ApiResult<Note[]>> => {
    const all = readLocalNotes().map((note) => normalizeNotePayload(note));
    if (!kbId) return ok(all);
    return ok(all.filter((item) => item.kb_id === kbId));
  },

  get: async (id: string): Promise<ApiResult<Note>> => {
    const note = readLocalNotes().find((item) => item.id === id);
    if (!note) return fail("note not found");
    return ok(normalizeNotePayload(note));
  },

  create: async (
    kbId: string,
    content: string,
    title?: string,
    noteType?: string,
  ): Promise<ApiResult<Note>> => {
    const note: Note = {
      id: randomID("note"),
      kb_id: kbId,
      user_id: getRuntimeUserID(),
      title: title || undefined,
      content,
      note_type: normalizeNoteType(noteType) || "draft",
      is_shared: false,
      created_at: nowISO(),
      updated_at: nowISO(),
    };
    const all = readLocalNotes();
    all.unshift(note);
    writeLocalNotes(all);
    return ok(normalizeNotePayload(note));
  },

  update: async (
    id: string,
    data: {
      title?: string;
      content?: string;
      note_type?: "draft" | "committed";
    },
  ): Promise<ApiResult<Note>> => {
    const all = readLocalNotes();
    const idx = all.findIndex((item) => item.id === id);
    if (idx < 0) return fail("note not found");

    const updated: Note = {
      ...all[idx],
      title: data.title !== undefined ? data.title : all[idx].title,
      content: data.content !== undefined ? data.content : all[idx].content,
      note_type:
        data.note_type !== undefined ? data.note_type : all[idx].note_type,
      updated_at: nowISO(),
    };
    all[idx] = updated;
    writeLocalNotes(all);
    return ok(normalizeNotePayload(updated));
  },

  delete: async (id: string): Promise<ApiResult<{ status: string }>> => {
    const next = readLocalNotes().filter((item) => item.id !== id);
    writeLocalNotes(next);
    return ok({ status: "deleted" });
  },
};

export const chatApi = {
  listSessions: async (
    kbId?: string,
    sourceType?: "share" | "favorite",
    sourceToken?: string,
  ): Promise<ApiResult<ChatSession[]>> => {
    if ((sourceType === "share" || sourceType === "favorite") && sourceToken) {
      // Shared-link chat is stateless on server side; keep local-only history per page session.
      return ok([]);
    }
    try {
      const response = await api.get("/api/v1/chat/sessions");
      const rawSessions = Array.isArray(response.data?.sessions)
        ? response.data.sessions
        : [];
      const deleted = new Set(readDeletedSessions());
      let sessions: ChatSession[] = rawSessions
        .map((item: Record<string, any>) => mapSessionFromV5(item))
        .filter((item: ChatSession) => !deleted.has(item.id));
      if (kbId) {
        sessions = sessions.filter((item) => item.kb_id === kbId);
      }
      return ok(sessions);
    } catch (error) {
      return fail(errorMessageOf(error, "list sessions failed"));
    }
  },

  getSession: async (id: string): Promise<ApiResult<ChatSession>> => {
    try {
      const response = await api.get(`/api/v1/chat/sessions/${id}`);
      return ok(mapSessionFromV5(response.data || {}));
    } catch (error) {
      return fail(errorMessageOf(error, "get session failed"));
    }
  },

  createSession: async (
    kbId: string,
    title?: string,
    sourceType?: "share" | "favorite",
    sourceToken?: string,
  ): Promise<ApiResult<ChatSession>> => {
    void sourceType;
    void sourceToken;
    try {
      const response = await api.post("/api/v1/chat/sessions", {
        notebook_id: kbId,
        title,
        agent_type: "rag",
      });
      return ok(mapSessionFromV5(response.data || {}));
    } catch (error) {
      return fail(errorMessageOf(error, "create session failed"));
    }
  },

  deleteSession: async (id: string): Promise<ApiResult<{ status: string }>> => {
    try {
      await api.delete(`/api/v1/chat/sessions/${id}`);
      const deleted = new Set(readDeletedSessions());
      deleted.add(id);
      writeDeletedSessions(Array.from(deleted));
      clearLocalSessionMessages(id);
      return ok({ status: "deleted" });
    } catch (error) {
      return fail(errorMessageOf(error, "delete session failed"));
    }
  },

  getMessages: async (sessionId: string): Promise<ApiResult<ChatMessage[]>> => {
    try {
      const response = await api.get(
        `/api/v1/chat/sessions/${sessionId}/messages`,
      );
      const rawMessages = Array.isArray(response.data?.messages)
        ? response.data.messages
        : Array.isArray(response.data)
          ? response.data
          : [];
      const messages: ChatMessage[] = rawMessages.map(
        (item: Record<string, any>) => ({
          id: Number(item.id || 0),
          session_id: String(item.session_id || sessionId),
          role: item.role === "assistant" ? "assistant" : "user",
          content: String(item.content || ""),
          agent_id:
            typeof item.agent_id === "string" ? item.agent_id : undefined,
          agent_name:
            typeof item.agent_name === "string" ? item.agent_name : undefined,
          agent_icon:
            typeof item.agent_icon === "string" ? item.agent_icon : undefined,
          citations: Array.isArray(item.citations)
            ? (item.citations as Citation[])
            : [],
          created_at:
            typeof item.created_at === "string" ? item.created_at : nowISO(),
        }),
      );
      return ok(messages);
    } catch (error) {
      return fail(errorMessageOf(error, "get messages failed"));
    }
  },

  cacheMessages: (sessionId: string, messages: ChatMessage[]): void => {
    if (!sessionId) return;
    setLocalSessionMessages(sessionId, messages);
  },

  clearCachedMessages: (sessionId: string): void => {
    if (!sessionId) return;
    clearLocalSessionMessages(sessionId);
  },

  sendMessage: async (
    sessionId: string,
    message: string,
  ): Promise<ApiResult<any>> => {
    try {
      const response = await api.post("/api/v1/chat", {
        session_id: sessionId,
        query: message,
        stream: false,
      });
      return ok(response.data);
    } catch (error) {
      return fail(errorMessageOf(error, "send message failed"));
    }
  },

  lookupCitation: async (
    sessionId: string,
    messageId: number,
    citationId: number,
  ): Promise<ApiResult<CitationLookupResponse>> => {
    try {
      const response = await api.post("/api/v1/chat/citations/lookup", {
        session_id: sessionId,
        message_id: messageId,
        citation_id: citationId,
      });
      return ok(response.data || {});
    } catch (error) {
      return fail(errorMessageOf(error, "citation lookup is not available"));
    }
  },

  execute: async (data: {
    session_id: string;
    message: string;
    context?: { sources: any[]; notes: any[] };
    model_override?: string;
  }): Promise<ApiResult<any>> => {
    try {
      const response = await api.post("/api/v1/chat", {
        session_id: data.session_id,
        query: data.message,
        stream: false,
      });
      return ok(response.data);
    } catch (error) {
      return fail(errorMessageOf(error, "execute failed"));
    }
  },
};

export const agentsApi = {
  list: async (): Promise<ApiResult<{ agents: Agent[] }>> => {
    return ok({ agents: BUILTIN_CHAT_AGENTS });
  },
};

export const searchApi = {
  search: async (
    query: string,
    kbId?: string,
  ): Promise<ApiResult<{ results: SearchResult[] }>> => {
    try {
      const response = await api.post("/api/v1/chat", {
        query,
        notebook_id: kbId || "",
        agent_type: "search",
        stream: false,
      });
      const answer = String(response.data?.answer || "");
      const sources = Array.isArray(response.data?.sources)
        ? response.data.sources
        : [];
      const results: SearchResult[] = [
        {
          id: randomID("search"),
          title: query,
          score: 1,
          source_type: "search",
          summary: answer,
          content: answer,
          updated: nowISO(),
          created: nowISO(),
        },
        ...sources.map((item: Record<string, any>, idx: number) => ({
          id: randomID(`search-source-${idx}`),
          title: String(item.title || item.url || `Source ${idx + 1}`),
          score: 0.9,
          source_type: "web",
          summary: String(item.snippet || ""),
          content: String(item.content || item.snippet || ""),
          updated: nowISO(),
          created: nowISO(),
        })),
      ];
      return ok({ results });
    } catch (error) {
      return fail(errorMessageOf(error, "search is unavailable"));
    }
  },
};

export const sourcesApi = {
  list: async (notebookId?: string): Promise<ApiResult<any[]>> => {
    try {
      const response = await api.get("/api/v1/sources", {
        params: notebookId ? { notebook_id: notebookId } : undefined,
      });
      const rows = Array.isArray(response.data?.sources)
        ? response.data.sources
        : [];
      const mapped = rows.map((row: Record<string, any>) => ({
        id: String(row.id || ""),
        title: String(row.title || row.file_name || row.filename || ""),
        file_name: String(row.file_name || row.title || ""),
        status: String(row.status || ""),
        kb_id:
          typeof row.notebook_id === "string" ? row.notebook_id : undefined,
        notebook_id:
          typeof row.notebook_id === "string" ? row.notebook_id : undefined,
        notebook_name:
          typeof row.notebook_name === "string" ? row.notebook_name : undefined,
      }));
      return ok(mapped);
    } catch (error) {
      return fail(errorMessageOf(error, "list sources failed"));
    }
  },
};

export const notebookApi = {
  addSources: async (
    notebookId: string,
    sourceIds: string[],
  ): Promise<ApiResult<{ status: string; added: number }>> => {
    try {
      const response = await api.post(`/api/v1/notebooks/${notebookId}/sources`, {
        source_ids: sourceIds,
      });
      return ok({
        status: String(response.data?.status || "queued"),
        added: Number(response.data?.added || 0),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "add existing sources failed"));
    }
  },
};

async function uploadWithPresignedURL(
  uploadURL: string,
  file: File,
  onUploadProgress?: (progress: number) => void,
): Promise<void> {
  await axios.put(uploadURL, file, {
    headers: {
      "Content-Type": file.type || "application/octet-stream",
    },
    onUploadProgress: (event: AxiosProgressEvent) => {
      if (!onUploadProgress) return;
      const total = event.total ?? file.size;
      if (!total) {
        onUploadProgress(0);
        return;
      }
      const progress = Math.round((event.loaded / total) * 100);
      onUploadProgress(Math.min(100, Math.max(0, progress)));
    },
  });
}

export const documentsApi = {
  list: async (kbId: string): Promise<ApiResult<Document[]>> => {
    try {
      const response = await api.get("/api/v1/documents", {
        params: { notebook_id: kbId },
      });
      const documentsRaw = Array.isArray(response.data?.documents)
        ? response.data.documents
        : [];
      const mapped = documentsRaw
        .map((item: Record<string, any>) => mapDocumentFromV5(item))
        .filter((item: Document) => item.kb_id === kbId);
      if (mapped.length > 0) {
        const localDocs = readLocalDocuments().filter(
          (item) => item.kb_id !== kbId,
        );
        writeLocalDocuments([...mapped, ...localDocs]);
      }
      return ok(mapped);
    } catch (error) {
      return fail(errorMessageOf(error, "list documents failed"));
    }
  },

  get: async (id: string): Promise<ApiResult<Document>> => {
    try {
      const response = await api.get("/api/v1/documents", {
        params: { document_id: id },
      });
      const documentsRaw = Array.isArray(response.data?.documents)
        ? response.data.documents
        : [];
      const matched = documentsRaw.find(
        (item: Record<string, any>) => String(item.id || "") === id,
      );
      if (matched) {
        return ok(mapDocumentFromV5(matched));
      }
    } catch {
      // Fall through to local cache.
    }

    const doc = readLocalDocuments().find((item) => item.id === id);
    if (!doc) return fail("document not found");
    return ok(doc);
  },

  getContent: async (
    id: string,
  ): Promise<ApiResult<{ content: string; summary?: string }>> => {
    try {
      const response = await api.get(`/api/v1/documents/${id}/content`);
      return ok({
        content: String(response.data?.content || ""),
        summary:
          typeof response.data?.summary === "string"
            ? response.data.summary
            : undefined,
      });
    } catch (error) {
      const status = statusCodeOf(error);
      return fail(
        errorMessageOf(error, "document content preview failed"),
        status === 429
          ? "RATE_LIMITED"
          : status === 401 || status === 403
            ? "UNAUTHORIZED"
            : undefined,
      );
    }
  },

  getParsedPreview: async (
    id: string,
    cursor = 0,
    limit = 80,
  ): Promise<
    ApiResult<{
      items: any[];
      has_more: boolean;
      next_cursor: number;
      summary?: string;
    }>
  > => {
    try {
      const response = await api.get(`/api/v1/documents/${id}/parsed-preview`, {
        params: { cursor, limit },
      });
      return ok({
        items: Array.isArray(response.data?.items) ? response.data.items : [],
        has_more: Boolean(response.data?.has_more),
        next_cursor: Number(response.data?.next_cursor || 0),
        summary:
          typeof response.data?.summary === "string"
            ? response.data.summary
            : undefined,
      });
    } catch (error) {
      const status = statusCodeOf(error);
      return fail(
        errorMessageOf(error, "document parsed preview failed"),
        status === 429
          ? "RATE_LIMITED"
          : status === 401 || status === 403
            ? "UNAUTHORIZED"
            : undefined,
      );
    }
  },

  previewText: async (id: string): Promise<string> => {
    const response = await documentsApi.getContent(id);
    if (!response.success || !response.data) {
      throw new Error(response.error || "document preview API is not available");
    }
    return response.data.content || response.data.summary || "";
  },

  previewBlob: async (id: string): Promise<Blob> => {
    void id;
    throw new Error("document preview API is not available");
  },

  upload: async (
    kbId: string,
    file: File,
    onUploadProgress?: (progress: number) => void,
  ): Promise<
    ApiResult<{ id: string; document_id: string; status: string }>
  > => {
    try {
      const createRes = await api.post(`/api/v1/notebooks/${kbId}/documents`, {
        filename: file.name,
        file_size: file.size,
        mime_type: file.type || "application/octet-stream",
      });

      const documentID = String(createRes.data?.document_id || "");
      const uploadURL = String(createRes.data?.upload_url || "");
      if (!documentID || !uploadURL) {
        return fail("upload url not returned");
      }

      const isHTTPUploadURL =
        uploadURL.startsWith("http://") || uploadURL.startsWith("https://");
      if (isHTTPUploadURL) {
        await uploadWithPresignedURL(uploadURL, file, onUploadProgress);
      } else {
        return fail("存储后端返回了非 HTTP 上传地址，无法完成上传");
      }

      await api.post(`/api/v1/documents/${documentID}/complete-upload`);

      onUploadProgress?.(100);

      return ok({
        id: documentID,
        document_id: documentID,
        status: "queued",
      });
    } catch (error) {
      return fail(errorMessageOf(error, "upload failed"));
    }
  },

  update: async (
    id: string,
    data: { status?: string; file_name?: string; kb_id?: string },
  ): Promise<ApiResult<{ status: string }>> => {
    try {
      await api.put(`/api/v1/documents/${id}`, {
        status: data.status,
        filename: data.file_name,
        notebook_id: data.kb_id,
      });
      return ok({ status: "updated" });
    } catch (error) {
      return fail(errorMessageOf(error, "update document failed"));
    }
  },

  delete: async (
    id: string,
    kbId?: string,
  ): Promise<ApiResult<{ status: string }>> => {
    try {
      if (kbId) {
        await api.delete(`/api/v1/notebooks/${kbId}/documents/${id}`);
      } else {
        await api.delete(`/api/v1/documents/${id}`);
      }
      return ok({ status: "deleted" });
    } catch (error) {
      return fail(errorMessageOf(error, "delete document failed"));
    }
  },

  getStatus: async (
    id: string,
  ): Promise<ApiResult<{ status: Document["status"] }>> => {
    try {
      const response = await api.get(`/api/v1/documents/${id}/status`);
      return ok({
        status: normalizeDocumentStatus(response.data?.status),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "document status lookup failed"));
    }
  },

  listStatusEvents: async (
    kbId: string,
    afterSeq = 0,
    limit = 200,
  ): Promise<ApiResult<{ events: any[] }>> => {
    void kbId;
    void afterSeq;
    void limit;
    return fail("document status backlog API is not available");
  },

  streamStatusEvents: async (
    _kbId: string,
    _afterSeq: number,
    options?: {
      signal?: AbortSignal;
      onEvent?: (event: string, payload: unknown) => void;
    },
  ): Promise<void> => {
    void _kbId;
    void _afterSeq;
    void options;
    throw new Error("document status event stream is not available");
  },

  addUrl: async (
    kbId: string,
    url: string,
  ): Promise<ApiResult<{ id?: string; document_id?: string; status: string }>> => {
    try {
      const response = await api.post(`/api/v1/notebooks/${kbId}/sources/url`, {
        url,
      });
      return ok({
        id:
          typeof response.data?.id === "string"
            ? response.data.id
            : undefined,
        document_id:
          typeof response.data?.document_id === "string"
            ? response.data.document_id
            : undefined,
        status: String(response.data?.status || "queued"),
      });
    } catch (error) {
      return fail(errorMessageOf(error, "URL source ingestion failed"));
    }
  },
};

/**
 * Poll document status until it reaches a terminal state (completed/failed).
 * Returns the final status and chunk_count when available.
 */
export async function pollDocumentStatus(
  documentId: string,
  intervalMs = 2000,
  maxAttempts = 60,
): Promise<{
  status: "completed" | "failed" | "pending" | "processing" | "queued";
  chunkCount?: number;
}> {
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const result = await documentsApi.getStatus(documentId);
    if (!result.success) {
      throw new Error(result.error);
    }
    const { status } = result.data!;
    if (status === "completed" || status === "failed") {
      // TODO: Fetch chunk_count from document content endpoint if needed
      return { status, chunkCount: undefined };
    }
    // Wait before next poll
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
  throw new Error(`Polling timed out after ${maxAttempts} attempts`);
}
