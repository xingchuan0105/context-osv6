import { ApiError, fetchResponse, request, type ApiEnvelope } from "../http/request";
import {
  parseWorkspaceChatEventStream,
  type WorkspaceChatStreamEvent,
} from "../workspace/stream";

/** Stable backend code when enabling share exceeds plan slots (ADR-0010). */
export const SHARE_WORKSPACE_QUOTA_EXCEEDED = "share_workspace_quota_exceeded";

export type ShareSettings = {
  share_token: string;
  access_level: string;
  expires_at: string | null;
  allow_download: boolean;
  /** Daily anon visitor question cap; 0 = unlimited. */
  anon_question_limit: number;
  /** Daily registered visitor cap; null = unlimited. */
  member_question_limit: number | null;
};

/** Owner share-slot usage for the current plan. */
export type ShareQuotaSummary = {
  used: number;
  max: number;
  plan_id: string;
};

/**
 * Visitor access preference persisted via `access_level`:
 * - `public` → anonymous visitors allowed
 * - `link` → require register (signed-in) for interaction
 */
export type VisitorAccessMode = "anonymous" | "require_register";

export function visitorModeFromAccessLevel(accessLevel: string | null | undefined): VisitorAccessMode {
  return accessLevel === "public" ? "anonymous" : "require_register";
}

export function accessLevelFromVisitorMode(mode: VisitorAccessMode): "link" | "public" {
  return mode === "anonymous" ? "public" : "link";
}

export type ShareAnalyticsResponse = {
  total_views: number;
  total_unique_visitors: number;
  views_by_day: Record<string, number>;
};

export type AccessLogEntry = {
  id: string;
  visitor_id: string;
  accessed_at: string;
  action: string;
};

export type AccessLogsResponse = {
  logs: AccessLogEntry[];
};

export type MemberRow = {
  member_id: string;
  user_id: string;
  email: string;
  role: string;
  status: string;
  invited_at: string;
};

export type MembersResponse = {
  members: MemberRow[];
};

export type SharedKnowledgeBase = {
  id: string;
  title: string;
  description?: string | null;
};

export type SharedShareInfo = {
  permission: string;
  expires_at: string | null;
  allow_download: boolean;
  scope: string;
};

export type SharedSource = {
  id: string;
  file_name: string;
  status: string;
};

export type ShareOwnerCard = {
  display_name: string;
  bio?: string | null;
  contact_url?: string | null;
  avatar_url?: string | null;
  banner_url?: string | null;
};

export type SharedWorkspacePayload = {
  knowledge_base: SharedKnowledgeBase;
  share: SharedShareInfo;
  sources: SharedSource[];
  owner?: ShareOwnerCard | null;
};

type SharedWorkspaceEnvelope = {
  success?: boolean;
  data?: SharedWorkspacePayload | null;
  error?: string | null;
};

type RawShareTokenInfo = {
  token: string;
  access_level: string;
  expires_at?: string | null;
  revoked_at?: string | null;
};

type RawShareSettings = {
  access_level: string;
  allow_download: boolean;
  anon_question_limit?: number;
  member_question_limit?: number | null;
  share_tokens?: RawShareTokenInfo[];
};

type RawShareAnalytics = {
  token: string;
  access_level: string;
  total_views: number;
  last_accessed_at?: number | null;
  created_at?: string | null;
};

type RawShareAccessLog = {
  id: string;
  workspace_id: string;
  share_token: string;
  action: string;
  accessed_at: number;
};

function mapShareSettings(raw: RawShareSettings): ShareSettings {
  const activeShareToken =
    raw.share_tokens?.find((token) => !token.revoked_at) ?? raw.share_tokens?.[0] ?? null;

  return {
    share_token: activeShareToken?.token ?? "",
    access_level: raw.access_level,
    expires_at: activeShareToken?.expires_at ?? null,
    allow_download: raw.allow_download,
    anon_question_limit: raw.anon_question_limit ?? 10,
    member_question_limit:
      raw.member_question_limit === undefined ? null : raw.member_question_limit,
  };
}

export function buildShareUrl(shareToken: string) {
  if (!shareToken.trim()) {
    return "";
  }

  const relativePath = `/shared/kb/${shareToken}`;

  if (typeof window === "undefined") {
    return relativePath;
  }

  return new URL(relativePath, window.location.origin).toString();
}

export function isShareEnabled(settings: ShareSettings | null) {
  if (!settings) {
    return false;
  }

  return settings.share_token.trim().length > 0 && settings.access_level !== "private";
}

export async function getShareSettings(token: string, workspaceId: string) {
  const raw = await request<RawShareSettings>(
    `/api/v1/workspaces/${workspaceId}/share/settings`,
    { method: "GET" },
    token,
  );

  return mapShareSettings(raw);
}

export async function getShareQuota(token: string) {
  return request<ShareQuotaSummary>("/api/v1/share/quota", { method: "GET" }, token);
}

