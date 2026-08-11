import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CitationRenderer } from "../../components/workspace/citation-renderer";
import type { UiChatMessage } from "../../hooks/use-chat-session";
import type { Citation } from "../../lib/workspace/stream";

function webCitation(id: number, title: string, url: string): Citation {
  return {
    citation_id: id,
    doc_id: url,
    doc_name: title,
    preview: `${title} snippet`,
    score: 0.9,
    layer: "search",
    chunk_type: "web",
    source_locator: { url, title },
  };
}

function searchMessage(
  content: string,
  citations: Citation[],
): UiChatMessage {
  return {
    id: "m1",
    role: "assistant",
    mode: "search",
    capabilities: ["search"],
    content,
    answerBlocks: [],
    citations,
    degradeTrace: [],
    guarded: false,
    messageId: 1,
    sessionId: "sess-1",
    toolResults: [],
  };
}

describe("CitationRenderer web observation indices", () => {
  it("resolves [[web:n]] by citation_id even when the filtered list is sparse", async () => {
    const onOpenWebSources = vi.fn();
    const onSelectCitation = vi.fn();
    const user = userEvent.setup();

    // Sparse list: only observation indices 2 and 7 (as backend filter keeps referenced ids).
    // Array position must NOT be used: index 0 would wrongly match [[web:1]].
    render(
      <CitationRenderer
        locale="zh-CN"
        message={searchMessage("Fact A [[web:7]] and B [[web:2]].", [
          webCitation(2, "Second Source", "https://example.com/2"),
          webCitation(7, "Seventh Source", "https://example.com/7"),
        ])}
        onOpenWebSources={onOpenWebSources}
        onSelectCitation={onSelectCitation}
      />,
    );

    const chips = await screen.findAllByTestId("workspace-citation");
    expect(chips).toHaveLength(2);

    await user.click(chips[0]!);
    expect(onOpenWebSources).toHaveBeenCalledWith({
      sources: [
        expect.objectContaining({
          title: "Seventh Source",
          url: "https://example.com/7",
        }),
      ],
    });

    onOpenWebSources.mockClear();
    await user.click(chips[1]!);
    expect(onOpenWebSources).toHaveBeenCalledWith({
      sources: [
        expect.objectContaining({
          title: "Second Source",
          url: "https://example.com/2",
        }),
      ],
    });
    expect(onSelectCitation).not.toHaveBeenCalled();
  });

  it("renders non-clickable fallback for missing [[web:n]] (not a wrong source)", async () => {
    const { container } = render(
      <CitationRenderer
        locale="zh-CN"
        message={searchMessage("Missing [[web:99]] marker.", [
          webCitation(1, "Only One", "https://example.com/1"),
        ])}
        onOpenWebSources={vi.fn()}
        onSelectCitation={vi.fn()}
      />,
    );

    // Unresolved [[web:99]] → fallback span, not a button wired to citation 1.
    const fallback = container.querySelector('[class*="inlineCitationFallback"]');
    expect(fallback).toBeTruthy();
    expect(fallback?.textContent).toBe("1");
  });
});
