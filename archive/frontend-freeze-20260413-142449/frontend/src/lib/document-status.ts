import type { Document } from '@/types';

export interface DocumentStatusEvent {
  seq: number;
  id?: string;
  document_id: string;
  kb_id: string;
  user_id?: string;
  stage: string;
  status: 'start' | 'progress' | 'done' | 'error';
  document_status: Document['status'];
  message: string;
  metadata?: Record<string, unknown>;
  timestamp: number;
}

export interface DocumentRuntimeStatus {
  seq: number;
  stage: string;
  status: DocumentStatusEvent['status'];
  message: string;
  timestamp: number;
}

export function normalizeDocumentStatusValue(value: unknown): Document['status'] | null {
  if (typeof value !== 'string') return null;

  const normalized = value.toLowerCase();
  switch (normalized) {
    case 'pending':
    case 'enqueueing':
    case 'queued':
    case 'processing':
    case 'completed':
    case 'failed':
      return normalized;
    case 'active':
    case 'indexed':
      return 'completed';
    case 'uploaded':
      return 'queued';
    case 'parsed':
    case 'chunked':
    case 'embedded':
      return 'processing';
    default:
      if (normalized.startsWith('failed_')) {
        return 'failed';
      }
      return null;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function normalizeDocumentStatusEvent(raw: unknown): DocumentStatusEvent | null {
  if (!isObject(raw)) return null;

  const seq = Number(raw.seq);
  const stage = typeof raw.stage === 'string' ? raw.stage : '';
  const documentId = typeof raw.document_id === 'string' ? raw.document_id : '';
  const kbId = typeof raw.kb_id === 'string' ? raw.kb_id : '';
  const documentStatus = normalizeDocumentStatusValue(raw.document_status);

  if (!Number.isFinite(seq) || seq <= 0 || !stage || !documentId || !kbId || !documentStatus) {
    return null;
  }

  const statusRaw = typeof raw.status === 'string' ? raw.status : 'progress';
  const status: DocumentStatusEvent['status'] =
    statusRaw === 'start' || statusRaw === 'done' || statusRaw === 'error' ? statusRaw : 'progress';

  return {
    seq,
    id: typeof raw.id === 'string' ? raw.id : undefined,
    document_id: documentId,
    kb_id: kbId,
    user_id: typeof raw.user_id === 'string' ? raw.user_id : undefined,
    stage,
    status,
    document_status: documentStatus,
    message: typeof raw.message === 'string' ? raw.message : '',
    metadata: isObject(raw.metadata) ? (raw.metadata as Record<string, unknown>) : undefined,
    timestamp: Number(raw.timestamp || Date.now()),
  };
}

export function applyDocumentStatusEvent(
  documents: Document[],
  event: DocumentStatusEvent,
  runtimeByDoc: Record<string, DocumentRuntimeStatus> = {},
  latestSeq = 0
): {
  documents: Document[];
  runtimeByDoc: Record<string, DocumentRuntimeStatus>;
  latestSeq: number;
} {
  if (!event || !Number.isFinite(event.seq) || event.seq <= 0) {
    return { documents, runtimeByDoc, latestSeq };
  }

  const current = runtimeByDoc[event.document_id];
  if (current && current.seq >= event.seq) {
    return { documents, runtimeByDoc, latestSeq };
  }

  const nextDocuments = documents.map((doc) =>
    doc.id === event.document_id ? { ...doc, status: event.document_status } : doc
  );

  const nextRuntimeByDoc: Record<string, DocumentRuntimeStatus> = {
    ...runtimeByDoc,
    [event.document_id]: {
      seq: event.seq,
      stage: event.stage,
      status: event.status,
      message: event.message,
      timestamp: event.timestamp,
    },
  };

  return {
    documents: nextDocuments,
    runtimeByDoc: nextRuntimeByDoc,
    latestSeq: Math.max(latestSeq, event.seq),
  };
}
