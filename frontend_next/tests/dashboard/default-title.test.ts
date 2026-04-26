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
  it("formats localized titles and duplicate suffixes", () => {
    expect(formatDefaultWorkspaceTitle("zh-CN", "2026-04-17")).toBe("工作区1");
    expect(formatDefaultWorkspaceTitle("en", "2026-04-17")).toBe("Workspace1");
    expect(formatDefaultWorkspaceTitle("en", "2026-04-17", 1)).toBe("Workspace2");
  });

  it("uses local counters per locale", () => {
    expect(getDefaultWorkspaceTitle("en", "2026-04-17")).toBe("Workspace1");

    markDefaultWorkspaceTitleUsed("en", "2026-04-17");
    expect(getDefaultWorkspaceTitle("en", "2026-04-17")).toBe("Workspace2");

    markDefaultWorkspaceTitleUsed("en", "2026-04-17");
    expect(getDefaultWorkspaceTitle("en", "2026-04-17")).toBe("Workspace3");

    expect(getDefaultWorkspaceTitle("zh-CN", "2026-04-17")).toBe("工作区1");
  });
});
