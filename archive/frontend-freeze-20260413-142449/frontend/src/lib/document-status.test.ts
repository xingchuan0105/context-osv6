import { describe, expect, it } from 'vitest';
import {
  applyDocumentStatusEvent,
  normalizeDocumentStatusEvent,
  normalizeDocumentStatusValue,
} from './document-status';
import type { Document } from '@/types';

describe('document-status', () => {
  const baseDoc: Document = {
    id: 'doc-1',
    kb_id: 'kb-1',
    user_id: 'user-1',
    file_name: 'a.pdf',
    status: 'pending',
    chunk_count: 0,
    created_at: '2026-01-01T00:00:00.000Z',
  };

  it('normalizes valid payload', () => {
    const evt = normalizeDocumentStatusEvent({
      seq: 10,
      document_id: 'doc-1',
      kb_id: 'kb-1',
      stage: 'ingest.start',
      status: 'start',
      document_status: 'processing',
      message: 'processing',
      timestamp: 100,
    });

    expect(evt).not.toBeNull();
    expect(evt?.seq).toBe(10);
    expect(evt?.document_id).toBe('doc-1');
  });

  it('ignores invalid payload', () => {
    const evt = normalizeDocumentStatusEvent({ seq: 'x' });
    expect(evt).toBeNull();
  });

  it('normalizes legacy and pipeline-specific status values', () => {
    expect(normalizeDocumentStatusValue('active')).toBe('completed');
    expect(normalizeDocumentStatusValue('indexed')).toBe('completed');
    expect(normalizeDocumentStatusValue('uploaded')).toBe('queued');
    expect(normalizeDocumentStatusValue('embedded')).toBe('processing');
    expect(normalizeDocumentStatusValue('failed_parse')).toBe('failed');
  });

  it('applies newer event to matching document', () => {
    const evt = normalizeDocumentStatusEvent({
      seq: 11,
      document_id: 'doc-1',
      kb_id: 'kb-1',
      stage: 'ingest.done',
      status: 'done',
      document_status: 'completed',
      message: 'done',
      timestamp: 101,
    });
    const out = applyDocumentStatusEvent([baseDoc], evt!);

    expect(out.documents[0].status).toBe('completed');
    expect(out.latestSeq).toBe(11);
    expect(out.runtimeByDoc['doc-1']?.message).toBe('done');
  });

  it('does not regress from older seq', () => {
    const first = normalizeDocumentStatusEvent({
      seq: 20,
      document_id: 'doc-1',
      kb_id: 'kb-1',
      stage: 'ingest.done',
      status: 'done',
      document_status: 'completed',
      message: 'done',
      timestamp: 101,
    });
    const second = normalizeDocumentStatusEvent({
      seq: 10,
      document_id: 'doc-1',
      kb_id: 'kb-1',
      stage: 'ingest.start',
      status: 'start',
      document_status: 'processing',
      message: 'old',
      timestamp: 100,
    });

    const one = applyDocumentStatusEvent([baseDoc], first!);
    const two = applyDocumentStatusEvent(one.documents, second!, one.runtimeByDoc, one.latestSeq);

    expect(two.documents[0].status).toBe('completed');
    expect(two.latestSeq).toBe(20);
    expect(two.runtimeByDoc['doc-1']?.seq).toBe(20);
  });
});
