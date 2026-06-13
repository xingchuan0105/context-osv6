import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

import { mockReducedMotionPreference, resetWorkspaceChatPaneMocks } from "./helpers/workspace-chat-pane.setup";

import { WorkspaceChatPane } from "../../components/workspace/workspace-chat-pane";
import { workspaceUiStore } from "../../lib/workspace/ui-store";

beforeEach(() => {
  resetWorkspaceChatPaneMocks(mocks);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceChatPane composer", () => {
  it("renders a draggable composer handle and resizes the textarea", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(<WorkspaceChatPane workspaceId="ws-resize" sessionId={null} selectedSourceIds={[]} />);

    const composer = await screen.findByRole("textbox", { name: "工作区对话输入框" });
    const resizeHandle = screen.getByRole("button", { name: "调整输入框高度" });
    const initialHeight = Number.parseFloat((composer as HTMLTextAreaElement).style.height || "0");

    fireEvent.mouseDown(resizeHandle, { button: 0, clientY: 240 });
    fireEvent.mouseMove(window, { clientY: 180 });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      const resizedHeight = Number.parseFloat((composer as HTMLTextAreaElement).style.height || "0");
      expect(resizedHeight).toBeGreaterThan(initialHeight);
    });
  });
});
