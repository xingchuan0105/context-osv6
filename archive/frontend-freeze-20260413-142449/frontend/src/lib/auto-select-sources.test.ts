import { describe, expect, it } from 'vitest';
import { collectAutoSelectableCompletedSourceIds } from './auto-select-sources';

describe('collectAutoSelectableCompletedSourceIds', () => {
  it('returns completed ids that are pending and not selected', () => {
    const documents = [
      { id: 'doc-1', status: 'completed' as const },
      { id: 'doc-2', status: 'processing' as const },
    ];

    const result = collectAutoSelectableCompletedSourceIds(documents, ['doc-1', 'doc-2'], []);

    expect(result).toEqual(['doc-1']);
  });

  it('skips documents that are already selected', () => {
    const documents = [{ id: 'doc-1', status: 'completed' as const }];

    const result = collectAutoSelectableCompletedSourceIds(documents, ['doc-1'], ['doc-1']);

    expect(result).toEqual([]);
  });

  it('returns empty when no pending ids exist', () => {
    const documents = [{ id: 'doc-1', status: 'completed' as const }];

    const result = collectAutoSelectableCompletedSourceIds(documents, [], []);

    expect(result).toEqual([]);
  });

  it('returns multiple ids in document order', () => {
    const documents = [
      { id: 'doc-2', status: 'completed' as const },
      { id: 'doc-1', status: 'completed' as const },
      { id: 'doc-3', status: 'failed' as const },
    ];

    const result = collectAutoSelectableCompletedSourceIds(documents, ['doc-1', 'doc-2', 'doc-3'], []);

    expect(result).toEqual(['doc-2', 'doc-1']);
  });

  it('accepts raw active status from transitional API responses', () => {
    const documents = [{ id: 'doc-1', status: 'active' as unknown as 'completed' }];

    const result = collectAutoSelectableCompletedSourceIds(documents, ['doc-1'], []);

    expect(result).toEqual(['doc-1']);
  });
});
