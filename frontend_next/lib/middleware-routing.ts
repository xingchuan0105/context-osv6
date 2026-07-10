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

  // Legacy/public share URLs under /workspaces/:id/share → app dashboard share UI.
  if (pathname.startsWith("/workspaces/")) {
    const match = pathname.match(
      /^\/workspaces\/([^/]+)\/share(?:\/(analytics|access-logs))?$/,
    );
    if (match) {
      const [, workspaceId, suffix] = match;
      return suffix
        ? `/dashboard/${workspaceId}/share/${suffix}`
        : `/dashboard/${workspaceId}/share`;
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
