/**
 * Workspace session deep-link helpers.
 * Canonical: `/dashboard/:workspaceId?session=:sessionId`
 */

export function workspaceSessionHref(
  workspaceId: string,
  sessionId: string | null | undefined,
): string {
  const base = `/dashboard/${workspaceId}`;
  const id = sessionId?.trim();
  if (!id) {
    return base;
  }
  return `${base}?session=${encodeURIComponent(id)}`;
}

/** True when the URL's `session` query already matches the active selection. */
export function sessionQueryMatches(
  sessionParam: string | null | undefined,
  sessionId: string | null | undefined,
): boolean {
  const fromUrl = sessionParam?.trim() || null;
  const active = sessionId?.trim() || null;
  return fromUrl === active;
}