export async function updateShareSettings(
  token: string,
  workspaceId: string,
  settings: Partial<
    Pick<
      ShareSettings,
      "access_level" | "allow_download" | "anon_question_limit" | "member_question_limit"
    >
  > & {
    member_question_limit_set?: boolean;
  },
) {
  const raw = await request<RawShareSettings>(
    `/api/v1/workspaces/${workspaceId}/share/settings`,
    {
      method: "PUT",
      body: JSON.stringify(settings),
    },
    token,
  );

  return mapShareSettings(raw);
}

/** Map known share API error codes to i18n message keys when possible. */
export function shareActionErrorMessage(
  error: unknown,
  locale: "zh-CN" | "en",
  fallbackKey: string,
  formatMessage: (locale: "zh-CN" | "en", key: string) => string,
): string {
  if (error instanceof ApiError && error.code === SHARE_WORKSPACE_QUOTA_EXCEEDED) {
    return formatMessage(locale, "shareCenter.quotaExceeded");
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  return formatMessage(locale, fallbackKey);
}

export async function createShareLink(
  token: string,
  workspaceId: string,
  requestBody: {
    role: string;
    expires_at?: string | null;
  },
) {
  return request<{ share_token: string }>(
    `/api/v1/workspaces/${workspaceId}/share`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function revokeShareLink(token: string, workspaceId: string, shareToken: string) {
  await request<Record<string, never>>(
    `/api/v1/workspaces/${workspaceId}/share/${shareToken}`,
    { method: "DELETE" },
    token,
  );
}

export async function getShareAnalytics(token: string, workspaceId: string) {
  const envelope = await request<ApiEnvelope<RawShareAnalytics[]>>(
    `/api/v1/workspaces/${workspaceId}/share/analytics`,
    { method: "GET" },
    token,
  );

  const entries = envelope.data ?? [];
  const viewsByDay: Record<string, number> = {};
  let totalViews = 0;

  for (const entry of entries) {
    totalViews += entry.total_views;
    const day = (entry.created_at ?? "unknown").slice(0, 10) || "unknown";
    viewsByDay[day] = (viewsByDay[day] ?? 0) + entry.total_views;
  }

  return {
    total_views: totalViews,
    total_unique_visitors: entries.length,
    views_by_day: viewsByDay,
  } satisfies ShareAnalyticsResponse;
}

export async function getShareAccessLogs(token: string, workspaceId: string) {
  const envelope = await request<ApiEnvelope<RawShareAccessLog[]>>(
    `/api/v1/workspaces/${workspaceId}/share/access-logs`,
    { method: "GET" },
    token,
  );

  return {
    logs: (envelope.data ?? []).map((entry) => ({
      id: entry.id,
      visitor_id: entry.share_token,
      accessed_at: String(entry.accessed_at),
      action: entry.action,
    })),
  } satisfies AccessLogsResponse;
}

export async function listMembers(token: string, workspaceId: string) {
  return request<MembersResponse>(`/api/v1/workspaces/${workspaceId}/members`, { method: "GET" }, token);
}

export async function inviteMember(
  token: string,
  workspaceId: string,
  email: string,
  role: "viewer" | "editor",
) {
  await request<Record<string, never>>(
    `/api/v1/workspaces/${workspaceId}/members/invite`,
    {
      method: "POST",
      body: JSON.stringify({ email, role }),
    },
    token,
  );
}

export async function removeMember(token: string, workspaceId: string, memberId: string) {
  await request<Record<string, never>>(
    `/api/v1/workspaces/${workspaceId}/members/${memberId}`,
    { method: "DELETE" },
    token,
  );
}

export async function acceptInvite(token: string, workspaceId: string, memberId: string) {
  await request<Record<string, never>>(
    `/api/v1/workspaces/${workspaceId}/members/${memberId}/accept`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}

export async function declineInvite(token: string, workspaceId: string, memberId: string) {
  await request<Record<string, never>>(
    `/api/v1/workspaces/${workspaceId}/members/${memberId}/decline`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}

export async function getSharedWorkspace(shareToken: string) {
  const envelope = await request<SharedWorkspaceEnvelope>(`/api/shared/kb/${shareToken}`, {
    method: "GET",
  });

  if (!envelope.success || !envelope.data) {
    throw new Error(envelope.error ?? "共享链接无效或已过期。");
  }

  return envelope.data;
}

export async function streamSharedChat(
  shareToken: string,
  notebookId: string,
  query: string,
  onEvent: (event: WorkspaceChatStreamEvent) => void | Promise<void>,
  authToken?: string | null,
  turnstileToken?: string | null,
) {
  const headers: Record<string, string> = {
    Accept: "text/event-stream",
    "Content-Type": "application/json",
  };
  if (turnstileToken?.trim()) {
    headers["cf-turnstile-response"] = turnstileToken.trim();
  }
  const response = await fetchResponse(
    "/api/v1/chat",
    {
      method: "POST",
      headers,
      body: JSON.stringify({
        query,
        workspace_id: notebookId,
        session_id: null,
        agent_type: "rag",
        source_type: "share",
        source_token: shareToken,
        doc_scope: [],
        messages: [],
        stream: true,
        turnstile_token: turnstileToken?.trim() || undefined,
      }),
    },
    { token: authToken ?? undefined },
  );

  await parseWorkspaceChatEventStream(response.body, onEvent);
}
