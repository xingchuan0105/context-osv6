import { describe, expect, it } from "vitest";
import {
  localizeProgressActivity,
  progressSnapshotFromTurnMetadata,
} from "../../hooks/chat-session/progress-i18n";
import type { ChatEvent } from "../../lib/contracts";

function activity(
  partial: Partial<Extract<ChatEvent, { event: "activity" }>> &
    Pick<Extract<ChatEvent, { event: "activity" }>, "title">,
): Extract<ChatEvent, { event: "activity" }> {
  return {
    event: "activity",
    request_id: "r1",
    phase: partial.phase ?? "act:retrieve_semantic",
    title: partial.title,
    detail: partial.detail ?? null,
    counts: partial.counts ?? {},
    sources_preview: partial.sources_preview ?? [],
    timestamp: null,
  };
}

describe("localizeProgressActivity", () => {
  it("maps progress keys to Chinese and English", () => {
    const ev = activity({
      title: "progress.retrieve_semantic.running",
      detail: "数字化转型",
    });
    expect(localizeProgressActivity("zh-CN", ev).title).toBe("正在语义检索");
    expect(localizeProgressActivity("en", ev).title).toBe("Running semantic search");
    expect(localizeProgressActivity("zh-CN", ev).detail).toContain("数字化转型");
    expect(localizeProgressActivity("en", ev).detail).toContain("数字化转型");
  });

  it("formats hits with query in en", () => {
    const ev = activity({
      title: "progress.retrieve_semantic.done",
      detail: "core views",
      counts: { hits: 12 },
    });
    const { title, detail } = localizeProgressActivity("en", ev);
    expect(title).toBe("Semantic search complete");
    expect(detail).toBe("“core views” · 12 hits");
  });

  it("falls back to raw title when not a progress key", () => {
    const ev = activity({ title: "Legacy hard-coded step" });
    expect(localizeProgressActivity("en", ev).title).toBe("Legacy hard-coded step");
  });
});

describe("progressSnapshotFromTurnMetadata", () => {
  it("restores and localizes a server-side progress snapshot", () => {
    const snap = progressSnapshotFromTurnMetadata("zh-CN", {
      progress: {
        mode: "rag",
        collapsed: false,
        activities: [
          {
            id: "act-0",
            phase: "act:retrieve_semantic",
            title: "progress.retrieve_semantic.done",
            detail: "模块",
            counts: { hits: 3 },
            sources_preview: [],
          },
        ],
      },
    });
    expect(snap?.mode).toBe("rag");
    expect(snap?.activities).toHaveLength(1);
    expect(snap?.activities[0]?.title).toBe("完成语义检索");
    expect(snap?.activities[0]?.detail).toContain("模块");
  });
});
