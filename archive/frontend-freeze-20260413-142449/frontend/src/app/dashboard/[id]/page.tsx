'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useParams, useRouter } from 'next/navigation';
import { useTranslation } from 'react-i18next';
import {
  Plus,
  FileText,
  Trash2,
  Upload,
  Loader2,
  FolderOpen,
  AlertCircle,
  CheckCircle2,
  Clock3,
  X,
  Home,
  PanelLeftClose,
  PanelLeft,
  BookOpen,
  Edit3,
  MoreVertical,
  Pencil,
} from 'lucide-react';
import { kbApi, documentsApi, notesApi } from '@/lib/api/client';
import {
  type DocumentRuntimeStatus,
  normalizeDocumentStatusValue,
} from '@/lib/document-status';
import { collectAutoSelectableCompletedSourceIds } from '@/lib/auto-select-sources';
import { partitionSupportedUploadFiles } from '@/lib/upload-file-validation';
import {
  filterSelectedSourceIdsByDocumentIds,
  parsePersistedSelectedSourceIds,
  stringifySelectedSourceIds,
} from '@/lib/selected-source-persistence';
import { useAppStore } from '@/stores/useAppStore';
import { ChatPanel } from '@/components/chat/chat-panel';
import { AddSourceModal } from '@/components/dashboard/add-source-modal';
import { CreateNoteModal } from '@/components/dashboard/create-note-modal';
import { DocumentViewer } from '@/components/document/document-viewer';
import { toast } from '@/components/ui/toaster';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { KnowledgeBase, Document, Note } from '@/types';

const LEFT_MIN_WIDTH = 15;
const LEFT_MAX_WIDTH = 55;
const DEFAULT_LEFT_WIDTH = 30;
const COLLAPSED_RAIL_PX = 72;
const NOTE_CLICK_GUARD_MS = 1500;

function normalizeNoteText(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }

  if (value && typeof value === 'object') {
    const candidate = value as { String?: unknown; Valid?: unknown };
    if (candidate.Valid === true && typeof candidate.String === 'string') {
      return candidate.String;
    }
  }

  return '';
}

function normalizeNoteForUI(note: Note): Note {
  return {
    ...note,
    title: normalizeNoteText(note.title),
    content: typeof note.content === 'string' ? note.content : '',
  };
}

