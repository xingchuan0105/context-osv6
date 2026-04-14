import { describe, expect, it } from 'vitest';
import {
  filterSelectedSourceIdsByDocumentIds,
  parsePersistedSelectedSourceIds,
  stringifySelectedSourceIds,
} from './selected-source-persistence';

describe('selected-source-persistence', () => {
  it('parses persisted ids and removes duplicates/invalid values', () => {
    const parsed = parsePersistedSelectedSourceIds('["doc-1", "", "doc-1", 1, " doc-2 "]');

    expect(parsed).toEqual(['doc-1', 'doc-2']);
  });

  it('returns empty array for invalid persisted content', () => {
    expect(parsePersistedSelectedSourceIds(null)).toEqual([]);
    expect(parsePersistedSelectedSourceIds('not-json')).toEqual([]);
    expect(parsePersistedSelectedSourceIds('{"id":"doc-1"}')).toEqual([]);
  });

  it('stringifies normalized ids and returns null for empty selections', () => {
    expect(stringifySelectedSourceIds(['doc-1', ' doc-2 ', 'doc-1'])).toBe('["doc-1","doc-2"]');
    expect(stringifySelectedSourceIds([])).toBeNull();
  });

  it('filters selected ids by existing documents while preserving order', () => {
    const filtered = filterSelectedSourceIdsByDocumentIds(['doc-2', 'doc-1', 'doc-2', 'doc-3'], [
      { id: 'doc-1' },
      { id: 'doc-2' },
    ]);

    expect(filtered).toEqual(['doc-2', 'doc-1']);
  });
});
