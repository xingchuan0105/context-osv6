import { describe, expect, it } from "vitest";

import { getMessageCatalog } from "../../lib/i18n/messages";

describe("getMessageCatalog", () => {
  it("builds nested next-intl messages without flat dotted keys", () => {
    const catalog = getMessageCatalog("en");

    expect(catalog.workspaceRightRail).toBeTruthy();
    expect(catalog.workspaceRightRail).toMatchObject({
      sourcesSectionTitle: "Sources",
    });
    expect(Object.prototype.hasOwnProperty.call(catalog, "workspaceRightRail.sourcesSectionTitle")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(catalog, "settings.tabsLabel")).toBe(false);
  });
});