export default function WorkspacePage() {
  const params = useParams();
  const router = useRouter();
  const { t, i18n } = useTranslation();
  const { setCurrentWorkspace } = useAppStore();

  const [kb, setKb] = useState<KnowledgeBase | null>(null);
  const [documents, setDocuments] = useState<Document[]>([]);
  const [documentsLoaded, setDocumentsLoaded] = useState(false);
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);
  const [leftWidth, setLeftWidth] = useState(DEFAULT_LEFT_WIDTH);
  const [lastExpandedWidth, setLastExpandedWidth] = useState(DEFAULT_LEFT_WIDTH);
  const [isDragging, setIsDragging] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState<Record<string, { fileName: string; progress: number }>>({});
  const [runtimeStatusByDoc, setRuntimeStatusByDoc] = useState<Record<string, DocumentRuntimeStatus>>({});
  const [selectedSourceIdsReady, setSelectedSourceIdsReady] = useState(false);
  const [activeTab, setActiveTab] = useState<'docs' | 'notes'>('docs');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [addSourceOpen, setAddSourceOpen] = useState(false);
  const [createNoteOpen, setCreateNoteOpen] = useState(false);
  const [noteModalMode, setNoteModalMode] = useState<'create' | 'edit'>('create');
  const [selectedSourceIds, setSelectedSourceIds] = useState<string[]>([]);
  const [previewDocument, setPreviewDocument] = useState<Document | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [importingNoteId, setImportingNoteId] = useState<string | null>(null);
  const [renameOpen, setRenameOpen] = useState(false);
  const [renameKind, setRenameKind] = useState<'doc' | 'note'>('doc');
  const [renameTargetId, setRenameTargetId] = useState<string>('');
  const [renameValue, setRenameValue] = useState<string>('');
  const [renaming, setRenaming] = useState(false);
  const [editingNote, setEditingNote] = useState<Note | null>(null);
  const [noteModalInitialTitle, setNoteModalInitialTitle] = useState('');
  const [noteModalInitialContent, setNoteModalInitialContent] = useState('');
  const [deletingNoteId, setDeletingNoteId] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const uploadSequenceRef = useRef(0);
  const uploadedFilesByDocumentRef = useRef<Map<string, File>>(new Map());
  const pendingAutoSelectSourceIdsRef = useRef<Set<string>>(new Set());
  const hasPersistedSelectedSourceIdsRef = useRef(false);
  const runtimeStatusRef = useRef<Record<string, DocumentRuntimeStatus>>({});
  const documentsRef = useRef<Document[]>([]);
  const latestStatusSeqRef = useRef(0);
  const documentRefreshTimerRef = useRef<number | null>(null);
  const suppressNoteCardClickUntilRef = useRef(0);
  const armNoteCardClickGuard = useCallback((ms = NOTE_CLICK_GUARD_MS) => {
    const next = Date.now() + ms;
    suppressNoteCardClickUntilRef.current = Math.max(suppressNoteCardClickUntilRef.current, next);
  }, []);

  const kbId = params.id as string;
  const locale = i18n.resolvedLanguage?.startsWith('en') ? 'en-US' : 'zh-CN';
  const selectedSourceIdsStorageKey = useMemo(() => `kb:${kbId}:selected-source-ids`, [kbId]);

  useEffect(() => {
    setSelectedSourceIdsReady(false);

    let restoredSelection: string[] = [];
    const persistedRaw = window.localStorage.getItem(selectedSourceIdsStorageKey);
    hasPersistedSelectedSourceIdsRef.current = persistedRaw !== null;

    try {
      restoredSelection = parsePersistedSelectedSourceIds(persistedRaw);
    } catch {
      restoredSelection = [];
    }

    setSelectedSourceIds(restoredSelection);
    setSelectedSourceIdsReady(true);
    setDocumentsLoaded(false);
    setPreviewDocument(null);
    setPreviewOpen(false);
    pendingAutoSelectSourceIdsRef.current = new Set();
    runtimeStatusRef.current = {};
    documentsRef.current = [];
    latestStatusSeqRef.current = 0;
    setRuntimeStatusByDoc({});
  }, [kbId, selectedSourceIdsStorageKey]);

  useEffect(() => {
    documentsRef.current = documents;
  }, [documents]);

  useEffect(() => {
    if (!selectedSourceIdsReady) {
      return;
    }

    try {
      const serializedSelection = stringifySelectedSourceIds(selectedSourceIds);
      window.localStorage.setItem(selectedSourceIdsStorageKey, serializedSelection || '[]');
    } catch (error) {
      console.warn('Failed to persist selected source ids', error);
    }
  }, [selectedSourceIds, selectedSourceIdsReady, selectedSourceIdsStorageKey]);

  useEffect(() => {
    if (!documentsLoaded) {
      return;
    }

    setSelectedSourceIds((prev) => {
      const filtered = filterSelectedSourceIdsByDocumentIds(prev, documents);

      if (!hasPersistedSelectedSourceIdsRef.current && filtered.length === 0 && documents.length > 0) {
        hasPersistedSelectedSourceIdsRef.current = true;
        return documents.map((doc) => doc.id);
      }

      if (filtered.length === prev.length && filtered.every((id, index) => id === prev[index])) {
        return prev;
      }
      return filtered;
    });
  }, [documents, documentsLoaded]);

  useEffect(() => {
    if (!previewDocument) {
      return;
    }

    const latestDocument = documents.find((item) => item.id === previewDocument.id);
    if (!latestDocument) {
      setPreviewOpen(false);
      setPreviewDocument(null);
      return;
    }

    if (latestDocument !== previewDocument) {
      setPreviewDocument(latestDocument);
    }
  }, [documents, previewDocument]);

  useEffect(() => {
    const pendingSourceIds = pendingAutoSelectSourceIdsRef.current;
    if (pendingSourceIds.size === 0 || documents.length === 0) {
      return;
    }

    setSelectedSourceIds((prev) => {
      const autoSelectableIds = collectAutoSelectableCompletedSourceIds(documents, pendingSourceIds, prev);
      if (autoSelectableIds.length === 0) {
        return prev;
      }

      for (const id of autoSelectableIds) {
        pendingSourceIds.delete(id);
      }

      return [...prev, ...autoSelectableIds];
    });
  }, [documents]);

  const visibleNotes = useMemo(() => notes, [notes]);
  const activeUploads = useMemo(() => Object.entries(uploadProgress), [uploadProgress]);

  const tryAutoSelectCompletedSource = useCallback((documentId: string, status: Document['status']) => {
    if (!isReadyDocumentStatus(status)) {
      return;
    }

    const pendingSourceIds = pendingAutoSelectSourceIdsRef.current;
    if (!pendingSourceIds.has(documentId)) {
      return;
    }

    pendingSourceIds.delete(documentId);
    setSelectedSourceIds((prev) => (prev.includes(documentId) ? prev : [...prev, documentId]));
  }, []);

  const registerPendingAutoSelectSource = useCallback(
    (documentId: string | undefined, status: unknown) => {
      if (!documentId) {
        return;
      }

      pendingAutoSelectSourceIdsRef.current.add(documentId);
      if (normalizeDocumentStatusValue(status) === 'completed') {
        tryAutoSelectCompletedSource(documentId, 'completed');
      }
    },
    [tryAutoSelectCompletedSource]
  );

  const toggleSelectedSource = useCallback((id: string) => {
    setSelectedSourceIds((prev) => (prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]));
  }, []);

  const openDocumentPreview = useCallback((doc: Document) => {
    if (!isPreviewableDocumentStatus(doc.status)) {
      return;
    }

    setPreviewDocument(doc);
    setPreviewOpen(true);
  }, []);

  const openRename = useCallback((kind: 'doc' | 'note', id: string, currentName: string) => {
    setRenameKind(kind);
    setRenameTargetId(id);
    setRenameValue(currentName);
    setRenameOpen(true);
  }, []);

  const confirmRename = useCallback(async () => {
    const nextName = renameValue.trim();
    if (!nextName) {
      toast.error(t('validation.required'));
      return;
    }

    setRenaming(true);
    try {
      if (renameKind === 'doc') {
        const response = await documentsApi.update(renameTargetId, { file_name: nextName, kb_id: kbId });
        if (!response?.success) {
          throw new Error(response?.error || 'rename-failed');
        }
        setDocuments((prev) =>
          prev.map((doc) => (doc.id === renameTargetId ? { ...doc, file_name: nextName } : doc))
        );
        toast.success(t('document.renameSuccess'));
      } else {
        const response = await notesApi.update(renameTargetId, { title: nextName });
        if (!response?.success) {
          throw new Error(response?.error || 'rename-failed');
        }
        setNotes((prev) => prev.map((note) => (note.id === renameTargetId ? { ...note, title: nextName } : note)));
        toast.success(t('note.renameSuccess'));
      }
      setRenameOpen(false);
    } catch {
      toast.error(renameKind === 'doc' ? t('document.renameFailed') : t('note.renameFailed'));
    } finally {
      setRenaming(false);
    }
  }, [kbId, renameKind, renameTargetId, renameValue, t]);

  const loadWorkspace = useCallback(async () => {
    try {
      setLoading(true);
      const [kbRes, docsRes, notesRes] = await Promise.all([
        kbApi.get(kbId),
        documentsApi.list(kbId),
        notesApi.list(kbId),
      ]);

      if (kbRes.success && kbRes.data) {
        setKb(kbRes.data);
        setCurrentWorkspace(kbRes.data);
      } else {
        setKb(null);
      }

      if (docsRes.success) {
        setDocuments(docsRes.data || []);
        setDocumentsLoaded(true);
      } else {
        setDocuments([]);
        setDocumentsLoaded(false);
        toast.error(docsRes.error || t('document.refreshFailed'));
      }

      if (notesRes.success && notesRes.data) {
        setNotes(notesRes.data.map((item: Note) => normalizeNoteForUI(item)));
      } else {
        setNotes([]);
      }
    } catch {
      toast.error(t('workspace.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [kbId, setCurrentWorkspace, t]);

  useEffect(() => {
    if (!kbId) return;
    void loadWorkspace();
  }, [kbId, loadWorkspace]);

  const loadDocuments = useCallback(async () => {
    try {
      const response = await documentsApi.list(kbId);
      if (response.success) {
        setDocuments(response.data || []);
        setDocumentsLoaded(true);
      } else {
        toast.error(response.error || t('document.refreshFailed'));
      }
    } catch {
      toast.error(t('document.refreshFailed'));
    }
  }, [kbId, t]);

  const scheduleDocumentRefreshWindow = useCallback(
    (durationMs = 45000) => {
      if (typeof window === 'undefined') {
        return;
      }

      if (documentRefreshTimerRef.current !== null) {
        window.clearTimeout(documentRefreshTimerRef.current);
        documentRefreshTimerRef.current = null;
      }

      const deadline = Date.now() + durationMs;
      const tick = async () => {
        await loadDocuments();
        const hasPendingDocuments = documentsRef.current.some((doc) => !isTerminalDocumentStatus(doc.status));
        if (hasPendingDocuments && Date.now() < deadline) {
          documentRefreshTimerRef.current = window.setTimeout(() => {
            void tick();
          }, 2500);
          return;
        }
        documentRefreshTimerRef.current = null;
      };

      void tick();
    },
    [loadDocuments]
  );

  useEffect(() => {
    return () => {
      if (documentRefreshTimerRef.current !== null) {
        window.clearTimeout(documentRefreshTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!kbId || !documentsLoaded) return;

    const hasPendingDocuments = documents.some((doc) => !isTerminalDocumentStatus(doc.status));
    if (!hasPendingDocuments) {
      return;
    }

    let stopped = false;

    const syncDocumentStatuses = async () => {
      const response = await documentsApi.list(kbId);
      if (!response.success || !response.data) {
        setRuntimeStatusByDoc((prev) => {
          const next = { ...prev };
          for (const doc of documentsRef.current) {
            if (!isTerminalDocumentStatus(doc.status)) {
              next[doc.id] = buildRuntimeStatusEntry(
                latestStatusSeqRef,
                'status.poll',
                'error',
                '文档状态同步失败，正在重试'
              );
            }
          }
          runtimeStatusRef.current = next;
          return next;
        });
        return;
      }

      const previousByID = new Map(documentsRef.current.map((doc) => [doc.id, doc]));
      const nextDocuments = response.data;
      documentsRef.current = nextDocuments;
      setDocuments(nextDocuments);
      setDocumentsLoaded(true);

      setRuntimeStatusByDoc((prev) => {
        const next = { ...prev };
        for (const doc of nextDocuments) {
          const previous = previousByID.get(doc.id);
          if (!previous || previous.status !== doc.status) {
            next[doc.id] = buildRuntimeStatusEntry(
              latestStatusSeqRef,
              'status.poll',
              runtimeEventStatusForDocument(doc.status),
              runtimeMessageForDocumentStatus(doc.status)
            );
          } else if (!isTerminalDocumentStatus(doc.status) && !next[doc.id]) {
            next[doc.id] = buildRuntimeStatusEntry(
              latestStatusSeqRef,
              'status.poll',
              'progress',
              runtimeMessageForDocumentStatus(doc.status)
            );
          }
          tryAutoSelectCompletedSource(doc.id, doc.status);
        }
        runtimeStatusRef.current = next;
        return next;
      });
    };

    void syncDocumentStatuses();
    const timer = window.setInterval(() => {
      if (!stopped) {
        void syncDocumentStatuses();
      }
    }, 2500);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [documentsLoaded, kbId, tryAutoSelectCompletedSource]);

  const handleMouseDown = useCallback(() => {
    setIsDragging(true);
  }, []);

  const handleMouseMove = useCallback(
    (event: MouseEvent) => {
      if (!isDragging || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const newLeftWidthPx = event.clientX - rect.left;
      const newLeftWidth = (newLeftWidthPx / rect.width) * 100;

      // When collapsed, dragging beyond threshold expands the sidebar
      if (sidebarCollapsed) {
        if (newLeftWidthPx > COLLAPSED_RAIL_PX + 10) {
          setSidebarCollapsed(false);
          const nextWidth = Math.min(LEFT_MAX_WIDTH, Math.max(LEFT_MIN_WIDTH, newLeftWidth));
          setLeftWidth(nextWidth);
          setLastExpandedWidth(nextWidth);
        }
        return;
      }

      // Normal dragging behavior
      const nextWidth = Math.min(LEFT_MAX_WIDTH, Math.max(LEFT_MIN_WIDTH, newLeftWidth));
      setLeftWidth(nextWidth);

      // Auto-collapse when dragged below threshold
      if (newLeftWidthPx <= COLLAPSED_RAIL_PX) {
        setSidebarCollapsed(true);
        setLastExpandedWidth(Math.max(nextWidth, DEFAULT_LEFT_WIDTH));
      } else {
        setLastExpandedWidth(nextWidth);
      }
    },
    [isDragging, sidebarCollapsed]
  );

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  useEffect(() => {
    if (!isDragging) return;

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [handleMouseMove, handleMouseUp, isDragging]);

  const uploadSingleFile = useCallback(
    async (file: File) => {
      const uploadKey = `${file.name}-${file.lastModified}-${file.size}-${uploadSequenceRef.current++}`;
      setUploadProgress((prev) => ({
        ...prev,
        [uploadKey]: {
          fileName: file.name,
          progress: 0,
        },
      }));

      try {
        const response = await documentsApi.upload(kbId, file, (progress) => {
          setUploadProgress((prev) => {
            const entry = prev[uploadKey];
            if (!entry) return prev;
            return {
              ...prev,
              [uploadKey]: {
                ...entry,
                progress,
              },
            };
          });
        });

        if (!response.success) {
          toast.error(t('document.uploadFailed', { file: file.name }));
          return false;
        }

        const documentId = response?.data?.id as string | undefined;
        registerPendingAutoSelectSource(documentId, response?.data?.status);
        if (documentId) {
          uploadedFilesByDocumentRef.current.set(documentId, file);
          setSelectedSourceIds((prev) => (prev.includes(documentId) ? prev : [...prev, documentId]));
        }
        return true;
      } finally {
        setUploadProgress((prev) => {
          if (!prev[uploadKey]) return prev;
          const next = { ...prev };
          delete next[uploadKey];
          return next;
        });
      }
    },
    [kbId, registerPendingAutoSelectSource, t]
  );

  const uploadFiles = useCallback(
    async (files: File[]) => {
      if (!files.length) return;

      const { supported, unsupported } = partitionSupportedUploadFiles(files);
      if (unsupported.length > 0) {
        const unsupportedNames = unsupported.map((file) => file.name).join(', ');
        toast.error(t('document.unsupportedFileType', { files: unsupportedNames }));
      }
      if (supported.length === 0) {
        return;
      }

      setUploading(true);
      let successCount = 0;

      try {
        for (const file of supported) {
          try {
            const success = await uploadSingleFile(file);
            if (success) successCount += 1;
          } catch {
            toast.error(t('document.uploadFailed', { file: file.name }));
          }
        }

        if (successCount > 0) {
          await loadDocuments();
          scheduleDocumentRefreshWindow();
          if (successCount === supported.length) {
            toast.success(t('document.submitted', { count: supported.length }));
          } else {
            toast.info(t('document.submittedPartial', { success: successCount, total: supported.length }));
          }
        }
      } catch {
        toast.error(t('document.uploadFailedGeneric'));
      } finally {
        setUploading(false);
      }
    },
    [loadDocuments, scheduleDocumentRefreshWindow, t, uploadSingleFile]
  );

  const handleDrop = async (event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setIsDragOver(false);
    const files = Array.from(event.dataTransfer.files || []);
    await uploadFiles(files);
  };

  const handleDeleteDoc = async (docId: string) => {
    if (!window.confirm(t('document.deleteConfirm'))) return;

    try {
      const response = await documentsApi.delete(docId, kbId);
      if (!response?.success) {
        throw new Error(response?.error || 'delete-document-failed');
      }
      setDocuments((prev) => prev.filter((doc) => doc.id !== docId));
      setSelectedSourceIds((prev) => prev.filter((id) => id !== docId));
      uploadedFilesByDocumentRef.current.delete(docId);
      pendingAutoSelectSourceIdsRef.current.delete(docId);
      toast.success(t('document.deleted'));
    } catch {
      toast.error(t('document.deleteFailed'));
    }
  };

  const retryFailedDocument = async (doc: Document) => {
    const file = uploadedFilesByDocumentRef.current.get(doc.id);
    if (!file) {
      toast.error(t('document.retryMissing'));
      return;
    }

    setUploading(true);
    try {
      const success = await uploadSingleFile(file);
      if (!success) return;
      uploadedFilesByDocumentRef.current.delete(doc.id);
      await loadDocuments();
      toast.success(t('document.retried', { file: file.name }));
    } catch {
      toast.error(t('document.retryFailed'));
    } finally {
      setUploading(false);
    }
  };

  const handleNoteCreated = (note: Note) => {
    const normalizedNote = normalizeNoteForUI(note);
    setNotes((prev) => {
      const idx = prev.findIndex((item) => item.id === normalizedNote.id);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = normalizedNote;
        return next;
      }
      return [normalizedNote, ...prev];
    });
    setActiveTab('notes');
    toast.success(t('note.saved'));
  };

  const deleteNote = async (noteId: string) => {
    if (deletingNoteId) {
      return;
    }

    armNoteCardClickGuard();
    setDeletingNoteId(noteId);
    if (!window.confirm(t('note.deleteConfirm'))) {
      armNoteCardClickGuard();
      setDeletingNoteId((prev) => (prev === noteId ? null : prev));
      return;
    }

    try {
      armNoteCardClickGuard();
      await notesApi.delete(noteId);
      setNotes((prev) => prev.filter((note) => note.id !== noteId));
      let deletedEditingNote = false;
      setEditingNote((prev) => {
        if (prev?.id === noteId) {
          deletedEditingNote = true;
          return null;
        }
        return prev;
      });
      if (deletedEditingNote) {
        setCreateNoteOpen(false);
        setNoteModalMode('create');
        setNoteModalInitialTitle('');
        setNoteModalInitialContent('');
      }
      toast.success(t('note.deleted'));
    } catch {
      toast.error(t('note.deleteFailed'));
    } finally {
      armNoteCardClickGuard();
      setDeletingNoteId((prev) => (prev === noteId ? null : prev));
    }
  };

  const importNoteToKnowledgeBase = async (note: Note) => {
    const content = (note.content || '').trim();
    if (!content) {
      toast.error(t('note.importEmpty'));
      return;
    }

    setImportingNoteId(note.id);
    try {
      const fileBaseName = (normalizeNoteText(note.title) || t('note.untitled') || 'note')
        .replace(/[\\/:*?"<>|\r\n]+/g, '_')
        .slice(0, 64)
        .trim();
      const fileName = `${fileBaseName || 'note'}.md`;
      const file = new File([content], fileName, { type: 'text/markdown;charset=utf-8' });

      const response = await documentsApi.upload(kbId, file);
      if (!response?.success) {
        throw new Error(response?.error || 'import-note-failed');
      }

      const documentId = response?.data?.id as string | undefined;
      registerPendingAutoSelectSource(documentId, response?.data?.status);

      await loadDocuments();
      scheduleDocumentRefreshWindow();
      setActiveTab('docs');
      toast.success(t('note.importSuccess'));
    } catch {
      toast.error(t('note.importFailed'));
    } finally {
      setImportingNoteId(null);
    }
  };

  const openCreateNoteModal = () => {
    setNoteModalMode('create');
    setEditingNote(null);
    setNoteModalInitialTitle('');
    setNoteModalInitialContent('');
    setCreateNoteOpen(true);
  };

  const openEditNoteModal = (note: Note) => {
    setNoteModalMode('edit');
    const normalizedNote = normalizeNoteForUI(note);
    setEditingNote(normalizedNote);
    setNoteModalInitialTitle(normalizedNote.title || '');
    setNoteModalInitialContent(normalizedNote.content || '');
    setCreateNoteOpen(true);
  };

  const openNoteFromChatExtract = (content: string) => {
    setNoteModalMode('create');
    setEditingNote(null);
    setNoteModalInitialTitle(`${t('chat.extractTitle')} ${new Date().toLocaleString(locale)}`);
    setNoteModalInitialContent(content);
    setCreateNoteOpen(true);
    setActiveTab('notes');
  };

  const handleNoteModalOpenChange = (open: boolean) => {
    setCreateNoteOpen(open);
    if (!open) {
      setNoteModalMode('create');
      setEditingNote(null);
      setNoteModalInitialTitle('');
      setNoteModalInitialContent('');
    }
  };

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <Loader2 className="w-8 h-8 text-primary animate-spin" />
      </div>
    );
  }

  if (!kb) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center text-muted-foreground">
          <FolderOpen className="w-12 h-12 mx-auto mb-4" />
          <p>{t('workspace.notFound')}</p>
          <button
            onClick={() => router.push('/dashboard')}
            className="mt-4 text-primary hover:opacity-80"
          >
            {t('workspace.backToList')}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="relative h-full p-2 md:p-3 flex gap-2 bg-background text-foreground overflow-hidden">
      {/* Mobile header */}
      <div className="md:hidden absolute top-4 left-4 z-20 flex gap-2">
        <button
          onClick={() => router.push('/dashboard')}
          className="p-2 rounded-lg bg-card border border-border hover:bg-accent"
          aria-label={t('dashboard.backToHome')}
        >
          <Home className="w-5 h-5" />
        </button>
        <button
          onClick={() => setSidebarOpen(true)}
          className="p-2 rounded-lg bg-card border border-border hover:bg-accent"
          aria-label={t('dashboard.openSidebar')}
        >
          <FolderOpen className="w-5 h-5" />
        </button>
      </div>

      {/* Left Panel - Unified Document + Notes Management (30%) */}
      <div
        data-testid="left-panel"
        data-collapsed={sidebarCollapsed}
        className={`h-full overflow-hidden flex flex-col rounded-xl border m-2 bg-card bg-card/95 dark:bg-card/95 border-border/80 transition-all duration-300 ${
          sidebarOpen
            ? 'fixed md:relative inset-0 z-30 md:z-auto w-full md:w-auto'
            : 'hidden md:flex'
        }`}
        style={{
          width: sidebarCollapsed ? `${COLLAPSED_RAIL_PX}px` : (sidebarOpen ? undefined : `${leftWidth}%`),
        }}
      >
        {sidebarCollapsed ? (
          <div data-testid="collapsed-rail" className="hidden md:flex h-full flex-col items-center justify-start py-3 gap-2">
            <button
              onClick={() => {
                setSidebarCollapsed(false);
                setLeftWidth(Math.max(lastExpandedWidth, DEFAULT_LEFT_WIDTH));
              }}
              className="w-10 h-10 rounded-xl border border-border bg-card hover:bg-accent text-muted-foreground hover:text-foreground flex items-center justify-center"
              title={t('dashboard.expandSidebar')}
              aria-label={t('dashboard.expandSidebar')}
            >
              <PanelLeft className="w-4 h-4" />
            </button>
            <button
              onClick={() => setAddSourceOpen(true)}
              className="w-10 h-10 rounded-xl border border-border bg-card hover:bg-accent text-muted-foreground hover:text-foreground flex items-center justify-center"
              title={t('dashboard.addSource')}
              aria-label={t('dashboard.addSource')}
            >
              <Upload className="w-4 h-4" />
            </button>
            <button
              onClick={() => openCreateNoteModal()}
              className="w-10 h-10 rounded-xl border border-border bg-card hover:bg-accent text-muted-foreground hover:text-foreground flex items-center justify-center"
              title={t('dashboard.createNote')}
              aria-label={t('dashboard.createNote')}
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
        ) : (
          <>
        {/* Mobile header */}
        <div className="md:hidden shrink-0 p-4 border-b border-border flex items-center justify-between gap-3 bg-card">
          <button
            onClick={() => setSidebarOpen(false)}
            className="p-2 -ml-2 rounded-lg hover:bg-accent min-w-[44px] min-h-[44px] flex items-center justify-center"
          >
            <X className="w-5 h-5" />
          </button>
          <h2 className="text-lg font-semibold">
            {activeTab === 'docs' ? t('dashboard.documents') : t('dashboard.notes')}
          </h2>
          <div className="w-10" />
        </div>

        {/* Tab Switcher + Action Buttons */}
        <div className="shrink-0 p-3 md:p-4 border-b border-border space-y-2.5">
          <div className="hidden md:flex items-center justify-start">
            <button
              onClick={() => {
                setLastExpandedWidth(leftWidth);
                setSidebarCollapsed(!sidebarCollapsed);
              }}
              className="p-1.5 rounded hover:bg-accent text-muted-foreground"
              title={t('dashboard.toggleSidebar')}
              aria-label={t('dashboard.toggleSidebar')}
            >
              {sidebarCollapsed ? <PanelLeft className="w-4 h-4" /> : <PanelLeftClose className="w-4 h-4" />}
            </button>
          </div>

          <div className="grid grid-cols-2 gap-2">
            <button
              onClick={() => setAddSourceOpen(true)}
              disabled={uploading}
              className="h-10 w-full inline-flex items-center justify-center gap-1.5 rounded-lg bg-primary hover:opacity-90 text-primary-foreground text-sm font-medium transition-colors disabled:opacity-60"
              title={t('dashboard.addSource')}
            >
              {uploading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Upload className="w-4 h-4" />}
              <span className="truncate">{t('document.addSource')}</span>
            </button>

            <button
              onClick={() => openCreateNoteModal()}
              className="h-10 w-full inline-flex items-center justify-center gap-1.5 rounded-lg border border-border hover:bg-accent text-sm font-medium transition-colors"
              title={t('dashboard.createNote')}
            >
              <Plus className="w-4 h-4" />
              <span className="truncate">{t('note.buttonLabel')}</span>
            </button>
          </div>

          {/* Tabs */}
          <div className="grid grid-cols-2 gap-2">
            <button
              onClick={() => setActiveTab('docs')}
              className={`h-10 w-full inline-flex items-center justify-center gap-1.5 rounded-lg border text-sm font-medium transition-colors ${
                activeTab === 'docs'
                  ? 'bg-card border-border text-foreground shadow-sm'
                  : 'border-border/60 text-muted-foreground hover:text-foreground hover:bg-accent/40'
              }`}
            >
              <BookOpen className="w-4 h-4" />
              <span className="truncate">{t('dashboard.sourceList')}</span>
              <span className="text-xs opacity-70">({documents.length})</span>
            </button>
            <button
              onClick={() => setActiveTab('notes')}
              className={`h-10 w-full inline-flex items-center justify-center gap-1.5 rounded-lg border text-sm font-medium transition-colors ${
                activeTab === 'notes'
                  ? 'bg-card border-border text-foreground shadow-sm'
                  : 'border-border/60 text-muted-foreground hover:text-foreground hover:bg-accent/40'
              }`}
            >
              <Edit3 className="w-4 h-4" />
              <span className="truncate">{t('dashboard.noteList')}</span>
              <span className="text-xs opacity-70">({visibleNotes.length})</span>
            </button>
          </div>
        </div>

        {/* Content Area */}
        <div
          className={`flex-1 overflow-auto p-3 md:p-4 transition-colors ${
            isDragOver ? 'bg-primary/10' : 'bg-background'
          }`}
          onDragOver={(event) => {
            event.preventDefault();
            setIsDragOver(true);
          }}
          onDragLeave={() => setIsDragOver(false)}
          onDrop={handleDrop}
        >
          {activeTab === 'docs' ? (
            /* Documents Tab */
            <>
              {documents.length === 0 && activeUploads.length === 0 ? (
                <div className="h-full flex items-center justify-center text-muted-foreground border border-dashed border-border rounded-xl">
                  <div className="text-center p-8">
                    <Upload className="w-12 h-12 mx-auto mb-4 opacity-60" />
                    <p>{t('document.dragOrUpload')}</p>
                    <p className="text-sm mt-2 hidden sm:block">{t('document.statusHint')}</p>
                  </div>
                </div>
              ) : (
                <div className="space-y-2">
                  {/* Upload Progress */}
                  {activeUploads.map(([uploadKey, item]) => (
                    <div key={uploadKey} className="p-3 rounded-lg bg-card border border-border">
                      <div className="flex items-center gap-3">
                        <Upload className="w-5 h-5 text-primary shrink-0" />
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center justify-between gap-3">
                            <div className="text-sm font-medium truncate">{item.fileName}</div>
                            <span className="text-xs text-muted-foreground tabular-nums">{item.progress}%</span>
                          </div>
                          <div className="mt-2 h-1.5 rounded-full bg-border overflow-hidden">
                            <div
                              className="h-full rounded-full bg-primary transition-all duration-200"
                              style={{ width: `${item.progress}%` }}
                            />
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}

                  {/* Document List */}
                  {documents.map((doc) => (
                    <div
                      key={doc.id}
                      className={`flex items-center gap-3 p-3 rounded-lg bg-card border border-border transition-colors ${
                        isPreviewableDocumentStatus(doc.status) ? 'hover:border-border/80 cursor-pointer' : ''
                      }`}
                      onClick={() => openDocumentPreview(doc)}
                    >
                      <input
                        type="checkbox"
                        checked={selectedSourceIds.includes(doc.id)}
                        onChange={() => toggleSelectedSource(doc.id)}
                        onClick={(event) => event.stopPropagation()}
                        className="h-4 w-4 rounded border-border accent-primary"
                        aria-label={t('common.select')}
                      />
                      <FileText className="w-5 h-5 text-indigo-400 shrink-0" />
                      <div className="flex-1 min-w-0">
                        <div className="text-sm font-medium truncate">{doc.file_name}</div>
                        <div className="mt-1">{renderStatusBadge(doc.status, t)}</div>
                        {runtimeStatusByDoc[doc.id]?.message ? (
                          <div className="mt-1 text-[11px] text-muted-foreground truncate">
                            {runtimeStatusByDoc[doc.id]?.message}
                          </div>
                        ) : null}
                      </div>

                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <button
                            type="button"
                            onClick={(event) => event.stopPropagation()}
                            className="p-2 rounded-lg hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                            aria-label={t('common.edit')}
                          >
                            <MoreVertical className="w-4 h-4" />
                          </button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            onSelect={(e) => {
                              e.preventDefault();
                              openRename('doc', doc.id, doc.file_name);
                            }}
                          >
                            <Pencil className="w-4 h-4 mr-2" />
                            {t('document.rename')}
                          </DropdownMenuItem>
                          {doc.status === 'failed' && (
                            <DropdownMenuItem
                              onSelect={(e) => {
                                e.preventDefault();
                                void retryFailedDocument(doc);
                              }}
                              disabled={uploading}
                            >
                              {t('document.retry')}
                            </DropdownMenuItem>
                          )}
                          <DropdownMenuSeparator />
                          <DropdownMenuItem
                            onSelect={(e) => {
                              e.preventDefault();
                              void handleDeleteDoc(doc.id);
                            }}
                            className="text-red-500 focus:text-red-500"
                          >
                            <Trash2 className="w-4 h-4 mr-2" />
                            {t('common.delete')}
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  ))}
                </div>
              )}
            </>
          ) : (
            /* Notes Tab */
            <>
              {visibleNotes.length === 0 ? (
                <div className="h-48 flex items-center justify-center text-muted-foreground text-sm border border-dashed border-border rounded-xl">
                  {t('note.noNotes')}
                </div>
              ) : (
                <div className="space-y-2">
                  {visibleNotes.map((note) => (
                    <div
                      key={note.id}
                      className="p-3 rounded-lg bg-card border border-border hover:border-border/80 transition-colors cursor-pointer"
                      onClick={(event) => {
                        if (deletingNoteId) {
                          return;
                        }
                        if (Date.now() < suppressNoteCardClickUntilRef.current) {
                          return;
                        }
                        const target = event.target as HTMLElement;
                        if (target.closest('[data-note-action="true"]')) {
                          return;
                        }
                        openEditNoteModal(note);
                      }}
                    >
                      <div className="flex items-start gap-3">
                        <div className="flex-1 min-w-0">
                          <div className="text-sm font-medium truncate">{normalizeNoteText(note.title) || t('note.untitled')}</div>
                          <div className="text-xs text-muted-foreground mt-1 line-clamp-2">
                            {normalizeNoteText(note.content) || t('note.empty')}
                          </div>
                        </div>

                        <div className="flex items-center gap-1.5 shrink-0" data-note-action="true">
                          <button
                            type="button"
                            data-note-action="true"
                            onClick={(e) => {
                              armNoteCardClickGuard();
                              e.preventDefault();
                              e.stopPropagation();
                              void importNoteToKnowledgeBase(note);
                            }}
                            disabled={importingNoteId === note.id}
                            className="h-7 inline-flex items-center gap-1 px-2 rounded-md border border-border hover:bg-accent text-[11px] font-medium disabled:opacity-60 disabled:cursor-not-allowed"
                          >
                            {importingNoteId === note.id ? (
                              <Loader2 className="w-3 h-3 animate-spin" />
                            ) : (
                              <Upload className="w-3 h-3" />
                            )}
                            <span>{t('note.importToKB')}</span>
                          </button>

                          <DropdownMenu>
                            <DropdownMenuTrigger asChild>
                              <button
                                type="button"
                                onClick={(e) => {
                                  armNoteCardClickGuard();
                                  e.preventDefault();
                                  e.stopPropagation();
                                }}
                                data-note-action="true"
                                className="p-2 -mr-1 rounded-lg hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                                aria-label={t('common.edit')}
                              >
                                <MoreVertical className="w-4 h-4" />
                              </button>
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align="end">
                              <DropdownMenuItem
                                onSelect={(e) => {
                                  armNoteCardClickGuard();
                                  e.preventDefault();
                                  e.stopPropagation();
                                  openRename('note', note.id, normalizeNoteText(note.title));
                                }}
                              >
                                <Pencil className="w-4 h-4 mr-2" />
                                {t('note.rename')}
                              </DropdownMenuItem>
                              <DropdownMenuSeparator />
                              <DropdownMenuItem
                                onSelect={(e) => {
                                  armNoteCardClickGuard();
                                  e.preventDefault();
                                  e.stopPropagation();
                                  void deleteNote(note.id);
                                }}
                                className="text-red-500 focus:text-red-500"
                              >
                                <Trash2 className="w-4 h-4 mr-2" />
                                {t('common.delete')}
                              </DropdownMenuItem>
                            </DropdownMenuContent>
                          </DropdownMenu>
                        </div>
                      </div>

                      <div className="mt-2 text-xs text-muted-foreground">
                        {new Date(note.updated_at).toLocaleDateString(locale)}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
          </>
        )}
      </div>

      {/* Resize Handle */}
      <div
        data-testid="resize-handle"
        className={`w-1 shrink-0 bg-border hover:bg-primary cursor-col-resize transition-colors hidden md:block ${
          isDragging ? 'bg-primary' : ''
        }`}
        onMouseDown={handleMouseDown}
      />

      {/* Right Panel - Fixed Chat Area (70%) */}
      <div
        className="h-full overflow-hidden flex flex-col rounded-2xl border border-border bg-card/40 transition-all duration-300"
        style={{ width: sidebarCollapsed ? `calc(100% - ${COLLAPSED_RAIL_PX}px - 4px)` : `calc(${100 - leftWidth}% - 4px)` }}
      >
        <div className="flex-1 overflow-hidden">
          <ChatPanel
            workspaceId={kbId}
            selectedSourceIds={selectedSourceIds}
            onExtractToNote={openNoteFromChatExtract}
          />
        </div>
      </div>

      {/* Modals */}
      <AddSourceModal
        open={addSourceOpen}
        onOpenChange={setAddSourceOpen}
        kbId={kbId}
        onUpload={uploadFiles}
        onAdded={(newIds) => {
          if (newIds && newIds.length > 0) {
            setSelectedSourceIds((prev) => {
              const next = [...prev];
              newIds.forEach((id) => {
                if (!next.includes(id)) {
                  next.push(id);
                }
              });
              return next;
            });
          }
          void loadDocuments();
          scheduleDocumentRefreshWindow();
        }}
      />

      <CreateNoteModal
        open={createNoteOpen}
        onOpenChange={handleNoteModalOpenChange}
        kbId={kbId}
        mode={noteModalMode}
        editNote={editingNote}
        initialTitle={noteModalInitialTitle}
        initialContent={noteModalInitialContent}
        onSuccess={handleNoteCreated}
      />

      <DocumentViewer
        document={previewDocument}
        open={previewOpen}
        onClose={() => {
          setPreviewOpen(false);
          setPreviewDocument(null);
        }}
      />

      <Dialog
        open={renameOpen}
        onOpenChange={(open) => {
          setRenameOpen(open);
          if (!open) {
            setRenameValue('');
            setRenameTargetId('');
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {renameKind === 'doc' ? t('document.renameTitle') : t('note.renameTitle')}
            </DialogTitle>
            <DialogDescription>
              {renameKind === 'doc' ? t('document.renamePlaceholder') : t('note.renamePlaceholder')}
            </DialogDescription>
          </DialogHeader>

          <Input
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            placeholder={renameKind === 'doc' ? t('document.renamePlaceholder') : t('note.renamePlaceholder')}
            onKeyDown={(e) => e.key === 'Enter' && void confirmRename()}
          />

          <DialogFooter>
            <Button variant="outline" onClick={() => setRenameOpen(false)} disabled={renaming}>
              {t('common.cancel')}
            </Button>
            <Button onClick={() => void confirmRename()} disabled={renaming || !renameValue.trim()}>
              {renaming ? <Loader2 className="w-4 h-4 animate-spin mr-2" /> : null}
              {t('common.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function renderStatusBadge(status: Document['status'], t: (key: string) => string) {
  if (status === 'completed') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full border border-green-500/40 bg-green-500/10 px-2 py-0.5 text-xs text-green-300">
        <CheckCircle2 className="h-3 w-3" />
        {t('document.status.completed')}
      </span>
    );
  }

  if (status === 'processing') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-xs text-amber-300">
        <Loader2 className="h-3 w-3 animate-spin" />
        {t('document.status.processing')}
      </span>
    );
  }

  if (status === 'queued' || status === 'enqueueing') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full border border-border/60 bg-muted/40 px-2 py-0.5 text-xs text-foreground/80">
        <Clock3 className="h-3 w-3" />
        {t('document.status.pending')}
      </span>
    );
  }

  if (status === 'failed') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full border border-red-500/40 bg-red-500/10 px-2 py-0.5 text-xs text-red-300">
        <AlertCircle className="h-3 w-3" />
        {t('document.status.failed')}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-border/60 bg-muted/40 px-2 py-0.5 text-xs text-foreground/80">
      <Clock3 className="h-3 w-3" />
      {t('document.status.pending')}
    </span>
  );
}

function isReadyDocumentStatus(status: Document['status']): boolean {
  return status === 'completed';
}

function isPreviewableDocumentStatus(status: Document['status']): boolean {
  return isReadyDocumentStatus(status);
}

function isTerminalDocumentStatus(status: Document['status']): boolean {
  return isReadyDocumentStatus(status) || status === 'failed';
}

function runtimeMessageForDocumentStatus(status: Document['status']): string {
  switch (status) {
    case 'queued':
    case 'enqueueing':
    case 'pending':
      return '已接收文件，等待处理';
    case 'processing':
      return '正在处理文档内容';
    case 'completed':
      return '文档已处理完成';
    case 'failed':
      return '文档处理失败';
    default:
      return '正在同步文档状态';
  }
}

function runtimeEventStatusForDocument(
  status: Document['status']
): DocumentRuntimeStatus['status'] {
  if (status === 'completed') return 'done';
  if (status === 'failed') return 'error';
  return 'progress';
}

function buildRuntimeStatusEntry(
  latestStatusSeqRef: { current: number },
  stage: string,
  status: DocumentRuntimeStatus['status'],
  message: string
): DocumentRuntimeStatus {
  latestStatusSeqRef.current += 1;
  return {
    seq: latestStatusSeqRef.current,
    stage,
    status,
    message,
    timestamp: Date.now(),
  };
}
