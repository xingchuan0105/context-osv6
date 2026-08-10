import { readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  APP_NAV_ENTRIES,
  appNavEntry,
  paletteNavEntries,
} from "@/lib/navigation/nav-config";

/**
 * Guards for the canonical nav catalog (PRODUCT_IA §4 single source of truth):
 * entries stay linkable (every href resolves to a real route) and palette
 * membership stays complete. Orphan/dead destinations fail here, not in prod.
 */

function appRoutePaths(): Set<string> {
  const appDir = join(process.cwd(), "app");
  const routes = new Set<string>();
  for (const entry of readdirSync(appDir, { recursive: true })) {
    const file = String(entry);
    if (!file.endsWith("page.tsx")) {
      continue;
    }
    const segments = file
      .slice(0, -"page.tsx".length)
      .split("/")
      .filter((segment) => segment && !segment.startsWith("("));
    // Dynamic segments ([id]) never appear in the catalog; keep as-is for clarity.
    routes.add(`/${segments.join("/")}`.replace(/\/$/, "") || "/");
  }
  return routes;
}

describe("app nav config", () => {
  it("has unique ids", () => {
    const ids = APP_NAV_ENTRIES.map((entry) => entry.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("resolves every href to an existing app route", () => {
    const routes = appRoutePaths();
    for (const entry of APP_NAV_ENTRIES) {
      const path = entry.href.split(/[?#]/)[0]!;
      expect(routes.has(path), `${entry.id} → ${path}`).toBe(true);
    }
  });

  it("keeps palette entries complete (group + keywords)", () => {
    for (const entry of paletteNavEntries()) {
      expect(entry.paletteGroup, entry.id).toBeTruthy();
      expect(entry.paletteKeywords?.trim(), entry.id).toBeTruthy();
    }
  });

  it("locks PRODUCT_IA §4 canonical destinations", () => {
    expect(appNavEntry("pricing").href).toBe("/pricing");
    expect(appNavEntry("topup").href).toBe("/pricing#topup");
    expect(appNavEntry("providers").href).toBe("/settings?tab=providers");
    expect(appNavEntry("desktop").href).toBe("/desktop");
    expect(appNavEntry("share-traffic").href).toBe("/dashboard/analytics");
    expect(appNavEntry("dashboard").href).toBe("/dashboard");
  });
});
