import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createWorkspaceChatPaneMocks());

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.useAuthMock(),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../../lib/workspace/client", () => ({
  listWorkspaceSessionMessages: mocks.listWorkspaceSessionMessagesMock,
}));

vi.mock("../../lib/runtime/transport", () => ({
  streamChat: mocks.streamWorkspaceChatMock,
}));

const listChatSessionFilesMock = vi.hoisted(() => vi.fn());
const deleteChatSessionFileMock = vi.hoisted(() => vi.fn());

vi.mock("../../lib/chat/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/chat/client")>();
  return {
    ...actual,
    listChatSessionFiles: (...args: unknown[]) => listChatSessionFilesMock(...args),
    deleteChatSessionFile: (...args: unknown[]) => deleteChatSessionFileMock(...args),
  };
});

import { ChatCanvas } from "../../components/chat/chat-canvas";
import { workspaceUiStore } from "../../lib/workspace/ui-store";
import { resetWorkspaceChatPaneMocks } from "../workspace/helpers/workspace-chat-pane.setup";

beforeEach(() => {
  resetWorkspaceChatPaneMocks(mocks);
  listChatSessionFilesMock.mockReset();
  deleteChatSessionFileMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

function readyRow(overrides: Partial<{ binding_id: string; status: string }> = {}) {
  return {
    binding_id: "bind-1",
    document_id: "doc-1",
    file_name: "report.txt",
    mime_type: "text/plain",
    file_size: 4,
    status: "completed",
    created_at: "2026-09-03T08:00:00Z",
    ...overrides,
  };
}

describe("ChatCanvas personal-conversation RAG strip (review round-4)", () => {
  it("strips auto-attached rag when the last ready file is deleted (refresh fails)", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    // First observation: one ready file → the canvas auto-attaches rag.
    listChatSessionFilesMock.mockResolvedValue([readyRow()]);

    render(
      <ChatCanvas
        selectedSourceIds={[]}
        sessionId="sess-strip"
        workspaceId={null}
      />,
    );

    await waitFor(() => {
      const state = workspaceUiStore.getState().workspaces["chat:sess-strip"];
      expect(state?.capabilities).toContain("rag");
    });

    // Deletion succeeds server-side, but the follow-up refresh FAILS — the
    // optimistic removal must still drive ready → 0 and strip rag.
    deleteChatSessionFileMock.mockResolvedValue({ status: "deleted" });
    listChatSessionFilesMock.mockRejectedValue(new Error("network down"));

    const removeButton = await screen.findByRole("button", { name: "移除" });
    await userEvent.click(removeButton);

    await waitFor(() => {
      expect(deleteChatSessionFileMock).toHaveBeenCalledWith("token-123", "sess-strip", "bind-1");
    });
    await waitFor(() => {
      const state = workspaceUiStore.getState().workspaces["chat:sess-strip"];
      expect(state?.capabilities).not.toContain("rag");
    });
  });

  it("keeps user-selected (manual) rag when ready files drop to zero", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    listChatSessionFilesMock.mockResolvedValue([readyRow()]);

    render(
      <ChatCanvas
        selectedSourceIds={[]}
        sessionId="sess-manual"
        workspaceId={null}
      />,
    );

    await waitFor(() => {
      const state = workspaceUiStore.getState().workspaces["chat:sess-manual"];
      expect(state?.capabilities).toContain("rag");
    });

    act(() => {
      workspaceUiStore
        .getState()
        .setCapabilities("chat:sess-manual", ["rag"], { manual: true });
    });
    listChatSessionFilesMock.mockResolvedValue([]);

    const removeButton = await screen.findByRole("button", { name: "移除" });
    await userEvent.click(removeButton);

    await waitFor(() => {
      const state = workspaceUiStore.getState().workspaces["chat:sess-manual"];
      expect(state?.capabilities).toContain("rag");
      expect(state?.capabilitiesManual).toBe(true);
    });
  });
});
describe("SessionFileTray delete failure paths (review round-5)", () => {
  it("rolls the row back and surfaces an error when DELETE fails", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    // Initial load returns the ready row; every later refresh fails.
    listChatSessionFilesMock
      .mockResolvedValueOnce([readyRow()])
      .mockRejectedValue(new Error("refresh failed"));
    // DELETE fails and the reconcile refresh fails too — the row must be
    // rolled back, not silently dropped while the binding still exists.
    deleteChatSessionFileMock.mockRejectedValue(new Error("delete failed"));

    render(
      <ChatCanvas
        selectedSourceIds={[]}
        sessionId="sess-rollback"
        workspaceId={null}
      />,
    );
    const removeButton = await screen.findByRole("button", { name: "移除" });
    await userEvent.click(removeButton);

    await waitFor(() => {
      expect(deleteChatSessionFileMock).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByText("report.txt")).toBeInTheDocument();
    });
    // Upload-failure alert is surfaced instead of a silent fork.
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    // The auto-RAG strip must NOT fire while the binding is in doubt.
    const state = workspaceUiStore.getState().workspaces["chat:sess-rollback"];
    expect(state?.capabilities).toContain("rag");
  });

  it("keeps a deleted binding dead when an in-flight poll GET lands after the delete", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    // Start in-flight: a `processing` row is what makes production start the
    // 2s poll interval (review round-7 Spec-4 — the previous version seeded a
    // completed row, which never polls, so no in-flight GET existed).
    // Server truth never converges — every GET returns the row — so the ONLY
    // thing keeping the row dead is the tombstone.
    listChatSessionFilesMock.mockResolvedValue([
      readyRow({ binding_id: "bind-1", status: "processing" }),
    ]);

    // Deferred poll GET (review round-8 S5): the FIRST poll tick's GET is
    // captured unresolved. It is dispatched BEFORE the delete completes and
    // only released AFTER it — a genuinely in-flight read crossing the
    // delete, not an immediate-resolve mock.
    let releasePollGet: ((value: unknown) => void) | null = null;
    const deferredGet = new Promise((resolve) => {
      releasePollGet = resolve;
    });
    listChatSessionFilesMock.mockImplementationOnce(async () => [
      readyRow({ binding_id: "bind-1", status: "processing" }),
    ]);
    let pollTicks = 0;
    listChatSessionFilesMock.mockImplementation(async () => {
      pollTicks += 1;
      if (pollTicks === 1) {
        // First poll tick: dispatch and HOLD the response until released.
        return deferredGet;
      }
      return [readyRow({ binding_id: "bind-1", status: "processing" })];
    });

    vi.useFakeTimers();
    try {
      render(
        <ChatCanvas
          selectedSourceIds={[]}
          sessionId="sess-race"
          workspaceId={null}
        />,
      );
      // Initial GET ran (component mounted with the processing row); the
      // remove button renders with the row.
      await act(async () => {
        await vi.runOnlyPendingTimersAsync();
        await Promise.resolve();
      });
      const removeButton = screen.getByRole("button", { name: "移除" });
      expect(screen.getByText("report.txt")).toBeInTheDocument();
      const getCallsAfterMount = listChatSessionFilesMock.mock.calls.length;

      // Fire ONE poll tick whose GET stays in flight across the delete.
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2000);
      });
      const inFlightCalls = listChatSessionFilesMock.mock.calls.length;
      expect(inFlightCalls).toBe(getCallsAfterMount + 1);
      expect(releasePollGet).not.toBeNull();

      // Delete while that GET is still pending.
      await act(async () => {
        fireEvent.click(removeButton);
        await vi.advanceTimersByTimeAsync(80);
      });
      expect(deleteChatSessionFileMock).toHaveBeenCalled();
      expect(screen.queryByText("report.txt")).not.toBeInTheDocument();

      // Now release the pre-delete GET — it resolves AFTER the delete. The
      // tombstone must keep its stale row dead.
      await act(async () => {
        releasePollGet!([readyRow({ binding_id: "bind-1", status: "processing" })]);
        await deferredGet;
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(screen.queryByText("report.txt")).not.toBeInTheDocument();

      // Later poll ticks (stale GETs too) are filtered by the tombstone as well.
      await act(async () => {
        await vi.advanceTimersByTimeAsync(6000);
        await Promise.resolve();
      });
      expect(screen.queryByText("report.txt")).not.toBeInTheDocument();
      expect(listChatSessionFilesMock.mock.calls.length).toBeGreaterThan(
        getCallsAfterMount + 1,
      );
    } finally {
      vi.useRealTimers();
    }
  });
});
