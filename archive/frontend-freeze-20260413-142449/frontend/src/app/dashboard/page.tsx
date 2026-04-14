'use client';

import { useCallback, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import {
  Plus,
  LayoutGrid,
  List,
  MoreVertical,
  Edit,
  Trash2,
  FolderOpen,
  Loader2,
  Star,
  ExternalLink,
} from 'lucide-react';
import { kbApi } from '@/lib/api/client';
import { toast } from '@/components/ui/toaster';
import { useAppStore } from '@/stores/useAppStore';
import type { FavoriteKnowledgeBase, KnowledgeBase } from '@/types';
import { useTranslation } from 'react-i18next';

export default function DashboardPage() {
  const router = useRouter();
  const { t } = useTranslation();
  const { currentWorkspace, setCurrentWorkspace } = useAppStore();

  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBase[]>([]);
  const [favoriteKnowledgeBases, setFavoriteKnowledgeBases] = useState<FavoriteKnowledgeBase[]>([]);
  const [loading, setLoading] = useState(true);
  const [viewMode, setViewMode] = useState<'card' | 'list'>('card');
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingKB, setEditingKB] = useState<KnowledgeBase | null>(null);
  const [deletingKBId, setDeletingKBId] = useState<string | null>(null);

  const loadKnowledgeBases = useCallback(async () => {
    try {
      setLoading(true);
      const [ownedResult, favoriteResult] = await Promise.allSettled([
        kbApi.list(),
        kbApi.listFavorites(),
      ]);

      if (ownedResult.status === 'fulfilled' && ownedResult.value?.success && ownedResult.value?.data) {
        setKnowledgeBases(ownedResult.value.data);
      } else if (
        ownedResult.status === 'fulfilled' &&
        ownedResult.value?.error_code === 'UNAUTHORIZED'
      ) {
        toast.error(t('errors.unauthorized'));
        router.replace('/login');
      } else {
        toast.error(t('dashboard.loadFailed'));
      }

      if (favoriteResult.status === 'fulfilled' && favoriteResult.value?.success && favoriteResult.value?.data) {
        setFavoriteKnowledgeBases(favoriteResult.value.data);
      } else {
        setFavoriteKnowledgeBases([]);
      }
    } finally {
      setLoading(false);
    }
  }, [router, t]);

  useEffect(() => {
    void loadKnowledgeBases();
  }, [loadKnowledgeBases]);

  const handleCreateKB = async (title: string, description: string) => {
    try {
      const response = await kbApi.create(title, description);
      if (!response.success || !response.data) {
        toast.error(response.error || t('dashboard.createFailed'));
        return;
      }
      setKnowledgeBases((prev) => [...prev, response.data]);
      setShowCreateModal(false);
      toast.success(t('dashboard.createSuccess'));
    } catch {
      toast.error(t('dashboard.createFailed'));
    }
  };

  const handleUpdateKB = async (id: string, title: string, description: string) => {
    try {
      const response = await kbApi.update(id, { title, description });
      if (response.success) {
        setKnowledgeBases((prev) =>
          prev.map((kb) => (kb.id === id ? { ...kb, title, description } : kb))
        );
        setEditingKB(null);
        toast.success(t('dashboard.updateSuccess'));
      }
    } catch {
      toast.error(t('dashboard.updateFailed'));
    }
  };

  const handleDeleteKB = async (id: string) => {
    if (deletingKBId) return;
    setDeletingKBId(id);
    if (!confirm(t('dashboard.deleteConfirm'))) {
      setDeletingKBId((prev) => (prev === id ? null : prev));
      return;
    }

    try {
      setEditingKB((prev) => (prev?.id === id ? null : prev));
      await kbApi.delete(id);
      setKnowledgeBases((prev) => prev.filter((kb) => kb.id !== id));
      if (currentWorkspace?.id === id) {
        setCurrentWorkspace(null);
      }
      toast.success(t('dashboard.deleteSuccess'));
    } catch {
      toast.error(t('dashboard.deleteFailed'));
    } finally {
      setDeletingKBId((prev) => (prev === id ? null : prev));
    }
  };

  const handleSelectKB = (kb: KnowledgeBase) => {
    if (deletingKBId) {
      return;
    }
    setCurrentWorkspace(kb);
    router.push(`/dashboard/${kb.id}`);
  };

  const handleEditKB = (kb: KnowledgeBase) => {
    if (deletingKBId) {
      return;
    }
    setEditingKB(kb);
  };

  const handleSelectFavoriteKB = (favorite: FavoriteKnowledgeBase) => {
    router.push(`/shared/kb/${favorite.share_token}`);
  };

  const handleUnfavoriteKB = async (shareToken: string) => {
    try {
      const response = await kbApi.unfavoriteByToken(shareToken);
      if (!response?.success) {
        throw new Error(response?.error || 'unfavorite-failed');
      }

      setFavoriteKnowledgeBases((prev) => prev.filter((item) => item.share_token !== shareToken));
      toast.success(t('share.favoriteRemoved'));
    } catch {
      toast.error(t('share.favoriteRemoveFailed'));
    }
  };

  return (
    <div className="min-h-full bg-background text-foreground">
      <div className="p-3 md:p-4 border-b border-border flex items-center justify-between gap-2 md:gap-4">
        <div className="flex items-center gap-2">
          <div className="flex items-center bg-card rounded-lg p-1 border border-border">
            <button
              onClick={() => setViewMode('card')}
              className={`p-2 rounded min-w-[44px] min-h-[44px] flex items-center justify-center ${
                viewMode === 'card' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'
              }`}
              aria-label={t('dashboard.cardView')}
            >
              <LayoutGrid className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('list')}
              className={`p-2 rounded min-w-[44px] min-h-[44px] flex items-center justify-center ${
                viewMode === 'list' ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'
              }`}
              aria-label={t('dashboard.listView')}
            >
              <List className="w-4 h-4" />
            </button>
          </div>
        </div>

        <button
            onClick={() => setShowCreateModal(true)}
            className="flex items-center gap-2 h-10 px-3 md:px-4 rounded-lg bg-primary hover:opacity-90 text-primary-foreground font-medium transition-colors min-w-[44px]"
          >
            <Plus className="w-4 h-4" />
            <span className="hidden sm:inline">{t('dashboard.newWorkspace')}</span>
            <span className="sm:hidden">{t('dashboard.newShort')}</span>
          </button>
      </div>

      <div className="p-2 md:p-4">
        {loading ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 md:gap-4">
            {[...Array(8)].map((_, i) => (
              <div key={i} className="bg-card border border-border rounded-xl p-4 animate-pulse">
                <div className="w-10 h-10 rounded-lg bg-muted mb-3" />
                <div className="h-4 bg-muted rounded mb-2 w-3/4" />
                <div className="h-3 bg-muted rounded w-full" />
              </div>
            ))}
          </div>
        ) : (
          <div className="space-y-8">
            <section className="space-y-3">
              <div className="flex items-center gap-2">
                <FolderOpen className="w-4 h-4 text-muted-foreground" />
                <h2 className="text-base font-semibold">{t('dashboard.workspaceListTitle')}</h2>
              </div>

              {knowledgeBases.length === 0 ? (
                <div className="flex flex-col items-center justify-center h-48 rounded-xl border border-dashed border-border text-muted-foreground">
                  <FolderOpen className="w-10 h-10 mb-3 opacity-50" />
                  <p className="text-sm">{t('dashboard.noWorkspace')}</p>
                </div>
              ) : viewMode === 'card' ? (
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 md:gap-4">
                  {knowledgeBases.map((kb) => (
                    <KBGridCard
                      key={kb.id}
                      kb={kb}
                      onSelect={() => handleSelectKB(kb)}
                      onEdit={() => handleEditKB(kb)}
                      onDelete={() => void handleDeleteKB(kb.id)}
                      deleting={deletingKBId === kb.id}
                    />
                  ))}
                </div>
              ) : (
                <div className="space-y-2">
                  {knowledgeBases.map((kb) => (
                    <KBListItem
                      key={kb.id}
                      kb={kb}
                      onSelect={() => handleSelectKB(kb)}
                      onEdit={() => handleEditKB(kb)}
                      onDelete={() => void handleDeleteKB(kb.id)}
                      deleting={deletingKBId === kb.id}
                    />
                  ))}
                </div>
              )}
            </section>

            <section className="space-y-3">
              <div className="flex items-center gap-2">
                <Star className="w-4 h-4 text-amber-500" />
                <h2 className="text-base font-semibold">{t('dashboard.favoriteWorkspaceListTitle')}</h2>
              </div>

              {favoriteKnowledgeBases.length === 0 ? (
                <div className="flex items-center justify-center h-32 rounded-xl border border-dashed border-border text-sm text-muted-foreground">
                  {t('dashboard.noFavoriteWorkspace')}
                </div>
              ) : viewMode === 'card' ? (
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 md:gap-4">
                  {favoriteKnowledgeBases.map((favorite) => (
                    <FavoriteKBGridCard
                      key={favorite.favorite_id || favorite.share_token || favorite.id}
                      kb={favorite}
                      onSelect={() => handleSelectFavoriteKB(favorite)}
                      onUnfavorite={() => void handleUnfavoriteKB(favorite.share_token)}
                    />
                  ))}
                </div>
              ) : (
                <div className="space-y-2">
                  {favoriteKnowledgeBases.map((favorite) => (
                    <FavoriteKBListItem
                      key={favorite.favorite_id || favorite.share_token || favorite.id}
                      kb={favorite}
                      onSelect={() => handleSelectFavoriteKB(favorite)}
                      onUnfavorite={() => void handleUnfavoriteKB(favorite.share_token)}
                    />
                  ))}
                </div>
              )}
            </section>
          </div>
        )}
      </div>

      {showCreateModal && (
        <KBModal
          title={t('dashboard.createWorkspaceTitle')}
          onSubmit={handleCreateKB}
          onClose={() => setShowCreateModal(false)}
        />
      )}

      {editingKB && (
        <KBModal
          title={t('dashboard.editWorkspaceTitle')}
          initialTitle={editingKB.title}
          initialDescription={editingKB.description || ''}
          onSubmit={(title, desc) => handleUpdateKB(editingKB.id, title, desc)}
          onClose={() => setEditingKB(null)}
        />
      )}
    </div>
  );
}

