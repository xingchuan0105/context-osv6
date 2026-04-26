type MiddlewareRedirect = {
  type: "redirect";
  destination: string;
};

type MiddlewareNext = {
  type: "next";
};

export type MiddlewareAction = MiddlewareRedirect | MiddlewareNext;

function resolveCompatibilityRedirect(pathname: string): string | null {
  if (pathname === "/dashboard/search") {
    return "/dashboard";
  }

  if (pathname.startsWith("/notebooks/")) {
    const match = pathname.match(
      /^\/notebooks\/([^/]+)\/share(?:\/(analytics|access-logs))?$/,
    );

    if (match) {
      const [, workspaceId, suffix] = match;
      return suffix
        ? `/dashboard/${workspaceId}/share/${suffix}`
        : `/dashboard/${workspaceId}/share`;
    }
  }

  if (pathname.startsWith("/admin/orgs/")) {
    const match = pathname.match(/^\/admin\/orgs\/([^/]+)$/);

    if (match) {
      return `/admin/organizations/${match[1]}`;
    }
  }

  return null;
}

export function resolveMiddlewareAction(
  pathname: string,
  _hasAuthSessionHint: boolean,
): MiddlewareAction {
  const compatibilityRedirect = resolveCompatibilityRedirect(pathname);

  if (compatibilityRedirect) {
    return { type: "redirect", destination: compatibilityRedirect };
  }

  return { type: "next" };
}
