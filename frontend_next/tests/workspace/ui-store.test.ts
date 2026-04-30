import { describe, expect, it } from "vitest";

import {
  DEFAULT_WORKSPACE_UI_STATE,
  WORKSPACE_UI_STORAGE_KEY,
  createWorkspaceUiStore,
  resolveWorkspaceChatMode,
} from "../../lib/workspace/ui-store";

describe("workspaceUiStore", () => {
  it("keeps workspace slices isolated by workspace id", () => {
    const store = createWorkspaceUiStore({ name: "workspace-ui-test-isolation" });

    store.getState().setHistoryRailOpen("ws-1", false);
    store.getState().setSelectedSourceIds("ws-1", ["src-1", "src-1", "src-2"]);
    store.getState().setChatMode("ws-2", "search");

    expect(store.getState().workspaces["ws-1"]).toMatchObject({
      historyRailOpen: false,
      selectedSourceIds: ["src-1", "src-2"],
      chatMode: DEFAULT_WORKSPACE_UI_STATE.chatMode,
      chatModePreference: DEFAULT_WORKSPACE_UI_STATE.chatModePreference,
    });
    expect(store.getState().workspaces["ws-2"]).toMatchObject({
      chatMode: "search",
      chatModePreference: "manual",
      historyRailOpen: DEFAULT_WORKSPACE_UI_STATE.historyRailOpen,
    });
  });

  it("persists workspace UI state into localStorage", () => {
    window.localStorage.removeItem(WORKSPACE_UI_STORAGE_KEY);

    const store = createWorkspaceUiStore();
    store.getState().setRightRailSplitRatio("ws-1", 1.5);
    store.getState().setHistoryRailWidth("ws-1", 120);
    store.getState().setRightRailWidth("ws-1", 999);

    const persisted = window.localStorage.getItem(WORKSPACE_UI_STORAGE_KEY);

    expect(persisted).not.toBeNull();
    expect(JSON.parse(persisted!)).toMatchObject({
      state: {
        workspaces: {
          "ws-1": {
            historyRailWidth: 236,
            rightRailWidth: 392,
            rightRailSplitRatio: 0.8,
            chatModePreference: "auto",
          },
        },
      },
    });
  });

  it("migrates legacy default rail widths to the new maximum defaults", () => {
    const store = createWorkspaceUiStore({ name: "workspace-ui-test-migration" });

    store.setState({
      workspaces: {
        "ws-1": {
          ...DEFAULT_WORKSPACE_UI_STATE,
          historyRailWidth: 264,
          rightRailWidth: 336,
        },
      },
    });

    store.getState().setSelectedSourceIds("ws-1", ["src-1"]);

    expect(store.getState().workspaces["ws-1"]).toMatchObject({
      historyRailWidth: DEFAULT_WORKSPACE_UI_STATE.historyRailWidth,
      rightRailWidth: DEFAULT_WORKSPACE_UI_STATE.rightRailWidth,
      selectedSourceIds: ["src-1"],
    });
  });

  it("resolves auto mode from content availability and preserves manual mode overrides", () => {
    const store = createWorkspaceUiStore({ name: "workspace-ui-test-chat-mode" });

    const initial = store.getState().workspaces["ws-1"] ?? DEFAULT_WORKSPACE_UI_STATE;

    expect(resolveWorkspaceChatMode(initial, false)).toBe("chat");
    expect(resolveWorkspaceChatMode(initial, true)).toBe("rag");

    store.getState().setChatMode("ws-1", "search");

    const manualState = store.getState().workspaces["ws-1"]!;
    expect(resolveWorkspaceChatMode(manualState, false)).toBe("search");
    expect(resolveWorkspaceChatMode(manualState, true)).toBe("search");
  });
});
