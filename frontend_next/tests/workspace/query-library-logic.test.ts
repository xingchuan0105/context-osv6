import { describe, expect, it } from "vitest";

import {
  captureQueryLibraryItems,
  insertAtCursor,
  normalizeQueryText,
  removeQueryLibraryItem,
  searchQueryLibraryItems,
  touchQueryLibraryItem,
} from "../../lib/workspace/query-library/logic";
import type { QueryLibraryItem } from "../../lib/workspace/query-library/types";

function createItem(
  id: string,
  text: string,
  lastUsedAt: number,
  useCount = 1,
): QueryLibraryItem {
  return {
    id,
    text,
    createdAt: lastUsedAt,
    lastUsedAt,
    useCount,
  };
}

describe("normalizeQueryText", () => {
  it("collapses whitespace and lowercases", () => {
    expect(normalizeQueryText("  Hello   World  ")).toBe("hello world");
  });

  it("converts fullwidth characters", () => {
    expect(normalizeQueryText("ＡＢＣ　１２３")).toBe("abc 123");
  });
});

describe("captureQueryLibraryItems", () => {
  it("filters empty, short, and stopword queries", () => {
    const items: QueryLibraryItem[] = [];

    expect(captureQueryLibraryItems(items, "", 1)).toEqual([]);
    expect(captureQueryLibraryItems(items, "   ", 1)).toEqual([]);
    expect(captureQueryLibraryItems(items, "ok", 1)).toEqual([]);
    expect(captureQueryLibraryItems(items, "continue", 1)).toEqual([]);
    expect(captureQueryLibraryItems(items, "好的好的", 1)).toEqual([]);
  });

  it("inserts a new query at the head", () => {
    const items = captureQueryLibraryItems([], "  Summarize this PDF  ", 100, () => "id-1");

    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      id: "id-1",
      text: "Summarize this PDF",
      createdAt: 100,
      lastUsedAt: 100,
      useCount: 1,
    });
  });

  it("updates duplicates instead of creating new entries", () => {
    const initial = captureQueryLibraryItems([], "Summarize this PDF", 100, () => "id-1");
    const next = captureQueryLibraryItems(initial, "summarize   this pdf", 200, () => "id-2");

    expect(next).toHaveLength(1);
    expect(next[0]).toMatchObject({
      id: "id-1",
      text: "Summarize this PDF",
      lastUsedAt: 200,
      useCount: 2,
    });
  });

  it("evicts the oldest item when capacity is exceeded", () => {
    const items = Array.from({ length: 200 }, (_, index) =>
      createItem(`id-${index}`, `query ${index}`, index + 1),
    );
    const next = captureQueryLibraryItems(items, "brand new query", 999, () => "id-new");

    expect(next).toHaveLength(200);
    expect(next.some((item) => item.id === "id-0")).toBe(false);
    expect(next[0]).toMatchObject({
      id: "id-new",
      text: "brand new query",
      lastUsedAt: 999,
    });
  });
});

describe("searchQueryLibraryItems", () => {
  const items = [
    createItem("a", "Summarize quarterly report", 300),
    createItem("b", "Rewrite in formal tone", 200),
    createItem("c", "Summarize the contract", 100),
  ];

  it("returns all items sorted by lastUsedAt when query is empty", () => {
    expect(searchQueryLibraryItems(items, "")).toEqual(items);
    expect(searchQueryLibraryItems(items, "   ")).toEqual(items);
  });

  it("matches all tokens regardless of order", () => {
    expect(searchQueryLibraryItems(items, "formal rewrite")).toEqual([items[1]]);
  });

  it("treats Chinese queries as a single token", () => {
    const chineseItems = [createItem("zh", "帮我把这段改写成正式语气", 100)];
    expect(searchQueryLibraryItems(chineseItems, "正式语气")).toEqual([chineseItems[0]]);
    expect(searchQueryLibraryItems(chineseItems, "改写 合同")).toEqual([]);
  });
});

describe("touchQueryLibraryItem", () => {
  it("moves the touched item to the head and increments useCount", () => {
    const items = [
      createItem("a", "first", 100, 1),
      createItem("b", "second", 200, 2),
    ];
    const next = touchQueryLibraryItem(items, "a", 300);

    expect(next[0]).toMatchObject({ id: "a", lastUsedAt: 300, useCount: 2 });
    expect(next[1]?.id).toBe("b");
  });
});

describe("removeQueryLibraryItem", () => {
  it("removes the matching item", () => {
    const items = [createItem("a", "first", 100), createItem("b", "second", 200)];
    expect(removeQueryLibraryItem(items, "a")).toEqual([items[1]]);
  });
});

describe("insertAtCursor", () => {
  it("inserts at the cursor without adding separators", () => {
    expect(insertAtCursor("hello world", "INSERT", 5, 5)).toEqual({
      nextDraft: "helloINSERT world",
      nextCursor: 11,
    });
  });

  it("replaces the selected range", () => {
    expect(insertAtCursor("hello world", "X", 0, 5)).toEqual({
      nextDraft: "X world",
      nextCursor: 1,
    });
  });

  it("clamps out-of-range cursor positions", () => {
    expect(insertAtCursor("abc", "Z", -5, 0)).toEqual({
      nextDraft: "Zabc",
      nextCursor: 1,
    });
  });

  it("supports chained inserts via returned cursor", () => {
    const first = insertAtCursor("ab", "1", 1, 1);
    const second = insertAtCursor(first.nextDraft, "2", first.nextCursor, first.nextCursor);

    expect(second).toEqual({
      nextDraft: "a12b",
      nextCursor: 3,
    });
  });
});
