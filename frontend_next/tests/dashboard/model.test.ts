import { describe, expect, it } from "vitest";

import {
  buildDashboardWorkspaceListState,
  formatDashboardWorkspaceDateLabel,
  formatDashboardWorkspaceDisplayTitle,
  formatDashboardWorkspaceRoleLabel,
  formatDashboardWorkspaceStatusSummary,
} from "../../lib/dashboard/model";

const workspaces = [
  {
    id: "ws-1",
    title: "Alpha",
    name: "Alpha",
    description: "First workspace",
    createdAt: "2026-04-17T10:00:00Z",
    updatedAt: "2026-04-17T10:00:00Z",
    ownerId: "user-1",
    statusSummary: {
      ready: 1,
      processing: 2,
    },
  },
  {
    id: "ws-2",
    title: "",
    name: "Bravo",
    description: "Contains the second project",
    createdAt: "2026-04-16T10:00:00Z",
    updatedAt: "2026-04-16T10:00:00Z",
    ownerId: "user-2",
    statusSummary: {
      completed: 2,
      indexing: 3,
      error: 1,
    },
  },
  {
    id: "ws-3",
    title: "charlie",
    name: "charlie",
    description: "",
    createdAt: "2026-04-18T10:00:00Z",
    updatedAt: "2026-04-18T10:00:00Z",
    ownerId: "user-1",
    statusSummary: {},
  },
] as const;

describe("dashboard model helpers", () => {
  it("falls back to the notebook name when the title is empty", () => {
    expect(formatDashboardWorkspaceDisplayTitle(workspaces[1])).toBe("Bravo");
  });

  it("formats dates, roles, and status summaries for each locale", () => {
    expect(formatDashboardWorkspaceDateLabel("zh-CN", "2026-04-17T10:00:00Z")).toBe("2026年4月17日");
    expect(formatDashboardWorkspaceDateLabel("en", "2026-04-17T10:00:00Z")).toBe("2026-04-17");

    expect(formatDashboardWorkspaceRoleLabel("zh-CN", true)).toBe("所有者");
    expect(formatDashboardWorkspaceRoleLabel("zh-CN", false)).toBe("成员");
    expect(formatDashboardWorkspaceRoleLabel("en", false)).toBe("Member");

    expect(
      formatDashboardWorkspaceStatusSummary("en", {
        ready: 1,
        completed: 2,
        queued: 3,
        error: 4,
      }),
    ).toBe("3 ready · 3 processing · 4 failed");

    expect(
      formatDashboardWorkspaceStatusSummary("zh-CN", {
        ready: 1,
        completed: 2,
        indexing: 3,
        failed: 4,
      }),
    ).toBe("3 就绪 · 3 处理中 · 4 异常");
  });

  it("filters, searches, sorts, and projects dashboard workspaces", () => {
    const allRecent = buildDashboardWorkspaceListState(workspaces, {
      locale: "en",
      currentUserId: "user-1",
      favoriteIds: ["ws-1"],
      tab: "all",
      sort: "recent",
      query: "",
    });

    expect(allRecent.map((workspace) => workspace.id)).toEqual(["ws-3", "ws-1", "ws-2"]);
    expect(allRecent[1]).toMatchObject({
      title: "Alpha",
      description: "First workspace",
      dateLabel: "2026-04-17",
      roleLabel: "Owner",
      statusSummaryLabel: "1 ready · 2 processing",
      isFavorite: true,
    });
    expect(allRecent[2]).toMatchObject({
      title: "Bravo",
      description: "Contains the second project",
      dateLabel: "2026-04-16",
      roleLabel: "Member",
      statusSummaryLabel: "2 ready · 3 processing · 1 failed",
      isFavorite: false,
    });

    const mine = buildDashboardWorkspaceListState(workspaces, {
      locale: "en",
      currentUserId: "user-1",
      favoriteIds: ["ws-1"],
      tab: "mine",
      sort: "title",
      query: "",
    });
    expect(mine.map((workspace) => workspace.id)).toEqual(["ws-1", "ws-3"]);

    const favorites = buildDashboardWorkspaceListState(workspaces, {
      locale: "en",
      currentUserId: "user-1",
      favoriteIds: ["ws-1"],
      tab: "favorites",
      sort: "title",
      query: "",
    });
    expect(favorites.map((workspace) => workspace.id)).toEqual(["ws-1"]);

    const searchByTitleFallback = buildDashboardWorkspaceListState(workspaces, {
      locale: "en",
      currentUserId: "user-1",
      favoriteIds: ["ws-1"],
      tab: "all",
      sort: "recent",
      query: "bravo",
    });
    expect(searchByTitleFallback.map((workspace) => workspace.id)).toEqual(["ws-2"]);

    const searchByDescription = buildDashboardWorkspaceListState(workspaces, {
      locale: "en",
      currentUserId: "user-1",
      favoriteIds: ["ws-1"],
      tab: "all",
      sort: "recent",
      query: "project",
    });
    expect(searchByDescription.map((workspace) => workspace.id)).toEqual(["ws-2"]);
  });
});