function KBGridCard({
  kb,
  onSelect,
  onEdit,
  onDelete,
  deleting = false,
}: {
  kb: KnowledgeBase;
  onSelect: () => void;
  onEdit: () => void;
  onDelete: () => void;
  deleting?: boolean;
}) {
  const { t } = useTranslation();
  const [showMenu, setShowMenu] = useState(false);
  const handleSelect = (e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest('[data-kb-action="true"]')) {
      return;
    }
    onSelect();
  };

  return (
    <div
      onClick={handleSelect}
      className={`group relative rounded-2xl p-4 cursor-pointer reading-panel
        hover:border-primary/35 hover:shadow-[var(--shadow-md)]
        transition-all duration-300 ease-out
        hover:-translate-y-1 active:scale-[0.98] ${deleting ? 'opacity-60 pointer-events-none' : ''}`}
    >
      <div className="absolute top-3 right-3" data-kb-action="true">
        <button
          data-kb-action="true"
          disabled={deleting}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setShowMenu((prev) => !prev);
          }}
          className="p-2 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center hover:bg-accent text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity"
        >
          <MoreVertical className="w-4 h-4" />
        </button>

        {showMenu && (
          <div
            data-kb-action="true"
            className="absolute right-0 top-12 w-36 bg-popover/96 border border-border rounded-2xl shadow-[var(--shadow-lg)] backdrop-blur-xl py-1.5 z-10"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              data-kb-action="true"
              disabled={deleting}
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onEdit();
                setShowMenu(false);
              }}
                className="w-full flex items-center gap-2 px-3 py-2.5 text-sm hover:bg-accent min-h-[44px] rounded-xl"
            >
              <Edit className="w-4 h-4" />
              {t('common.edit')}
            </button>
            <button
              data-kb-action="true"
              disabled={deleting}
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onDelete();
                setShowMenu(false);
              }}
                className="w-full flex items-center gap-2 px-3 py-2.5 text-sm text-red-400 hover:bg-accent min-h-[44px] rounded-xl"
            >
              <Trash2 className="w-4 h-4" />
              {t('common.delete')}
            </button>
          </div>
        )}
      </div>

      <div className="w-10 h-10 rounded-xl bg-[linear-gradient(135deg,rgba(124,58,237,0.16),rgba(37,99,235,0.14))] border border-white/6 flex items-center justify-center mb-3">
        <FolderOpen className="w-5 h-5 text-violet-300" />
      </div>

      <h3 className="font-medium mb-1 truncate text-[15px] text-[color:var(--text-primary)]">{kb.title}</h3>
      <p className="text-sm text-muted-foreground line-clamp-2">{kb.description || t('dashboard.noDescription')}</p>
    </div>
  );
}

