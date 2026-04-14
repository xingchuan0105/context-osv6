'use client';

import { useEffect, useMemo, useState } from 'react';
import Link from 'next/link';
import { useParams, useRouter } from 'next/navigation';
import { useTranslation } from 'react-i18next';
import { Loader2, Lock, FileText, Star } from 'lucide-react';
import { kbApi, getAuthToken } from '@/lib/api/client';
import { ChatPanel } from '@/components/chat/chat-panel';
import { toast } from '@/components/ui/toaster';

interface SharedSource {
  id: string;
  file_name: string;
  status: string;
  content?: string;
}

interface SharedData {
  knowledge_base: {
    id: string;
    title: string;
    description?: string;
  };
  share: {
    permission: 'full' | 'partial';
    expires_at?: string | null;
  };
  sources: SharedSource[];
}

export default function SharedKnowledgeBasePage() {
  const params = useParams();
  const router = useRouter();
  const { t } = useTranslation();
  const token = String(params.token || '');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [data, setData] = useState<SharedData | null>(null);
  const [isFavorite, setIsFavorite] = useState(false);
  const [favoriteId, setFavoriteId] = useState('');
  const [favoriteLoading, setFavoriteLoading] = useState(false);
  const [favoriteReady, setFavoriteReady] = useState(false);
  const [isLoggedIn, setIsLoggedIn] = useState(false);

  useEffect(() => {
    if (!token) return;
    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setError('');
      try {
        const response = await fetch(`/api/shared/kb/${token}`);
        const payload = await response.json();
        if (!response.ok || !payload?.success || !payload?.data) {
          throw new Error(payload?.error || 'not-found');
        }
        if (!cancelled) {
          setData(payload.data);
        }
      } catch {
        if (!cancelled) {
          setError(t('share.invalidLink'));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [t, token]);

  useEffect(() => {
    if (!token) return;
    let cancelled = false;
    const authToken = getAuthToken();

    if (!authToken) {
      setIsLoggedIn(false);
      setIsFavorite(false);
      setFavoriteId('');
      setFavoriteReady(true);
      return;
    }

    setIsLoggedIn(true);
    const loadFavoriteState = async () => {
      try {
        const response = await kbApi.listFavorites();
        if (!cancelled && response?.success && Array.isArray(response?.data)) {
          const matched = response.data.find((item: { share_token?: string; favorite_id?: string }) => item?.share_token === token);
          setIsFavorite(Boolean(matched));
          setFavoriteId(matched?.favorite_id || '');
        }
      } catch {
        if (!cancelled) {
          setIsFavorite(false);
          setFavoriteId('');
        }
      } finally {
        if (!cancelled) {
          setFavoriteReady(true);
        }
      }
    };

    void loadFavoriteState();

    return () => {
      cancelled = true;
    };
  }, [token]);

  const handleToggleFavorite = async () => {
    if (!token) return;

    if (!isLoggedIn) {
      router.push(`/login?next=${encodeURIComponent(`/shared/kb/${token}`)}`);
      return;
    }

    setFavoriteLoading(true);
    try {
      if (isFavorite) {
        const response = await kbApi.unfavoriteByToken(token);
        if (!response?.success) {
          throw new Error(response?.error || 'unfavorite-failed');
        }
        setIsFavorite(false);
        setFavoriteId('');
        toast.success(t('share.favoriteRemoved'));
      } else {
        const response = await kbApi.favoriteByToken(token, data?.knowledge_base?.title || undefined);
        if (!response?.success) {
          throw new Error(response?.error || 'favorite-failed');
        }
        setIsFavorite(true);
        setFavoriteId(String(response?.data?.id || ''));
        toast.success(t('share.favoriteAdded'));
      }
    } catch {
      toast.error(isFavorite ? t('share.favoriteRemoveFailed') : t('share.favoriteAddFailed'));
    } finally {
      setFavoriteLoading(false);
    }
  };

  const expiresText = useMemo(() => {
    if (!data?.share?.expires_at) return t('share.neverExpire');
    const d = new Date(data.share.expires_at);
    return Number.isNaN(d.getTime()) ? t('share.neverExpire') : d.toLocaleString();
  }, [data?.share?.expires_at, t]);

  const chatSource = useMemo(() => {
    if (!token) {
      return undefined;
    }
    if (favoriteId) {
      return {
        type: 'favorite' as const,
        token,
      };
    }
    return {
      type: 'share' as const,
      token,
    };
  }, [favoriteId, token]);

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background text-foreground">
        <Loader2 className="w-6 h-6 animate-spin" />
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background text-foreground p-6">
        <div className="w-full max-w-xl rounded-2xl border border-border bg-card p-6 text-center">
          <p className="text-lg font-semibold">{error || t('share.invalidLink')}</p>
          <Link href="/login" className="inline-block mt-4 text-primary hover:opacity-80">
            {t('auth.login')}
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background text-foreground p-4 md:p-8">
      <div className="mx-auto max-w-4xl space-y-4">
        <div className="rounded-2xl border border-border bg-card p-6">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h1 className="text-2xl font-semibold">{data.knowledge_base.title}</h1>
              {data.knowledge_base.description ? (
                <p className="mt-2 text-muted-foreground">{data.knowledge_base.description}</p>
              ) : null}
            </div>
            <button
              type="button"
              onClick={() => void handleToggleFavorite()}
              disabled={favoriteLoading || !favoriteReady}
              className={`inline-flex items-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors disabled:opacity-60 ${
                isFavorite
                  ? 'border-primary/40 bg-primary/10 text-primary'
                  : 'border-border hover:bg-accent text-foreground'
              }`}
              title={isFavorite ? t('share.unfavorite') : t('share.favorite')}
            >
              {favoriteLoading ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Star className={`w-4 h-4 ${isFavorite ? 'fill-current' : ''}`} />
              )}
              {isFavorite ? t('share.unfavorite') : isLoggedIn ? t('share.favorite') : t('share.loginToFavorite')}
            </button>
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
            <span>
              {t('share.permission')}: {data.share.permission === 'full' ? t('share.full') : t('share.partial')}
            </span>
            <span>·</span>
            <span>
              {t('share.expire')}: {expiresText}
            </span>
          </div>
        </div>

        <div className="rounded-2xl border border-border bg-card p-6">
          <div className="flex items-center gap-2 mb-4">
            <FileText className="w-4 h-4" />
            <h2 className="text-lg font-semibold">{t('dashboard.documents')}</h2>
          </div>

          {data.sources.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('document.noDocuments')}</p>
          ) : (
            <div className="space-y-3">
              {data.sources.map((source) => (
                <div key={source.id} className="rounded-xl border border-border p-3">
                  <div className="font-medium text-sm">{source.file_name}</div>
                  <div className="text-xs text-muted-foreground mt-1">{source.status}</div>
                  {data.share.permission === 'full' && source.content ? (
                    <p className="mt-2 text-sm text-muted-foreground line-clamp-4">{source.content}</p>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </div>

        {isLoggedIn && chatSource ? (
          <div className="rounded-2xl border border-border bg-card h-[620px] overflow-hidden">
            <ChatPanel workspaceId={data.knowledge_base.id} sessionSource={chatSource} />
          </div>
        ) : (
          <div className="rounded-2xl border border-border bg-card p-6 flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Lock className="w-4 h-4" />
              <span>{t('share.loginRequiredHint')}</span>
            </div>
            <div className="flex items-center gap-2">
              <Link href="/register" className="px-3 py-2 rounded-lg border border-border hover:bg-accent text-sm">
                {t('auth.register')}
              </Link>
              <Link href="/login" className="px-3 py-2 rounded-lg bg-primary text-primary-foreground hover:opacity-90 text-sm">
                {t('auth.login')}
              </Link>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
