"use client";

import { usePathname, useSearchParams } from "next/navigation";

import { resolveWorkspaceIdFromRoute } from "../lib/runtime/desktop-app-href";

/** Real workspace id when static export served `_placeholder` + `?ws=`. */
export function useAppWorkspaceId(propId: string): string {
  const pathname = usePathname() ?? "";
  const searchParams = useSearchParams();
  return resolveWorkspaceIdFromRoute(propId, pathname, searchParams.get("ws"));
}
