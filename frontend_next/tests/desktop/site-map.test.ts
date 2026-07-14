import { describe, expect, it } from "vitest";

import {
  APP_PATHS,
  EXTERNAL,
  SITE_LINKS,
  appAbsoluteUrl,
  familyNavLinks,
  getAppPublicOrigin,
} from "@/lib/site-map";

describe("site-map", () => {
  it("exposes desktop on family nav", () => {
    const ids = familyNavLinks("zh").map((l) => l.id);
    expect(ids).toContain("desktop");
    expect(ids).toContain("app");
  });

  it("desktop discovery covers footer and hub slots", () => {
    const desktop = SITE_LINKS.find((l) => l.id === "desktop");
    expect(desktop?.discovery).toEqual(
      expect.arrayContaining(["family_nav", "hub_cta", "app_footer", "help", "pricing"]),
    );
  });

  it("builds absolute app urls", () => {
    expect(getAppPublicOrigin()).toMatch(/^https?:\/\//);
    expect(appAbsoluteUrl(APP_PATHS.desktop)).toContain("/desktop");
    expect(EXTERNAL.appDesktop()).toContain("desktop");
  });
});
