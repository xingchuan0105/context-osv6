'use client';

import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FileText, X, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { notesApi } from '@/lib/api/client';
import { toast } from '@/components/ui/toaster';
import type { Note } from '@/types';

interface CreateNoteModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  kbId: string;
  mode?: 'create' | 'edit';
  editNote?: Note | null;
  initialTitle?: string;
  initialContent?: string;
  onSuccess?: (note: Note) => void;
}

function normalizeTextField(value: unknown): string {
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

export function CreateNoteModal({
  open,
  onOpenChange,
  kbId,
  mode = 'create',
  editNote = null,
  initialTitle = '',
  initialContent = '',
  onSuccess,
}: CreateNoteModalProps) {
  const { t } = useTranslation();
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);

  const isEditMode = useMemo(() => mode === 'edit', [mode]);

  useEffect(() => {
    if (!open) return;
    setTitle(normalizeTextField(editNote?.title) || initialTitle || '');
    setContent(normalizeTextField(editNote?.content) || initialContent || '');
  }, [editNote?.content, editNote?.title, initialContent, initialTitle, open]);

  const resetForm = () => {
    setTitle('');
    setContent('');
  };

  const handleSave = async () => {
    if (loading) return;
    setLoading(true);
    try {
      if (isEditMode && editNote) {
        const updateResponse = await notesApi.update(editNote.id, {
          title: title.trim(),
          content,
        });
        if (!updateResponse?.success) {
          throw new Error(updateResponse?.error || updateResponse?.message || 'note-update-failed');
        }
        const merged: Note = {
          ...editNote,
          title: title.trim(),
          content,
          updated_at: new Date().toISOString(),
        };
        toast.success(t('note.saved'));
        onSuccess?.(merged);
        onOpenChange(false);
        resetForm();
        return;
      }

      if (isEditMode && !editNote) {
        throw new Error(t('note.createFailed'));
      }

      const createResponse = await notesApi.create(kbId, content, title.trim() || t('note.createTitle'));
      if (!createResponse?.success || !createResponse?.data) {
        throw new Error(createResponse?.error || createResponse?.message || 'note-create-failed');
      }
      toast.success(t('note.saved'));
      onSuccess?.(createResponse.data);
      onOpenChange(false);
      resetForm();
    } catch (error: any) {
      toast.error(error?.message || t('note.createFailed'));
    } finally {
      setLoading(false);
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-200">
      <div className="w-full max-w-3xl bg-card border border-border rounded-2xl shadow-2xl animate-in zoom-in-95 slide-in-from-bottom-4 duration-300">
        <div className="p-5 border-b border-border flex items-center justify-between">
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <FileText className="w-5 h-5" />
            {isEditMode ? t('note.editNote') : t('note.newNote')}
          </h2>
          <button
            onClick={() => onOpenChange(false)}
            className="p-2 rounded-lg hover:bg-accent transition-colors"
            aria-label={t('common.close')}
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <div>
            <label className="text-sm font-medium mb-2 block">{t('note.title')}</label>
            <Input
              placeholder={t('note.titlePlaceholder')}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </div>

          <div>
            <label className="text-sm font-medium mb-2 block">{t('note.content')}</label>
            <Textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder={t('note.contentPlaceholder')}
              className="min-h-[320px] resize-y"
            />
          </div>

          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button onClick={() => void handleSave()} disabled={loading}>
              {loading ? <Loader2 className="w-4 h-4 animate-spin mr-2" /> : null}
              {t('common.save')}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
