/**
 * Desktop static-export href rules.
 *
 * Packaged Tauri serves `frontend_next/out`. Dynamic `/dashboard/:id` pages
 * only exist as `/dashboard/_placeholder.html`. Same-origin `http://tauri.localhost/...`
 * must stay in the webview — it is not a URL the OS browser can open.
 */

import { isTauri } from "./tauri-ipc";

const TAURI_HOSTS = new Set(["tauri.localhost", "ipc.localhost"]);

function stripKnownExt(segment: string): { base: string; suffix: string } {
  if (segment.endsWith(".html")) {
    return { base: segment.slice(0, -5), suffix: ".html" };
  }
  if (segment.endsWith(".txt")) {
    return { base: segment.slice(0, -4), suffix: ".txt" };
  }
  return { base: segment, suffix: "" };
}

function isReservedDashboardSegment(base: string): boolean {
  return base === "analytics" || base === "_placeholder" || base.startsWith("__next");
}

/**
 * True when this href should leave the webview (OS browser / mail).
 * Relative paths and same-origin `tauri.localhost` links stay in-app.
 */
export function shouldOpenInSystemBrowser(href: string, pageHref: string): boolean {
  const raw = href.trim();
  if (!raw || raw.startsWith("#") || raw.toLowerCase().startsWith("javascript:")) {
    return false;
  }
  if (raw.toLowerCase().startsWith("mailto:")) {
    return true;
  }

  let url: URL;
  let page: URL;
  try {
    url = new URL(raw, pageHref);
    page = new URL(pageHref);
  } catch {
    return false;
  }

  if (url.protocol === "mailto:") {
    return true;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return false;
  }
  if (TAURI_HOSTS.has(url.hostname)) {
    return false;
  }
  return url.origin !== page.origin;
}

/**
 * Rewrite a dynamic app path to the static-export file that actually exists.
 * `/dashboard/{id}` → `/dashboard/_placeholder?ws={id}`
 */
export function mapAppPathForStaticExport(href: string, pageHref = "http://tauri.localhost/"): string {
  let url: URL;
  try {
    url = new URL(href, pageHref);
  } catch {
    return href;
  }

  const parts = url.pathname.split("/");
  if (parts[1] === "dashboard" && parts[2]) {
    const { base, suffix } = stripKnownExt(parts[2]);
    if (!isReservedDashboardSegment(base)) {
      url.searchParams.set("ws", base);
      parts[2] = `_placeholder${suffix}`;
      url.pathname = parts.join("/") || "/";
    }
  }

  return `${url.pathname}${url.search}${url.hash}`;
}

/** Apply static-export mapping only inside the Tauri shell. */
export function desktopAppHref(href: string, desktop = isTauri()): string {
  if (!desktop) {
    return href;
  }
  return mapAppPathForStaticExport(href);
}

export function resolveWorkspaceIdFromRoute(
  propId: string,
  pathname: string,
  wsQuery: string | null | undefined,
): string {
  const fromQuery = wsQuery?.trim();
  if (fromQuery) {
    return fromQuery;
  }
  const match = pathname.match(/^\/dashboard\/([^/]+)/);
  const raw = match?.[1] ?? "";
  const { base } = stripKnownExt(raw);
  if (base && !isReservedDashboardSegment(base)) {
    return base;
  }
  return propId;
}
