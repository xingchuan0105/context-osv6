'use client';

import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FileText, Bot, Archive, Trash2, Upload, Loader2 } from 'lucide-react';
import { useAppStore } from '@/stores/useAppStore';
import { documentsApi, notesApi } from '@/lib/api/client';
import type { Note } from '@/types';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { toast } from '@/components/ui/toaster';
import { NoteEditor } from './note-editor';

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

function normalizeDraftNote(note: Note): Note {
  return {
    ...note,
    title: normalizeNoteField(note.title),
    content: normalizeNoteField(note.content),
  };
}

export function DraftList() {
  const { currentWorkspace } = useAppStore();
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage?.startsWith('en') ? 'en-US' : 'zh-CN';
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedNote, setSelectedNote] = useState<Note | null>(null);
  const [activeTab, setActiveTab] = useState<'drafts' | 'committed'>('drafts');
  const [importingNoteId, setImportingNoteId] = useState<string | null>(null);

  // Load notes
  const loadNotes = useCallback(async (type: 'drafts' | 'committed') => {
    if (!currentWorkspace) return;

    setLoading(true);
    try {
      const noteType = type === 'drafts' ? 'draft' : 'committed';
      const response = await notesApi.list(currentWorkspace.id);
      if (response.success) {
        const filteredNotes = (response.data || []).filter(
          (n: Note) => n.note_type === noteType || (!n.note_type && type === 'drafts')
        );
        setNotes(filteredNotes.map((note: Note) => normalizeDraftNote(note)));
      }
    } catch (error) {
      console.error('Failed to load notes:', error);
    } finally {
      setLoading(false);
    }
  }, [currentWorkspace]);

  // Load on tab change or workspace change
  useEffect(() => {
    void loadNotes(activeTab);
  }, [activeTab, loadNotes]);

  const handleArchive = async (noteId: string) => {
    try {
      const response = await notesApi.update(noteId, { note_type: 'committed' });
      if (response.success) {
        loadNotes(activeTab);
      }
    } catch (error) {
      console.error('Failed to archive note:', error);
    }
  };

  const handleDelete = async (noteId: string) => {
    if (!confirm('确定要删除这条笔记吗？')) return;
    
    try {
      const response = await notesApi.delete(noteId);
      if (response.success) {
        loadNotes(activeTab);
      }
    } catch (error) {
      console.error('Failed to delete note:', error);
    }
  };

  const handleImportToSource = async (note: Note) => {
    if (!currentWorkspace) {
      return;
    }

    const content = normalizeNoteField(note.content).trim();
    if (!content) {
      toast.error(t('note.importEmpty'));
      return;
    }

    setImportingNoteId(note.id);
    try {
      const fileBaseName = (normalizeNoteField(note.title) || t('note.untitled') || 'note')
        .replace(/[\\/:*?"<>|\r\n]+/g, '_')
        .slice(0, 64)
        .trim();
      const fileName = `${fileBaseName || 'note'}.md`;
      const file = new File([content], fileName, { type: 'text/markdown;charset=utf-8' });

      const response = await documentsApi.upload(currentWorkspace.id, file);
      if (!response?.success) {
        throw new Error(response?.error || 'import-note-failed');
      }

      toast.success(t('note.importSuccess'));
    } catch {
      toast.error(t('note.importFailed'));
    } finally {
      setImportingNoteId(null);
    }
  };

  // Editor mode
  if (selectedNote) {
    return (
      <NoteEditor
        note={selectedNote}
        onBack={() => setSelectedNote(null)}
        onArchive={() => {
          handleArchive(selectedNote.id);
          setSelectedNote(null);
        }}
      />
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Tab Header */}
      <div className="flex items-center gap-4 p-4 border-b border-border">
        <Button
          variant={activeTab === 'drafts' ? 'default' : 'ghost'}
          size="sm"
          onClick={() => {
            setActiveTab('drafts');
            loadNotes('drafts');
          }}
        >
          草稿箱
        </Button>
        <Button
          variant={activeTab === 'committed' ? 'default' : 'ghost'}
          size="sm"
          onClick={() => {
            setActiveTab('committed');
            loadNotes('committed');
          }}
        >
          已归档
        </Button>
      </div>

      {/* Notes List */}
      <div className="flex-1 overflow-auto p-4">
        {loading ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground/80">
            加载中...
          </div>
        ) : notes.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32 text-muted-foreground/80">
            <FileText className="w-8 h-8 mb-2 opacity-50" />
            <p>暂无{activeTab === 'drafts' ? '草稿' : '已归档笔记'}</p>
          </div>
        ) : (
          <div className="space-y-2">
            {notes.map((note) => (
              <Card 
                key={note.id}
                className="cursor-pointer hover:border-border/80 transition-colors"
                onClick={() => setSelectedNote(normalizeDraftNote(note))}
              >
                <CardHeader className="pb-2">
                  <CardTitle className="text-sm flex items-center gap-2">
                    <FileText className="w-4 h-4 text-indigo-400" />
                    {normalizeNoteField(note.title) || '无标题'}
                  </CardTitle>
                </CardHeader>
                <CardContent className="pb-3">
                  <p className="text-sm text-muted-foreground line-clamp-2">
                    {normalizeNoteField(note.content)}
                  </p>
                  <div className="flex items-center justify-between mt-2">
                    <div className="flex items-center gap-2 text-xs text-muted-foreground/80">
                      <Bot className="w-3 h-3" />
                      <span>AI 提取</span>
                      <span>·</span>
                      <span>{new Date(note.created_at).toLocaleDateString(locale)}</span>
                    </div>
                    <div className="flex items-center gap-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          void handleImportToSource(note);
                        }}
                        disabled={importingNoteId === note.id}
                        className="h-7 px-2 text-xs"
                      >
                        {importingNoteId === note.id ? (
                          <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                        ) : (
                          <Upload className="w-3 h-3 mr-1" />
                        )}
                        {t('note.importToKB')}
                      </Button>
                      {activeTab === 'drafts' && (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleArchive(note.id);
                          }}
                          className="h-7 px-2 text-xs text-green-400 hover:text-green-300"
                        >
                          <Archive className="w-3 h-3 mr-1" />
                          归档
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDelete(note.id);
                        }}
                        className="h-7 px-2 text-xs text-red-400 hover:text-red-300"
                      >
                        <Trash2 className="w-3 h-3" />
                      </Button>
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