function KBListItem({
  kb,
  onSelect,
  onEdit,
  onDelete,
  deleting = false,
}: {
  kb: KnowledgeBase;
  onSelect: () => void;
  onEdit: () => void;
  onDelete: () => void;
  deleting?: boolean;
}) {
  const { t } = useTranslation();
  const handleSelect = (e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest('[data-kb-action="true"]')) {
      return;
    }
    onSelect();
  };

  return (
    <div
      onClick={handleSelect}
      className={`group flex items-center gap-3 md:gap-4 p-3 md:p-4 bg-card/92 border border-border rounded-2xl cursor-pointer
        hover:border-primary/35 hover:shadow-[var(--shadow-sm)]
        transition-all duration-200 ease-out active:scale-[0.99] ${deleting ? 'opacity-60 pointer-events-none' : ''}`}
    >
      <div className="w-10 h-10 rounded-xl bg-[linear-gradient(135deg,rgba(124,58,237,0.16),rgba(37,99,235,0.14))] border border-white/6 flex items-center justify-center shrink-0">
        <FolderOpen className="w-5 h-5 text-violet-300" />
      </div>

      <div className="flex-1 min-w-0">
        <h3 className="font-medium truncate">{kb.title}</h3>
        <p className="text-sm text-muted-foreground truncate">{kb.description || t('dashboard.noDescription')}</p>
      </div>

      <div className="flex items-center gap-1 md:gap-2 shrink-0" data-kb-action="true">
        <button
          data-kb-action="true"
          disabled={deleting}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onEdit();
          }}
          className="p-2.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
        >
          <Edit className="w-4 h-4" />
        </button>
        <button
          data-kb-action="true"
          disabled={deleting}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onDelete();
          }}
          className="p-2.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center hover:bg-accent text-muted-foreground hover:text-red-400 transition-colors"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}

