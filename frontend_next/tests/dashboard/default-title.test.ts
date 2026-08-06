import { beforeEach, describe, expect, it } from "vitest";

import {
  formatDefaultWorkspaceTitle,
  getDefaultWorkspaceTitle,
  markDefaultWorkspaceTitleUsed,
  resetDefaultWorkspaceTitleCounters,
} from "../../lib/dashboard/default-title";

beforeEach(() => {
  resetDefaultWorkspaceTitleCounters();
});

describe("default workspace title helpers", () => {
  it("formats localized titles and duplicate suffixes (W7 #20)", () => {
    expect(formatDefaultWorkspaceTitle("zh-CN", "2026-04-17")).toBe("新建工作区1");
    expect(formatDefaultWorkspaceTitle("en", "2026-04-17")).toBe("New Workspace 1");
    expect(formatDefaultWorkspaceTitle("en", "2026-04-17", 1)).toBe("New Workspace 2");
  });

  it("uses local counters per locale", () => {
    expect(getDefaultWorkspaceTitle("en", "2026-04-17")).toBe("New Workspace 1");

    markDefaultWorkspaceTitleUsed("en", "2026-04-17");
    expect(getDefaultWorkspaceTitle("en", "2026-04-17")).toBe("New Workspace 2");

    markDefaultWorkspaceTitleUsed("en", "2026-04-17");
    expect(getDefaultWorkspaceTitle("en", "2026-04-17")).toBe("New Workspace 3");

    expect(getDefaultWorkspaceTitle("zh-CN", "2026-04-17")).toBe("新建工作区1");
  });

  it("never emits a digit-only bare title", () => {
    for (let i = 0; i < 5; i += 1) {
      expect(formatDefaultWorkspaceTitle("zh-CN", "", i)).not.toMatch(/^\d+$/);
      expect(formatDefaultWorkspaceTitle("en", "", i)).not.toMatch(/^\d+$/);
    }
  });
});
