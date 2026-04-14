/**
 * NoteEditor - 笔记编辑器组件
 * 
 * 功能：
 * - 笔记内容编辑（textarea）
 * - 自动保存（900ms 防抖）
 * - 手动保存（Cmd/Ctrl + S）
 * - 预览模式
 * - 归档功能
 * 
 * 自动保存逻辑：
 * - 用户停止输入 900ms 后自动保存
 * - 仅当内容发生变化时才保存
 * - 保存失败时显示错误提示
 * 
 * 快捷键：
 * - Cmd/Ctrl + S: 手动保存
 */

'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { ArrowLeft, Eye, Archive, Loader2, Save } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { notesApi } from '@/lib/api/client';
import { toast } from '@/components/ui/toaster';
import type { Note } from '@/types';
import { Button } from '@/components/ui/button';

interface NoteEditorProps {
  note: Note;
  onBack: () => void;
  onArchive: (note: Note) => Promise<void> | void;
  onUpdate?: (partial: Partial<Note> & { id: string }) => void;
}

function normalizeNoteField(value: unknown): string {
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

export function NoteEditor({ note, onBack, onArchive, onUpdate }: NoteEditorProps) {
  const [content, setContent] = useState(normalizeNoteField(note.content));
  const [title, setTitle] = useState(normalizeNoteField(note.title));
  const [saving, setSaving] = useState(false);
  const [lastSaved, setLastSaved] = useState<Date | null>(null);
  const [isPreview, setIsPreview] = useState(false);
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage?.startsWith('en') ? 'en-US' : 'zh-CN';
  const saveTimeoutRef = useRef<NodeJS.Timeout | undefined>(undefined);

  useEffect(() => {
    setContent(normalizeNoteField(note.content));
    setTitle(normalizeNoteField(note.title));
  }, [note.content, note.id, note.title]);

  const persist = useCallback(
    async (nextContent: string, nextTitle: string, showSuccessToast: boolean) => {
      setSaving(true);
      try {
        const payload = {
          content: nextContent,
          title: nextTitle || undefined,
        };
        const response = await notesApi.update(note.id, payload);
        if (!response.success) {
          throw new Error('save-failed');
        }

        const updatedAt = new Date().toISOString();
        setLastSaved(new Date(updatedAt));
        onUpdate?.({ id: note.id, ...payload, updated_at: updatedAt });
        if (showSuccessToast) {
          toast.success('笔记已保存');
        }
      } catch {
        toast.error('保存失败，请重试');
      } finally {
        setSaving(false);
      }
    },
    [note.id, onUpdate]
  );

  useEffect(() => {
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
    }

    saveTimeoutRef.current = setTimeout(() => {
      if (content !== normalizeNoteField(note.content) || title !== normalizeNoteField(note.title)) {
        void persist(content, title, false);
      }
    }, 900);

    return () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
      }
    };
  }, [content, note.content, note.title, persist, title]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        void persist(content, title, true);
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [content, persist, title]);

  const handleArchive = async () => {
    await onArchive({ ...note, content, title });
  };

  return (
    <div className="flex flex-col h-full bg-card">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeft className="w-4 h-4 mr-1" />
          返回
        </Button>

        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => void persist(content, title, true)}>
            <Save className="w-4 h-4 mr-1" />
            保存
          </Button>
          <Button variant="outline" size="sm" onClick={() => setIsPreview((prev) => !prev)}>
            <Eye className="w-4 h-4 mr-1" />
            {isPreview ? '编辑' : '预览'}
          </Button>
          <Button size="sm" className="bg-green-600 hover:bg-green-700 text-white" onClick={handleArchive}>
            <Archive className="w-4 h-4 mr-1" />
            归档
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-4">
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="笔记标题..."
          className="w-full bg-transparent border-none text-lg font-semibold placeholder:text-muted-foreground focus:outline-none mb-3"
        />

        {isPreview ? (
          <pre className="whitespace-pre-wrap text-sm text-foreground/90 leading-relaxed">{content}</pre>
        ) : (
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="开始输入笔记内容..."
            className="w-full h-[calc(100%-48px)] bg-transparent border-none resize-none focus:outline-none text-sm leading-relaxed"
          />
        )}
      </div>

      <div className="px-4 py-2 border-t border-border flex items-center justify-between text-xs text-muted-foreground">
        <div className="flex items-center gap-2">
          {saving ? (
            <>
              <Loader2 className="w-3 h-3 animate-spin" />
              <span>保存中...</span>
            </>
          ) : lastSaved ? (
            <span>已保存</span>
          ) : (
            <span>未更改</span>
          )}
        </div>
        {lastSaved && (
          <span>
            上次更新 {lastSaved.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })}
          </span>
        )}
      </div>
    </div>
  );
}