function FavoriteKBGridCard({
  kb,
  onSelect,
  onUnfavorite,
}: {
  kb: FavoriteKnowledgeBase;
  onSelect: () => void;
  onUnfavorite: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      onClick={onSelect}
      className="group relative bg-card/92 border border-border rounded-2xl p-4 cursor-pointer
        hover:border-primary/35 hover:shadow-[var(--shadow-md)]
        transition-all duration-300 ease-out
        hover:-translate-y-1 active:scale-[0.98]"
    >
      <button
        onClick={(e) => {
          e.stopPropagation();
          onUnfavorite();
        }}
        className="absolute top-3 right-3 p-2 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center text-amber-500 hover:text-red-400 hover:bg-accent transition-colors"
        title={t('share.unfavorite')}
      >
        <Star className="w-4 h-4 fill-current" />
      </button>

      <div className="w-10 h-10 rounded-xl bg-amber-500/12 border border-white/6 flex items-center justify-center mb-3">
        <Star className="w-5 h-5 text-amber-500 fill-current" />
      </div>

      <h3 className="font-medium mb-1 truncate">{kb.title}</h3>
      <p className="text-xs text-muted-foreground mb-2 truncate">
        {kb.origin_title && kb.origin_title !== kb.title ? `${t('dashboard.originTitle')}: ${kb.origin_title}` : t('dashboard.sharedWorkspace')}
      </p>
      <div className="inline-flex items-center gap-1 text-xs text-primary">
        <ExternalLink className="w-3 h-3" />
        <span>{t('dashboard.openSharedWorkspace')}</span>
      </div>
    </div>
  );
}

