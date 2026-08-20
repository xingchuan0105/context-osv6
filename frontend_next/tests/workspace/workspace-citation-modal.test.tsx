import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const lookupWorkspaceCitationMock = vi.fn();

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({ token: "token-123" }),
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
  lookupWorkspaceCitation: (...args: unknown[]) => lookupWorkspaceCitationMock(...args),
}));

import { WorkspaceCitationModal } from "../../components/workspace/workspace-citation-modal";
import type { WorkspaceCitationRequest } from "../../lib/workspace/model";

function citationRequest(overrides: Partial<WorkspaceCitationRequest> = {}): WorkspaceCitationRequest {
  return {
    session_id: "sess-1",
    message_id: 12,
    citation: {
      citation_id: 1,
      doc_id: "doc-1",
      chunk_id: "chunk-1",
      doc_name: "Doc One",
      score: 0.9,
      content: "Local chunk body from the stream payload",
    },
    ...overrides,
  };
}

describe("WorkspaceCitationModal", () => {
  it("skips lookup and shows local content when message_id is the stream placeholder", async () => {
    lookupWorkspaceCitationMock.mockRejectedValue(new Error("should not be called"));

    render(
      <WorkspaceCitationModal
        citationRequest={citationRequest({ message_id: 0 })}
        workspaceId="ws-1"
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByTestId("workspace-citation-body")).toHaveTextContent(
      "Local chunk body from the stream payload",
    );
    expect(lookupWorkspaceCitationMock).not.toHaveBeenCalled();
  });

  it("falls back to local citation text when lookup fails", async () => {
    lookupWorkspaceCitationMock.mockRejectedValue(new Error("not found"));

    render(
      <WorkspaceCitationModal
        citationRequest={citationRequest()}
        workspaceId="ws-1"
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(lookupWorkspaceCitationMock).toHaveBeenCalledWith("token-123", {
        session_id: "sess-1",
        message_id: 12,
        citation_id: 1,
      });
      expect(screen.getByTestId("workspace-citation-body")).toHaveTextContent(
        "Local chunk body from the stream payload",
      );
    });
    expect(screen.queryByText("加载引用片段失败。")).toBeNull();
  });
});
