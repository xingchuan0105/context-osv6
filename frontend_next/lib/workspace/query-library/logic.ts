import {
  QUERY_LIBRARY_CAP,
  QUERY_LIBRARY_MIN_LENGTH,
  QUERY_LIBRARY_STOPWORDS,
} from "./constants";
import type { QueryLibraryItem } from "./types";

function toHalfWidth(value: string) {
  return value
    .replace(/[\uFF01-\uFF5E]/g, (char) => String.fromCharCode(char.charCodeAt(0) - 0xfee0))
    .replace(/\u3000/g, " ");
}

export function normalizeQueryText(value: string) {
  return toHalfWidth(value).replace(/\s+/g, " ").trim().toLowerCase();
}

function shouldCapture(raw: string) {
  const normalized = normalizeQueryText(raw);
  if (normalized.length < QUERY_LIBRARY_MIN_LENGTH) {
    return false;
  }

  return !QUERY_LIBRARY_STOPWORDS.has(normalized);
}

function createItem(text: string, now: number, createId: () => string): QueryLibraryItem {
  return {
    id: createId(),
    text,
    createdAt: now,
    lastUsedAt: now,
    useCount: 1,
  };
}

function applyCapacity(items: QueryLibraryItem[]) {
  if (items.length <= QUERY_LIBRARY_CAP) {
    return items;
  }

  const sortedByAge = [...items].sort((left, right) => left.lastUsedAt - right.lastUsedAt);
  const keepIds = new Set(sortedByAge.slice(-QUERY_LIBRARY_CAP).map((item) => item.id));
  return items.filter((item) => keepIds.has(item.id));
}

export function captureQueryLibraryItems(
  items: readonly QueryLibraryItem[],
  raw: string,
  now: number,
  createId: () => string = () => crypto.randomUUID(),
): QueryLibraryItem[] {
  const text = raw.trim();
  if (!text || !shouldCapture(text)) {
    return [...items];
  }

  const normalized = normalizeQueryText(text);
  const existingIndex = items.findIndex((item) => normalizeQueryText(item.text) === normalized);

  if (existingIndex >= 0) {
    const existing = items[existingIndex]!;
    const updated: QueryLibraryItem = {
      ...existing,
      lastUsedAt: now,
      useCount: existing.useCount + 1,
    };
    const nextItems = [updated, ...items.filter((item) => item.id !== existing.id)];
    return applyCapacity(nextItems);
  }

  const nextItems = [createItem(text, now, createId), ...items];
  return applyCapacity(nextItems);
}

export function searchQueryLibraryItems(items: readonly QueryLibraryItem[], query: string) {
  const tokens = normalizeQueryText(query).split(" ").filter(Boolean);
  const sorted = [...items].sort((left, right) => right.lastUsedAt - left.lastUsedAt);

  if (tokens.length === 0) {
    return sorted;
  }

  return sorted.filter((item) => {
    const normalizedText = normalizeQueryText(item.text);
    return tokens.every((token) => normalizedText.includes(token));
  });
}

export function touchQueryLibraryItem(
  items: readonly QueryLibraryItem[],
  id: string,
  now: number,
): QueryLibraryItem[] {
  const index = items.findIndex((item) => item.id === id);
  if (index < 0) {
    return [...items];
  }

  const existing = items[index]!;
  const updated: QueryLibraryItem = {
    ...existing,
    lastUsedAt: now,
    useCount: existing.useCount + 1,
  };

  return [updated, ...items.filter((item) => item.id !== id)];
}

export function removeQueryLibraryItem(items: readonly QueryLibraryItem[], id: string) {
  return items.filter((item) => item.id !== id);
}

export function insertAtCursor(
  draft: string,
  snippet: string,
  start: number,
  end: number,
): { nextDraft: string; nextCursor: number } {
  const length = draft.length;
  const safeStart = Math.max(0, Math.min(start, length));
  const safeEnd = Math.max(safeStart, Math.min(end, length));
  const nextDraft = draft.slice(0, safeStart) + snippet + draft.slice(safeEnd);
  const nextCursor = safeStart + snippet.length;

  return { nextDraft, nextCursor };
}
