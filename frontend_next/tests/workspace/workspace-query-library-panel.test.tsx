import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "en" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

import { WorkspaceQueryLibraryPanel } from "../../components/workspace/workspace-query-library-panel";
import { insertAtCursor } from "../../lib/workspace/query-library/logic";
import { queryLibraryStore } from "../../lib/workspace/query-library/store";

describe("WorkspaceQueryLibraryPanel", () => {
  beforeEach(() => {
    window.localStorage.clear();
    queryLibraryStore.setState({ workspaces: {} });
  });

  it("shows the empty state before any prompts are captured", () => {
    render(<WorkspaceQueryLibraryPanel workspaceId="ws-1" onInsert={vi.fn(() => true)} />);

    expect(screen.getByText("Your sent prompts will appear here")).toBeTruthy();
  });

  it("filters prompts and inserts on item click", async () => {
    const user = userEvent.setup();
    const onInsert = vi.fn(() => true);

    queryLibraryStore.getState().capture("ws-1", "Summarize quarterly report");
    queryLibraryStore.getState().capture("ws-1", "Rewrite in formal tone");

    render(<WorkspaceQueryLibraryPanel workspaceId="ws-1" onInsert={onInsert} />);

    expect(screen.getByText("Summarize quarterly report")).toBeTruthy();
    expect(screen.getByText("Rewrite in formal tone")).toBeTruthy();

    await user.type(screen.getByRole("searchbox", { name: "Search prompts…" }), "formal");
    expect(screen.queryByText("Summarize quarterly report")).toBeNull();
    expect(screen.getByText("Rewrite in formal tone")).toBeTruthy();

    await user.click(screen.getByText("Rewrite in formal tone"));
    expect(onInsert).toHaveBeenCalledWith("Rewrite in formal tone");
    expect(queryLibraryStore.getState().workspaces["ws-1"]?.[0]?.text).toBe("Rewrite in formal tone");
  });

  it("deletes an item without triggering insert", async () => {
    const user = userEvent.setup();
    const onInsert = vi.fn(() => true);

    queryLibraryStore.getState().capture("ws-1", "Summarize quarterly report");
    render(<WorkspaceQueryLibraryPanel workspaceId="ws-1" onInsert={onInsert} />);

    const item = screen.getByTestId("query-library-item");
    await user.click(within(item).getByRole("button", { name: "Delete" }));

    expect(onInsert).not.toHaveBeenCalled();
    expect(screen.getByText("Your sent prompts will appear here")).toBeTruthy();
  });

  it("does not touch when insert is rejected", async () => {
    const user = userEvent.setup();
    const onInsert = vi.fn(() => false);

    queryLibraryStore.getState().capture("ws-1", "Summarize quarterly report");
    render(<WorkspaceQueryLibraryPanel workspaceId="ws-1" onInsert={onInsert} />);

    await user.click(screen.getByText("Summarize quarterly report"));

    expect(onInsert).toHaveBeenCalledWith("Summarize quarterly report");
    expect(queryLibraryStore.getState().workspaces["ws-1"]?.[0]?.useCount).toBe(1);
  });

  it("resets search when workspace changes", () => {
    const onInsert = vi.fn(() => true);

    queryLibraryStore.getState().capture("ws-1", "Summarize quarterly report");
    queryLibraryStore.getState().capture("ws-2", "Rewrite in formal tone");

    const { rerender } = render(<WorkspaceQueryLibraryPanel workspaceId="ws-1" onInsert={onInsert} />);

    fireEvent.change(screen.getByRole("searchbox", { name: "Search prompts…" }), {
      target: { value: "formal" },
    });
    expect(screen.queryByText("Summarize quarterly report")).toBeNull();

    rerender(<WorkspaceQueryLibraryPanel workspaceId="ws-2" onInsert={onInsert} />);

    expect(screen.getByRole("searchbox", { name: "Search prompts…" })).toHaveValue("");
    expect(screen.getByText("Rewrite in formal tone")).toBeTruthy();
  });

  it("concatenates prompts when two items are clicked in sequence", async () => {
    const user = userEvent.setup();
    let draft = "";
    const onInsert = vi.fn((text: string) => {
      const { nextDraft } = insertAtCursor(draft, text, draft.length, draft.length);
      draft = nextDraft;
      return true;
    });

    queryLibraryStore.getState().capture("ws-1", "Prompt alpha");
    queryLibraryStore.getState().capture("ws-1", "Prompt beta");

    render(<WorkspaceQueryLibraryPanel workspaceId="ws-1" onInsert={onInsert} />);

    await user.click(screen.getByText("Prompt beta"));
    await user.click(screen.getByText("Prompt alpha"));

    expect(onInsert).toHaveBeenCalledTimes(2);
    expect(draft).toBe("Prompt betaPrompt alpha");
  });
});
