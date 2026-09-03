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

  it("does not resurrect an in-flight deleted row from a racing poll", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    let polls = 0;
    listChatSessionFilesMock.mockImplementation(async () => {
      polls += 1;
      // Poll 2 resolves after the optimistic removal: without the in-flight
      // guard it would write the deleted row back.
      return polls <= 1 ? [readyRow()] : [];
    });
    deleteChatSessionFileMock.mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve({ status: "deleted" }), 50)),
    );

    render(
      <ChatCanvas
        selectedSourceIds={[]}
        sessionId="sess-race"
        workspaceId={null}
      />,
    );
    const removeButton = await screen.findByRole("button", { name: "移除" });
    await userEvent.click(removeButton);

    // The optimistic removal happens synchronously; the racing poll (already
    // dispatched with the row still present server-side) must not re-add it.
    await waitFor(() => {
      expect(screen.queryByText("report.txt")).not.toBeInTheDocument();
    });
  });
});
