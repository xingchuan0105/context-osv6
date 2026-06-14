import "../workspace-chat-pane.shared-mocks";

import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ComponentProps, ReactElement } from "react";
import { afterEach, beforeEach, expect, vi } from "vitest";

import { WorkspaceChatPane } from "../../../components/workspace/workspace-chat-pane";

import { resetWorkspaceChatPaneMocks } from "./workspace-chat-pane.setup";
import {
  queryLibraryCaptureHarnessMock,
  workspaceChatPaneMocks,
} from "../workspace-chat-pane.shared-mocks";

export { workspaceChatPaneMocks };

export type RenderChatPaneOptions = Partial<ComponentProps<typeof WorkspaceChatPane>>;

export async function renderChatPane(options: RenderChatPaneOptions = {}) {
  const props: ComponentProps<typeof WorkspaceChatPane> = {
    workspaceId: "ws-1",
    sessionId: null,
    selectedSourceIds: [],
    ...options,
  };

  const view = render(<WorkspaceChatPane {...props} />);

  await waitFor(() => {
    expect(screen.getByRole("textbox", { name: "工作区对话输入框" })).toBeTruthy();
  });

  return {
    ...view,
    props,
    composer: screen.getByRole("textbox", { name: "工作区对话输入框" }),
  };
}

/** Sync render for streaming/typewriter scenarios that use fake timers. */
export function renderStreamingChatPane(options: RenderChatPaneOptions = {}) {
  const props: ComponentProps<typeof WorkspaceChatPane> = {
    workspaceId: "ws-1",
    sessionId: null,
    selectedSourceIds: ["doc-1"],
    ...options,
  };

  const view = render(<WorkspaceChatPane {...props} />);

  return {
    ...view,
    props,
    composer: screen.getByRole("textbox", { name: "工作区对话输入框" }),
  };
}

export async function submitChatMessage(composer: HTMLElement, message: string) {
  await act(async () => {
    fireEvent.change(composer, { target: { value: message } });
    fireEvent.keyDown(composer, { key: "Enter" });
  });
}

export async function flushChatPaneMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

export function setupWorkspaceChatPaneTestLifecycle() {
  beforeEach(() => {
    resetWorkspaceChatPaneMocks(workspaceChatPaneMocks);
    queryLibraryCaptureHarnessMock.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });
}

export function rerenderChatPane(
  rerender: (ui: ReactElement) => void,
  options: RenderChatPaneOptions = {},
) {
  const props: ComponentProps<typeof WorkspaceChatPane> = {
    workspaceId: "ws-1",
    sessionId: null,
    selectedSourceIds: [],
    ...options,
  };

  rerender(<WorkspaceChatPane {...props} />);
}
