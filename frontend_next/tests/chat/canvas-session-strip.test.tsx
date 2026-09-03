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