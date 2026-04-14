'use client';

import { useState, useRef, useCallback, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Upload, Link, X, Loader2, Globe, FolderOpen, Search } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { documentsApi, notebookApi, sourcesApi } from '@/lib/api/client';
import { partitionSupportedUploadFiles, SUPPORTED_UPLOAD_ACCEPT } from '@/lib/upload-file-validation';
import { toast } from '@/components/ui/toaster';

interface AddSourceModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  kbId: string;
  onUpload?: (files: File[]) => Promise<void> | void;
  onAdded?: (ids: string[]) => void;
}

type SourceMode = 'file' | 'url' | 'existing';

interface SourceItem {
  id: string;
  title: string;
  status: string;
  kb_id?: string;
}

function normalizeSourceList(payload: any): SourceItem[] {
  const rows = payload?.data || [];
  if (!Array.isArray(rows)) return [];
  return rows.map((row) => ({
    id: String(row.id || ''),
    title: String(row.title || row.name || row.file_name || ''),
    status: String(row.status || ''),
    kb_id: row.kb_id ? String(row.kb_id) : undefined,
  }));
}

function isSuccessfulSourceStatus(status: string): boolean {
  return status === 'completed' || status === 'active';
}

export function AddSourceModal({ open, onOpenChange, kbId, onUpload, onAdded }: AddSourceModalProps) {
  const { t } = useTranslation();
  const [url, setUrl] = useState('');
  const [loading, setLoading] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);
  const [mode, setMode] = useState<SourceMode>('file');
  const [existingLoading, setExistingLoading] = useState(false);
  const [existingSources, setExistingSources] = useState<SourceItem[]>([]);
  const [existingSearch, setExistingSearch] = useState('');
  const [selectedExistingIds, setSelectedExistingIds] = useState<string[]>([]);
  const [existingError, setExistingError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  const loadExistingSources = useCallback(async () => {
    setExistingLoading(true);
    setExistingError('');
    try {
      const [allRes, currentKbRes] = await Promise.all([sourcesApi.list(), sourcesApi.list(kbId)]);
      if (!allRes.success) {
        const message = allRes.error || t('document.existingUnavailable');
        setExistingSources([]);
        setExistingError(message);
        toast.error(message);
        return;
      }
      if (!currentKbRes.success) {
        const message = currentKbRes.error || t('document.loadExistingFailed');
        setExistingSources([]);
        setExistingError(message);
        toast.error(message);
        return;
      }

      const allSources = normalizeSourceList(allRes);
      const currentSources = normalizeSourceList(currentKbRes);
      const currentSourceIdSet = new Set(currentSources.map((item) => item.id));

      const candidate = allSources.filter((item) => isSuccessfulSourceStatus(item.status) && !currentSourceIdSet.has(item.id));
      setExistingSources(candidate);
    } catch {
      setExistingSources([]);
      setExistingError(t('document.loadExistingFailed'));
      toast.error(t('document.loadExistingFailed'));
    } finally {
      setExistingLoading(false);
    }
  }, [kbId, t]);

  useEffect(() => {
    if (!open) return;
    if (mode === 'existing') {
      void loadExistingSources();
    }
  }, [loadExistingSources, mode, open]);

  useEffect(() => {
    if (open) return;
    setMode('file');
    setUrl('');
    setExistingSearch('');
    setSelectedExistingIds([]);
    setExistingError('');
    setIsDragOver(false);
  }, [open]);

  const filteredExistingSources = useMemo(() => {
    const keyword = existingSearch.trim().toLowerCase();
    if (!keyword) return existingSources;
    return existingSources.filter((item) => item.title.toLowerCase().includes(keyword));
  }, [existingSearch, existingSources]);

  const handleUrlSubmit = async () => {
    if (!url.trim()) return;

    let validatedUrl = url.trim();
    if (!validatedUrl.startsWith('http://') && !validatedUrl.startsWith('https://')) {
      validatedUrl = `https://${validatedUrl}`;
    }

    try {
      new URL(validatedUrl);
    } catch {
      toast.error(t('document.invalidUrl'));
      return;
    }

    setLoading(true);
    try {
      const response = await documentsApi.addUrl(kbId, validatedUrl);
      if (response.success) {
        toast.success(t('document.urlAdded'));
        setUrl('');
        const newId = (response.data as any)?.document_id || (response.data as any)?.id;
        onAdded?.(newId ? [newId] : []);
        onOpenChange(false);
      } else {
        toast.error(response.error || response.message || t('document.urlAddFailed'));
      }
    } catch {
      toast.error(t('document.urlAddFailed'));
    } finally {
      setLoading(false);
    }
  };

  const handleFileUpload = useCallback(async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    const fileArray = Array.from(files);
    const { supported, unsupported } = partitionSupportedUploadFiles(fileArray);

    if (unsupported.length > 0) {
      const unsupportedNames = unsupported.map((file) => file.name).join(', ');
      toast.error(t('document.unsupportedFileType', { files: unsupportedNames }));
    }
    if (supported.length === 0) {
      return;
    }

    if (onUpload) {
      setLoading(true);
      try {
        await onUpload(supported);
        // onUpload is handled in parent, but we might not have the IDs here.
        // Parent already handles auto-select for uploaded files via registerPendingAutoSelectSource
        onAdded?.([]); 
        onOpenChange(false);
      } finally {
        setLoading(false);
      }
      return;
    }

    setLoading(true);
    try {
      const newIds: string[] = [];
      for (const file of supported) {
        const response = await documentsApi.upload(kbId, file);
        if (!response.success) {
          throw new Error(response.error || 'upload-existing-file-failed');
        }
        const newId = (response.data as any)?.document_id || (response.data as any)?.id;
        if (newId) newIds.push(newId);
      }
      toast.success(t('document.submitted', { count: supported.length }));
      onAdded?.(newIds);
      onOpenChange(false);
    } catch {
      toast.error(t('document.uploadFailedGeneric'));
    } finally {
      setLoading(false);
    }
  }, [kbId, onAdded, onOpenChange, onUpload, t]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    const files = e.dataTransfer.files;
    if (files.length > 0) {
      void handleFileUpload(files);
    }
  }, [handleFileUpload]);

  const handleToggleExisting = (id: string) => {
    setSelectedExistingIds((prev) => (prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]));
  };

  const handleAddExistingToWorkspace = async () => {
    if (selectedExistingIds.length === 0) {
      toast.error(t('document.selectExistingHint'));
      return;
    }
    setLoading(true);
    try {
      const response = await notebookApi.addSources(kbId, selectedExistingIds);
      if (!response?.success) {
        toast.error(response?.error || t('document.existingAddFailed'));
        return;
      }
      const added = Number(response?.data?.added || selectedExistingIds.length);
      toast.success(t('document.existingAdded', { count: added }));
      onAdded?.(selectedExistingIds);
      onOpenChange(false);
      setSelectedExistingIds([]);
      setExistingSearch('');
    } catch {
      toast.error(t('document.existingAddFailed'));
    } finally {
      setLoading(false);
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-200">
      <div className="w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl animate-in zoom-in-95 slide-in-from-bottom-4 duration-300">
        <div className="p-5 border-b border-border flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t('document.addSource')}</h2>
          <button
            onClick={() => onOpenChange(false)}
            className="p-2 rounded-lg hover:bg-accent transition-colors"
            aria-label={t('common.close')}
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <button
              onClick={() => setMode('file')}
              className={`h-24 rounded-xl border px-4 text-left transition-colors ${
                mode === 'file' ? 'border-primary bg-primary/10' : 'border-border hover:border-primary/50'
              }`}
            >
              <div className="flex items-center gap-2 text-sm font-medium">
                <Upload className="w-4 h-4" />
                {t('document.uploadFile')}
              </div>
              <p className="mt-2 text-xs text-muted-foreground">{t('document.clickOrDrag')}</p>
            </button>

            <button
              onClick={() => setMode('url')}
              className={`h-24 rounded-xl border px-4 text-left transition-colors ${
                mode === 'url' ? 'border-primary bg-primary/10' : 'border-border hover:border-primary/50'
              }`}
            >
              <div className="flex items-center gap-2 text-sm font-medium">
                <Globe className="w-4 h-4" />
                {t('document.addWebsite')}
              </div>
              <p className="mt-2 text-xs text-muted-foreground">{t('document.addWebsiteDesc')}</p>
            </button>

            <button
              onClick={() => setMode('existing')}
              className={`h-24 rounded-xl border px-4 text-left transition-colors ${
                mode === 'existing' ? 'border-primary bg-primary/10' : 'border-border hover:border-primary/50'
              }`}
            >
              <div className="flex items-center gap-2 text-sm font-medium">
                <FolderOpen className="w-4 h-4" />
                {t('document.fromExisting')}
              </div>
              <p className="mt-2 text-xs text-muted-foreground">{t('document.fromExistingDesc')}</p>
            </button>
          </div>

          {mode === 'file' && (
            <div className="rounded-xl border border-border p-4 min-h-[320px]">
              <input
                ref={fileInputRef}
                type="file"
                multiple
                onChange={(e) => void handleFileUpload(e.target.files)}
                accept={SUPPORTED_UPLOAD_ACCEPT}
                className="hidden"
              />

              <div
                className={`h-full min-h-[284px] border-2 border-dashed rounded-xl p-10 text-center transition-colors cursor-pointer flex flex-col items-center justify-center ${
                  isDragOver ? 'border-primary bg-primary/10' : 'border-border hover:border-primary/50'
                }`}
                onDragOver={(e) => {
                  e.preventDefault();
                  setIsDragOver(true);
                }}
                onDragLeave={() => setIsDragOver(false)}
                onDrop={handleDrop}
                onClick={() => fileInputRef.current?.click()}
              >
                {loading ? (
                  <Loader2 className="w-10 h-10 mx-auto mb-3 animate-spin text-primary" />
                ) : (
                  <Upload className="w-10 h-10 mx-auto mb-3 text-muted-foreground" />
                )}
                <p className="text-sm font-medium mb-1">{t('document.clickOrDrag')}</p>
                <p className="text-xs text-muted-foreground">{t('document.supportedFormats')}</p>
              </div>
            </div>
          )}

          {mode === 'url' && (
            <div className="rounded-xl border border-border p-4 min-h-[320px] flex flex-col justify-center space-y-3">
              <div className="mx-auto w-full max-w-xl space-y-3">
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Globe className="w-4 h-4" />
                  <span>{t('document.addWebsiteDesc')}</span>
                </div>
                <div className="flex gap-2">
                  <Input
                    placeholder={t('document.urlPlaceholder')}
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && void handleUrlSubmit()}
                    className="flex-1"
                  />
                  <Button onClick={() => void handleUrlSubmit()} disabled={loading || !url.trim()}>
                    {loading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Link className="w-4 h-4" />}
                  </Button>
                </div>
              </div>
            </div>
          )}

          {mode === 'existing' && (
            <div className="rounded-xl border border-border p-4 min-h-[320px] flex flex-col space-y-3">
              {existingError ? (
                <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-200">
                  {existingError}
                </div>
              ) : null}

              <div className="relative">
                <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={existingSearch}
                  onChange={(e) => setExistingSearch(e.target.value)}
                  placeholder={t('document.searchExistingPlaceholder')}
                  className="pl-9"
                />
              </div>

              <div className="max-h-64 overflow-auto rounded-lg border border-border flex-1">
                {existingLoading ? (
                  <div className="py-8 text-center text-sm text-muted-foreground">
                    <Loader2 className="w-4 h-4 animate-spin inline-block mr-2" />
                    {t('common.loading')}
                  </div>
                ) : existingError ? (
                  <div className="py-8 text-center text-sm text-muted-foreground">{existingError}</div>
                ) : filteredExistingSources.length === 0 ? (
                  <div className="py-8 text-center text-sm text-muted-foreground">{t('document.noExistingSources')}</div>
                ) : (
                  filteredExistingSources.map((item) => (
                    <label
                      key={item.id}
                      className="flex items-center gap-3 px-3 py-2.5 border-b border-border/60 last:border-b-0 hover:bg-accent/40 cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={selectedExistingIds.includes(item.id)}
                        onChange={() => handleToggleExisting(item.id)}
                        className="h-4 w-4 rounded border-border accent-primary"
                      />
                      <div className="min-w-0 flex-1">
                        <div className="text-sm font-medium truncate">{item.title}</div>
                        <div className="text-xs text-muted-foreground truncate">{item.id}</div>
                      </div>
                    </label>
                  ))
                )}
              </div>

              <div className="flex justify-end">
                <Button
                  onClick={() => void handleAddExistingToWorkspace()}
                  disabled={loading || selectedExistingIds.length === 0 || Boolean(existingError)}
                >
                  {loading ? <Loader2 className="w-4 h-4 animate-spin mr-2" /> : null}
                  {t('document.addToCurrentWorkspace')}
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
