/**
 * Workspace deep-link helpers.
 * Canonical:
 *   `/dashboard/:workspaceId`
 *   `/dashboard/:workspaceId?session=:sessionId`
 *   `/dashboard/:workspaceId?source=:sourceId`
 *   `/dashboard/:workspaceId?session=:sessionId&source=:sourceId`
 */

export type WorkspaceDeepLink = {
  sessionId?: string | null;
  sourceId?: string | null;
};

function normalizeId(value: string | null | undefined): string | null {
  const id = value?.trim();
  return id ? id : null;
}

/** Build a workspace URL with optional session / source query params. */
export function workspaceDeepLinkHref(
  workspaceId: string,
  opts?: WorkspaceDeepLink,
): string {
  const base = `/dashboard/${workspaceId}`;
  const parts: string[] = [];
  const sessionId = normalizeId(opts?.sessionId);
  const sourceId = normalizeId(opts?.sourceId);
  // encodeURIComponent (not URLSearchParams) so spaces stay %20, not +.
  if (sessionId) {
    parts.push(`session=${encodeURIComponent(sessionId)}`);
  }
  if (sourceId) {
    parts.push(`source=${encodeURIComponent(sourceId)}`);
  }
  return parts.length > 0 ? `${base}?${parts.join("&")}` : base;
}

/** Session-only deep-link (drops any source query). */
export function workspaceSessionHref(
  workspaceId: string,
  sessionId: string | null | undefined,
): string {
  return workspaceDeepLinkHref(workspaceId, { sessionId });
}

/** Source viewer deep-link (optional concurrent session). */
export function workspaceSourceHref(
  workspaceId: string,
  sourceId: string,
  sessionId?: string | null,
): string {
  return workspaceDeepLinkHref(workspaceId, { sourceId, sessionId });
}

/** True when the URL's `session` query already matches the active selection. */
export function sessionQueryMatches(
  sessionParam: string | null | undefined,
  sessionId: string | null | undefined,
): boolean {
  return normalizeId(sessionParam) === normalizeId(sessionId);
}
