'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { Loader2, FileText, Image as ImageIcon, FileCode, File } from 'lucide-react';
import { documentsApi } from '@/lib/api/client';
import {
  getDocumentPreviewErrorMessage,
  shouldAttemptParsedPreviewFallback,
} from '@/lib/document-preview';
import type { Document } from '@/types';
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';

interface DocumentViewerProps {
  document: Document | null;
  open: boolean;
  onClose: () => void;
}

type ViewerState = 'loading' | 'success' | 'error';
type ParsedPreviewItem = {
  chunk_id: string;
  chunk_index: number;
  text: string;
};

const PAGE_SIZE = 80;

export function DocumentViewer({ document, open, onClose }: DocumentViewerProps) {
  const [state, setState] = useState<ViewerState>('loading');
  const [items, setItems] = useState<ParsedPreviewItem[]>([]);
  const [summary, setSummary] = useState<string>('');
  const [hasMore, setHasMore] = useState<boolean>(false);
  const [loadingMore, setLoadingMore] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const nextCursorRef = useRef<number | null>(null);
  const requestSeqRef = useRef(0);
  const documentId = document?.id ?? '';

  const getFileType = useCallback((mimeType?: string, fileName?: string): string => {
    if (mimeType) {
      if (mimeType.startsWith('image/')) return 'image';
      if (mimeType === 'application/pdf') return 'pdf';
      if (mimeType.includes('markdown') || mimeType === 'text/markdown') return 'markdown';
      if (mimeType.startsWith('text/') || mimeType === 'application/json') return 'text';
    }
    const ext = fileName?.split('.').pop()?.toLowerCase();
    if (ext === 'md' || ext === 'markdown') return 'markdown';
    if (ext === 'pdf') return 'pdf';
    if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg'].includes(ext || '')) return 'image';
    if (['txt', 'json', 'js', 'ts', 'html', 'css', 'xml', 'yaml', 'yml'].includes(ext || '')) return 'text';
    return 'text';
  }, []);

  const inferDisplayType = useCallback((doc: Document): string => {
    if (doc.mime_type && doc.mime_type.trim() !== '') {
      return doc.mime_type;
    }
    const kind = getFileType(doc.mime_type, doc.file_name);
    if (kind === 'pdf') return 'application/pdf (inferred)';
    if (kind === 'image') return 'image/* (inferred)';
    if (kind === 'markdown') return 'text/markdown (inferred)';
    if (kind === 'text') return 'text/plain (inferred)';
    return 'unknown';
  }, [getFileType]);

  const loadPreview = useCallback(
    async (reset = false) => {
      if (!documentId) return;

      const requestSeq = ++requestSeqRef.current;
      const isStale = () => requestSeqRef.current !== requestSeq;

      if (reset) {
        setState('loading');
        setItems([]);
        setSummary('');
        setHasMore(false);
        setError('');
        nextCursorRef.current = 0;
      } else {
        setLoadingMore(true);
      }

      const applyFallbackPreview = async (): Promise<boolean> => {
        const fallback = await documentsApi.getContent(documentId);
        if (isStale()) {
          return true;
        }
        if (!fallback.success || !fallback.data) {
          setError(
            getDocumentPreviewErrorMessage(
              fallback.success ? undefined : { error: fallback.error, error_code: fallback.error_code }
            )
          );
          setState('error');
          return false;
        }

        const fallbackContent = typeof fallback.data.content === 'string' ? fallback.data.content : '';
        const fallbackSummary = typeof fallback.data.summary === 'string' ? fallback.data.summary : '';
        if (!fallbackContent && !fallbackSummary) {
          return false;
        }

        setItems(
          fallbackContent
            ? [{ chunk_id: 'fallback-content', chunk_index: 0, text: fallbackContent }]
            : []
        );
        setSummary(fallbackSummary);
        setHasMore(false);
        nextCursorRef.current = null;
        setState('success');
        return true;
      };

      try {
        const cursor = reset ? 0 : (nextCursorRef.current ?? 0);
        const response = await documentsApi.getParsedPreview(documentId, cursor, PAGE_SIZE);
        if (isStale()) {
          return;
        }

        if (response.success && response.data && Array.isArray(response.data.items)) {
          const data = response.data;
          setItems((prev) => (reset ? data.items : [...prev, ...data.items]));
          setHasMore(Boolean(data.has_more));
          nextCursorRef.current = typeof data.next_cursor === 'number' ? data.next_cursor : null;
          setSummary(typeof data.summary === 'string' ? data.summary : '');
          setState('success');
          return;
        }

        const previewError = getDocumentPreviewErrorMessage(
          response.success ? undefined : { error: response.error, error_code: response.error_code }
        );

        if (
          reset &&
          shouldAttemptParsedPreviewFallback(
            response.success ? undefined : { error: response.error, error_code: response.error_code }
          ) &&
          await applyFallbackPreview()
        ) {
          return;
        }

        setError(previewError);
        setState('error');
      } catch (err) {
        if (isStale()) {
          return;
        }
        if (reset && await applyFallbackPreview()) {
          return;
        }
        setError(err instanceof Error ? err.message : '加载文档时出错');
        setState('error');
      } finally {
        if (!isStale()) {
          setLoadingMore(false);
        }
      }
    },
    [documentId]
  );

  useEffect(() => {
    if (open && documentId) {
      const timer = window.setTimeout(() => {
        void loadPreview(true);
      }, 0);
      return () => {
        requestSeqRef.current += 1;
        window.clearTimeout(timer);
      };
    }
    requestSeqRef.current += 1;
    return undefined;
  }, [documentId, loadPreview, open]);

  if (!document) return null;

  const fileType = getFileType(document.mime_type, document.file_name);

  const renderContent = () => {
    if (state === 'loading') {
      return (
        <div className="flex items-center justify-center h-64">
          <Loader2 className="w-8 h-8 animate-spin text-muted-foreground" />
        </div>
      );
    }

    if (state === 'error') {
      return (
        <div className="flex flex-col items-center justify-center h-64 text-center">
          <FileText className="w-12 h-12 text-muted-foreground/50 mb-4" />
          <p className="text-muted-foreground">{error || '无法预览此文档'}</p>
          <p className="text-sm text-muted-foreground/70 mt-2">
            文件类型: {inferDisplayType(document)}
          </p>
        </div>
      );
    }

    const content = items.map((item) => item.text).join('\n\n').trim();

    return (
      <div className="space-y-3">
        <pre className="whitespace-pre-wrap font-mono text-sm overflow-auto max-h-[60vh] bg-muted/30 p-4 rounded">
          {content || summary || '暂无解析文本'}
        </pre>
        {hasMore && (
          <div className="flex justify-center">
            <button
              type="button"
              className="px-3 py-1.5 text-xs border border-border rounded bg-background hover:bg-muted disabled:opacity-60"
              onClick={() => void loadPreview(false)}
              disabled={loadingMore}
            >
              {loadingMore ? '加载中...' : '加载更多'}
            </button>
          </div>
        )}
      </div>
    );
  };

  return (
    <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent className="max-w-3xl w-full max-h-[85vh] overflow-hidden flex flex-col">
        <DialogTitle className="sr-only">{document.file_name}</DialogTitle>
        <div className="flex items-center gap-3 pb-4 border-b">
          {fileType === 'pdf' && <FileCode className="w-5 h-5 text-red-400" />}
          {fileType === 'image' && <ImageIcon className="w-5 h-5 text-green-400" />}
          {fileType === 'markdown' && <FileText className="w-5 h-5 text-blue-400" />}
          {fileType === 'text' && <File className="w-5 h-5 text-muted-foreground" />}
          <div className="flex-1 min-w-0">
            <h3 className="font-medium truncate">{document.file_name}</h3>
            <p className="text-xs text-muted-foreground">
              {inferDisplayType(document)} • {document.chunk_count} chunks
            </p>
          </div>
        </div>
        
        <div className="flex-1 min-h-0 overflow-auto py-4">
          {renderContent()}
        </div>
      </DialogContent>
    </Dialog>
  );
}
