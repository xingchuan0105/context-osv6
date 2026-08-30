import { describe, expect, it } from "vitest";

import {
  DEFAULT_WORKSPACE_UI_STATE,
  WORKSPACE_UI_STORAGE_KEY,
  createWorkspaceUiStore,
} from "../../lib/workspace/ui-store";

describe("workspaceUiStore", () => {
  it("keeps workspace slices isolated by workspace id", () => {
    const store = createWorkspaceUiStore({ name: "workspace-ui-test-isolation" });

    store.getState().setHistoryRailOpen("ws-1", false);
    store.getState().setSelectedSourceIds("ws-1", ["src-1", "src-1", "src-2"]);
    store.getState().setCapabilities("ws-2", ["search"]);

    expect(store.getState().workspaces["ws-1"]).toMatchObject({
      historyRailOpen: false,
      selectedSourceIds: ["src-1", "src-2"],
      capabilities: DEFAULT_WORKSPACE_UI_STATE.capabilities,
      capabilitiesManual: DEFAULT_WORKSPACE_UI_STATE.capabilitiesManual,
    });
    expect(store.getState().workspaces["ws-2"]).toMatchObject({
      capabilities: ["search"],
      capabilitiesManual: true,
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
            rightRailWidth: 360,
            rightRailSplitRatio: 0.8,
            capabilitiesManual: false,
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

  it("auto-attaches rag without flipping manual, and manual overrides persist", () => {
    const store = createWorkspaceUiStore({ name: "workspace-ui-test-caps" });

    // Auto attach (source 0→N): rag on, still not manual.
    store.getState().setCapabilities("ws-1", ["rag"], { manual: false });
    expect(store.getState().workspaces["ws-1"]).toMatchObject({
      capabilities: ["rag"],
      capabilitiesManual: false,
    });

    // User toggles a chip manually → manual, auto attach must never override after this.
    store.getState().setCapabilities("ws-1", ["search"], { manual: true });
    expect(store.getState().workspaces["ws-1"]).toMatchObject({
      capabilities: ["search"],
      capabilitiesManual: true,
    });

    // Manual update keeps manual flag (default true without explicit options).
    store.getState().setCapabilities("ws-1", []);
    expect(store.getState().workspaces["ws-1"]).toMatchObject({
      capabilities: [],
      capabilitiesManual: true,
    });
  });
});