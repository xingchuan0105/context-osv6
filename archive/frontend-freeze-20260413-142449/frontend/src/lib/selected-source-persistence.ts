type DocumentIdLike = { id: string };

function uniqueStringIds(items: Iterable<unknown>): string[] {
  const seen = new Set<string>();
  const ids: string[] = [];

  for (const item of items) {
    if (typeof item !== 'string') {
      continue;
    }

    const normalized = item.trim();
    if (!normalized || seen.has(normalized)) {
      continue;
    }

    seen.add(normalized);
    ids.push(normalized);
  }

  return ids;
}

export function parsePersistedSelectedSourceIds(raw: string | null): string[] {
  if (!raw) {
    return [];
  }

  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return uniqueStringIds(parsed);
  } catch {
    return [];
  }
}

export function stringifySelectedSourceIds(selectedSourceIds: Iterable<string>): string | null {
  const normalized = uniqueStringIds(selectedSourceIds);
  if (normalized.length === 0) {
    return null;
  }
  return JSON.stringify(normalized);
}

export function filterSelectedSourceIdsByDocumentIds(
  selectedSourceIds: Iterable<string>,
  documents: DocumentIdLike[]
): string[] {
  const validDocumentIds = new Set(documents.map((document) => document.id));
  const normalized = uniqueStringIds(selectedSourceIds);

  return normalized.filter((id) => validDocumentIds.has(id));
}
