import { describe, expect, it } from "vitest";

import {
  desktopAppHref,
  mapAppPathForStaticExport,
  resolveChatSessionIdFromRoute,
  resolveWorkspaceIdFromRoute,
  shouldOpenInSystemBrowser,
} from "@/lib/runtime/desktop-app-href";

const PAGE = "http://tauri.localhost/dashboard";
const WORKSPACE = "67501731-e2c2-4d3f-ae71-7bdb711368e8";

describe("shouldOpenInSystemBrowser", () => {
  it("keeps same-origin tauri.localhost dashboard links in the webview", () => {
    expect(
      shouldOpenInSystemBrowser(`http://tauri.localhost/dashboard/${WORKSPACE}`, PAGE),
    ).toBe(false);
    expect(shouldOpenInSystemBrowser("/dashboard/ws-4", PAGE)).toBe(false);
    expect(shouldOpenInSystemBrowser("https://tauri.localhost/settings", PAGE)).toBe(false);
  });

  it("opens real external http(s) and mailto in the OS browser", () => {
    expect(shouldOpenInSystemBrowser("https://context-os.com/pricing", PAGE)).toBe(true);
    expect(shouldOpenInSystemBrowser("mailto:hi@example.com", PAGE)).toBe(true);
  });
});

describe("mapAppPathForStaticExport", () => {
  it("rewrites a live workspace id to the exported placeholder file", () => {
    expect(mapAppPathForStaticExport(`/dashboard/${WORKSPACE}`)).toBe(
      `/dashboard/_placeholder?ws=${WORKSPACE}`,
    );
    const withSession = new URL(
      mapAppPathForStaticExport(`/dashboard/${WORKSPACE}?session=sess-1`),
      "http://tauri.localhost",
    );
    expect(withSession.pathname).toBe("/dashboard/_placeholder");
    expect(withSession.searchParams.get("ws")).toBe(WORKSPACE);
    expect(withSession.searchParams.get("session")).toBe("sess-1");
    expect(mapAppPathForStaticExport(`/dashboard/${WORKSPACE}/share`)).toBe(
      `/dashboard/_placeholder/share?ws=${WORKSPACE}`,
    );
    expect(mapAppPathForStaticExport(`/dashboard/${WORKSPACE}.txt`)).toBe(
      `/dashboard/_placeholder.txt?ws=${WORKSPACE}`,
    );
  });

  it("leaves list and reserved dashboard paths alone", () => {
    expect(mapAppPathForStaticExport("/dashboard")).toBe("/dashboard");
    expect(mapAppPathForStaticExport("/dashboard/analytics")).toBe("/dashboard/analytics");
    expect(mapAppPathForStaticExport("/dashboard/_placeholder?ws=abc")).toBe(
      "/dashboard/_placeholder?ws=abc",
    );
  });

  it("rewrites a personal chat id to the exported placeholder file", () => {
    expect(mapAppPathForStaticExport("/chat/personal-1")).toBe(
      "/chat/_placeholder?session=personal-1",
    );
    expect(mapAppPathForStaticExport("/chat/personal-1?source=web#answer")).toBe(
      "/chat/_placeholder?source=web&session=personal-1#answer",
    );
    expect(mapAppPathForStaticExport("/chat")).toBe("/chat");
    expect(mapAppPathForStaticExport("/chat/_placeholder?session=personal-1")).toBe(
      "/chat/_placeholder?session=personal-1",
    );
  });
});

describe("desktopAppHref", () => {
  it("only remaps inside the desktop shell", () => {
    const href = `/dashboard/${WORKSPACE}`;
    expect(desktopAppHref(href, false)).toBe(href);
    expect(desktopAppHref(href, true)).toBe(`/dashboard/_placeholder?ws=${WORKSPACE}`);
  });
});

describe("resolveWorkspaceIdFromRoute", () => {
  it("prefers ?ws= then a real path segment then the server prop", () => {
    expect(resolveWorkspaceIdFromRoute("_placeholder", "/dashboard/_placeholder", WORKSPACE)).toBe(
      WORKSPACE,
    );
    expect(resolveWorkspaceIdFromRoute("_placeholder", `/dashboard/${WORKSPACE}`, null)).toBe(
      WORKSPACE,
    );
    expect(resolveWorkspaceIdFromRoute("prop-id", "/dashboard/_placeholder", null)).toBe("prop-id");
  });
});

describe("resolveChatSessionIdFromRoute", () => {
  it("prefers the desktop query and otherwise reads a real chat segment", () => {
    expect(
      resolveChatSessionIdFromRoute("/chat/_placeholder", "personal-query"),
    ).toBe("personal-query");
    expect(resolveChatSessionIdFromRoute("/chat/personal-path", null)).toBe(
      "personal-path",
    );
    expect(resolveChatSessionIdFromRoute("/chat/_placeholder", null)).toBeNull();
    expect(resolveChatSessionIdFromRoute("/chat", null)).toBeNull();
  });
});
