import type { Document } from '@/types';
import { normalizeDocumentStatusValue } from './document-status';

type AutoSelectableDocument = Pick<Document, 'id' | 'status'>;

export function collectAutoSelectableCompletedSourceIds(
  documents: AutoSelectableDocument[],
  pendingSourceIds: Iterable<string>,
  selectedSourceIds: Iterable<string>
): string[] {
  const pendingSet = pendingSourceIds instanceof Set ? pendingSourceIds : new Set(pendingSourceIds);
  if (pendingSet.size === 0) {
    return [];
  }

  const selectedSet = selectedSourceIds instanceof Set ? selectedSourceIds : new Set(selectedSourceIds);

  const autoSelectableIds: string[] = [];
  for (const doc of documents) {
    if (normalizeDocumentStatusValue(doc.status) !== 'completed') {
      continue;
    }
    if (!pendingSet.has(doc.id)) {
      continue;
    }
    if (selectedSet.has(doc.id)) {
      continue;
    }
    autoSelectableIds.push(doc.id);
  }

  return autoSelectableIds;
}