function FavoriteKBListItem({
  kb,
  onSelect,
  onUnfavorite,
}: {
  kb: FavoriteKnowledgeBase;
  onSelect: () => void;
  onUnfavorite: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      onClick={onSelect}
      className="group flex items-center gap-3 md:gap-4 p-3 md:p-4 bg-card/92 border border-border rounded-2xl cursor-pointer
        hover:border-primary/35 hover:shadow-[var(--shadow-sm)]
        transition-all duration-200 ease-out active:scale-[0.99]"
    >
      <div className="w-10 h-10 rounded-xl bg-amber-500/12 border border-white/6 flex items-center justify-center shrink-0">
        <Star className="w-5 h-5 text-amber-500 fill-current" />
      </div>

      <div className="flex-1 min-w-0">
        <h3 className="font-medium truncate">{kb.title}</h3>
        <p className="text-sm text-muted-foreground truncate">
          {kb.origin_title && kb.origin_title !== kb.title ? `${t('dashboard.originTitle')}: ${kb.origin_title}` : t('dashboard.sharedWorkspace')}
        </p>
      </div>

      <div className="flex items-center gap-1 md:gap-2 shrink-0">
        <button
          onClick={(e) => {
            e.stopPropagation();
            onSelect();
          }}
          className="p-2.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          title={t('dashboard.openSharedWorkspace')}
        >
          <ExternalLink className="w-4 h-4" />
        </button>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onUnfavorite();
          }}
          className="p-2.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center hover:bg-accent text-amber-500 hover:text-red-400 transition-colors"
          title={t('share.unfavorite')}
        >
          <Star className="w-4 h-4 fill-current" />
        </button>
      </div>
    </div>
  );
}

function KBModal({
  title,
  initialTitle = '',
  initialDescription = '',
  onSubmit,
  onClose,
}: {
  title: string;
  initialTitle?: string;
  initialDescription?: string;
  onSubmit: (title: string, description: string) => Promise<void> | void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [kbTitle, setKbTitle] = useState(initialTitle);
  const [description, setDescription] = useState(initialDescription);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!kbTitle.trim()) return;

    setLoading(true);
    await onSubmit(kbTitle.trim(), description.trim());
    setLoading(false);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 animate-in fade-in duration-200">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative w-full max-w-md bg-card/94 border border-border rounded-3xl p-4 md:p-6 animate-in zoom-in-95 slide-in-from-bottom-4 duration-300 shadow-[var(--shadow-lg)] backdrop-blur-xl">
        <h2 className="text-lg font-semibold mb-4">{title}</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-muted-foreground mb-2">{t('dashboard.nameLabel')}</label>
            <input
              type="text"
              value={kbTitle}
              onChange={(e) => setKbTitle(e.target.value)}
              placeholder={t('dashboard.namePlaceholder')}
              className="w-full h-11 md:h-10 px-3 rounded-xl bg-background/70 border border-border shadow-[var(--shadow-sm)]
                placeholder:text-muted-foreground
                focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary
                hover:border-border/80
                transition-all duration-200"
              autoFocus
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-muted-foreground mb-2">{t('dashboard.descLabel')}</label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t('dashboard.descPlaceholder')}
              rows={3}
              className="w-full px-3 py-2 rounded-xl bg-background/70 border border-border shadow-[var(--shadow-sm)]
                placeholder:text-muted-foreground
                focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary
                hover:border-border/80
                transition-all duration-200 resize-none"
            />
          </div>

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="h-11 md:h-10 px-4 rounded-lg border border-border hover:bg-accent transition-colors"
            >
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              disabled={!kbTitle.trim() || loading}
              className="h-11 md:h-10 px-4 rounded-lg bg-primary hover:opacity-90 text-primary-foreground font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
            >
              {loading && <Loader2 className="w-4 h-4 animate-spin" />}
              {t('common.confirm')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
