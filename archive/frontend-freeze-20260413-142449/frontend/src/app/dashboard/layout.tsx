'use client';

import { useEffect, useState } from 'react';
import { useRouter, usePathname } from 'next/navigation';
import { useTranslation } from 'react-i18next';
import { Bot, KeyRound, Search, Settings, Loader2, Menu, X, Home, Share2, Trash2, UserPlus, Users, Shield } from 'lucide-react';
import { useAppStore } from '@/stores/useAppStore';
import { SearchDialog } from '@/components/omnibar/search-dialog';
import { SettingsDrawer } from '@/components/settings/settings-drawer';
import { APIAccessModal } from '@/components/dashboard/api-access-modal';
import { NotificationCenter } from '@/components/dashboard/notification-center';
import { authApi, clearAuthToken, getCachedAuthUser, hasUsableAuthToken, kbApi } from '@/lib/api/client';
import { toast } from '@/components/ui/toaster';
import type { ShareMember } from '@/types';

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const { t } = useTranslation();
  const { currentWorkspace, toggleSearchDialog, user, setUser, clearUser, clearCurrentWorkspace } = useAppStore();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [shareOpen, setShareOpen] = useState(false);
  const [apiAccessOpen, setAPIAccessOpen] = useState(false);
  const [sharePermission, setSharePermission] = useState<'full' | 'partial'>('partial');
  const [shareExpireHours, setShareExpireHours] = useState<number>(24);
  const [sharing, setSharing] = useState(false);
  const [shareLink, setShareLink] = useState('');
  const [shareRefreshTick, setShareRefreshTick] = useState(0);
  const [copyingShareLink, setCopyingShareLink] = useState(false);
  const [shareAccessLevel, setShareAccessLevel] = useState<'private' | 'link' | 'public'>('private');
  const [shareMembers, setShareMembers] = useState<ShareMember[]>([]);
  const [loadingShareSettings, setLoadingShareSettings] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState<'viewer' | 'editor'>('viewer');
  const [invitingMember, setInvitingMember] = useState(false);
  const [removingMemberId, setRemovingMemberId] = useState<string | null>(null);
  const [checkingAuth, setCheckingAuth] = useState(true);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const isWorkspaceDetail = pathname.startsWith('/dashboard/') && pathname !== '/dashboard/search';

  useEffect(() => {
    let cancelled = false;

    const bootstrapAuth = async () => {
      if (user) {
        setCheckingAuth(false);
        return;
      }

      try {
        const response = await authApi.me();
        if (response.success && response.data?.user) {
          if (!cancelled) {
            setUser(response.data.user);
            setCheckingAuth(false);
          }
          return;
        }
        if (response.error_code === 'UNAUTHORIZED' || !hasUsableAuthToken()) {
          clearAuthToken();
          clearUser();
          router.replace('/login');
          return;
        }

        const cachedUser = getCachedAuthUser();
        if (!cancelled) {
          if (cachedUser) {
            setUser(cachedUser);
          }
          setCheckingAuth(false);
          toast.error(t('errors.networkError'));
        }
      } catch {
        if (!cancelled) {
          setCheckingAuth(false);
          toast.error(t('errors.networkError'));
        }
      }
    };

    void bootstrapAuth();

    return () => {
      cancelled = true;
    };
  }, [clearUser, router, setUser, t, user]);

  // Clear workspace when leaving workspace pages
  useEffect(() => {
    // If pathname is /dashboard or /dashboard/search, clear the workspace
    if (pathname === '/dashboard' || pathname === '/dashboard/search') {
      clearCurrentWorkspace();
    }
  }, [pathname, clearCurrentWorkspace]);

  useEffect(() => {
    if (!shareOpen || !currentWorkspace) {
      return;
    }

    let cancelled = false;

    const generateShareLink = async () => {
      setSharing(true);
      setShareLink('');
      try {
        const response = await kbApi.createShare(currentWorkspace.id, {
          permission: sharePermission,
          expire_in_hours: shareExpireHours,
        });
        if (!response?.success || !response?.data?.share_url) {
          throw new Error(response?.error || 'share-create-failed');
        }
        const nextShareLink = `${window.location.origin}${response.data.share_url}`;
        if (!cancelled) {
          setShareLink(nextShareLink);
        }
      } catch {
        if (!cancelled) {
          toast.error(t('share.createFailed'));
        }
      } finally {
        if (!cancelled) {
          setSharing(false);
        }
      }
    };

    void generateShareLink();

    return () => {
      cancelled = true;
    };
  }, [currentWorkspace, shareExpireHours, shareOpen, sharePermission, shareRefreshTick, t]);

  useEffect(() => {
    if (!shareOpen || !currentWorkspace) {
      return;
    }
    let cancelled = false;
    const loadShareSettings = async () => {
      setLoadingShareSettings(true);
      try {
        const response = await kbApi.getShareSettings(currentWorkspace.id);
        if (cancelled) return;
        if (!response.success || !response.data) {
          toast.error(response.error || t('share.createFailed'));
          return;
        }
        setShareAccessLevel(
          (response.data.access_level as 'private' | 'link' | 'public') || 'private',
        );
        setShareMembers(response.data.members || []);
      } finally {
        if (!cancelled) {
          setLoadingShareSettings(false);
        }
      }
    };
    void loadShareSettings();
    return () => {
      cancelled = true;
    };
  }, [shareOpen, currentWorkspace, shareRefreshTick, t]);

  const handleCopyShareLink = async () => {
    if (!shareLink) {
      return;
    }
    if (!navigator.clipboard?.writeText) {
      toast.error(t('share.copyFailed'));
      return;
    }

    setCopyingShareLink(true);
    try {
      await navigator.clipboard.writeText(shareLink);
      toast.success(t('share.linkCopied'));
    } catch {
      toast.error(t('share.copyFailed'));
    } finally {
      setCopyingShareLink(false);
    }
  };

  const handleUpdateAccessLevel = async (nextLevel: 'private' | 'link' | 'public') => {
    if (!currentWorkspace) return;
    setShareAccessLevel(nextLevel);
    const response = await kbApi.updateAccessLevel(currentWorkspace.id, nextLevel);
    if (!response.success) {
      toast.error(response.error || t('share.createFailed'));
      return;
    }
    setShareRefreshTick((value) => value + 1);
  };

  const handleInviteMember = async () => {
    if (!currentWorkspace || !inviteEmail.trim()) return;
    setInvitingMember(true);
    try {
      const response = await kbApi.inviteMember(currentWorkspace.id, inviteEmail.trim(), inviteRole);
      if (!response.success || !response.data) {
        toast.error(response.error || t('share.createFailed'));
        return;
      }
      setShareMembers((prev) => [...prev, response.data]);
      setInviteEmail('');
      setInviteRole('viewer');
    } finally {
      setInvitingMember(false);
    }
  };

  const handleRemoveMember = async (memberId: string) => {
    if (!currentWorkspace) return;
    setRemovingMemberId(memberId);
    try {
      const response = await kbApi.removeMember(currentWorkspace.id, memberId);
      if (!response.success) {
        toast.error(response.error || t('share.createFailed'));
        return;
      }
      setShareMembers((prev) => prev.filter((item) => item.id !== memberId));
    } finally {
      setRemovingMemberId(null);
    }
  };

  if (checkingAuth) {
    return (
      <div className="flex h-screen items-center justify-center bg-background text-foreground">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col bg-background text-foreground pt-3 px-2 md:px-3">
      <header className="relative h-14 shrink-0 rounded-2xl border border-border bg-card/84 backdrop-blur-xl px-4 md:px-6 shadow-[var(--shadow-sm)]">
        <div className="h-full flex items-center justify-between gap-3">
          <div className="flex items-center gap-3 min-w-0">
            {/* Mobile menu button */}
            <button
              onClick={() => setMobileMenuOpen(true)}
              className="md:hidden p-2 -ml-2 rounded-lg hover:bg-accent transition-colors"
              aria-label={t('dashboard.openMenu')}
            >
              <Menu className="w-5 h-5" />
            </button>

            {/* Brand */}
            <div className="flex items-center gap-2 min-w-0">
              <button
                onClick={() => router.push('/dashboard')}
                className="flex p-1.5 -ml-1.5 rounded-lg hover:bg-accent transition-colors"
                aria-label={t('dashboard.backToHome')}
                title={t('dashboard.backToHome')}
              >
                <Home className="w-4 h-4" />
              </button>
              <div className="w-8 h-8 rounded-xl bg-[linear-gradient(135deg,rgba(124,58,237,0.92),rgba(99,102,241,0.8))] shadow-[var(--shadow-sm)] flex items-center justify-center">
                <Bot className="w-4 h-4 text-white" />
              </div>
              <span className="font-semibold text-lg hidden sm:inline">Context OS</span>
            </div>

            {currentWorkspace && (
              <>
                <span className="text-muted-foreground hidden md:inline">/</span>
                <span className="text-sm font-medium text-foreground truncate max-w-[180px] md:max-w-none">
                  {currentWorkspace.title}
                </span>
              </>
            )}
          </div>

          <div className="flex items-center justify-end gap-2 md:gap-3">
            <NotificationCenter />

            <button
              onClick={() => {
                if (!currentWorkspace) {
                  toast.error(t('workspace.select'));
                  return;
                }
                setAPIAccessOpen(true);
              }}
              className="p-2 rounded-lg hover:bg-accent transition-colors text-muted-foreground hover:text-foreground"
              title={t('dashboard.apiAccess')}
            >
              <KeyRound className="w-5 h-5" />
            </button>

            <button
              onClick={() => {
                if (!currentWorkspace) {
                  toast.error(t('workspace.select'));
                  return;
                }
                setShareOpen(true);
              }}
              className="p-2 rounded-lg hover:bg-accent transition-colors text-muted-foreground hover:text-foreground"
              title={t('dashboard.share')}
            >
              <Share2 className="w-5 h-5" />
            </button>

            {isWorkspaceDetail && (
              <button
                onClick={() => window.dispatchEvent(new Event('dashboard:clear-chat'))}
                className="p-2 rounded-lg hover:bg-accent transition-colors text-muted-foreground hover:text-foreground"
                title={t('chat.clearHistory')}
              >
                <Trash2 className="w-5 h-5" />
              </button>
            )}

            <button
              onClick={() => setSettingsOpen(true)}
              className="p-2 rounded-lg hover:bg-accent transition-colors text-muted-foreground hover:text-foreground"
              title={t('settings.title')}
            >
              <Settings className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Centered global search (desktop) */}
        <div className="hidden md:flex pointer-events-none absolute inset-0 items-center justify-center px-28 lg:px-40">
          <button
            onClick={toggleSearchDialog}
            className="pointer-events-auto w-full max-w-[560px] flex items-center gap-2 px-3 py-2 rounded-2xl border border-border bg-background/62 hover:bg-accent/60 transition-colors text-muted-foreground hover:text-foreground shadow-[var(--shadow-sm)]"
          >
            <Search className="w-4 h-4 shrink-0" />
            <span className="text-sm truncate">{t('common.search')}</span>
            <kbd className="ml-auto px-1.5 py-0.5 text-xs bg-accent rounded text-muted-foreground">
              ⌘K
            </kbd>
          </button>
        </div>
      </header>

      {/* Mobile Slide-out Menu */}
      {mobileMenuOpen && (
        <div className="fixed inset-0 z-50 md:hidden">
          {/* Backdrop */}
          <div 
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setMobileMenuOpen(false)}
          />
          
          {/* Menu panel */}
          <div className="absolute left-0 top-0 bottom-0 w-72 bg-card/94 border-r border-border shadow-[var(--shadow-lg)] backdrop-blur-xl animate-in slide-in-from-left duration-300">
            <div className="p-4 border-b border-border flex items-center justify-between">
              <span className="font-semibold text-lg">{t('dashboard.menu')}</span>
              <button
                onClick={() => setMobileMenuOpen(false)}
                className="p-2 rounded-lg hover:bg-accent"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
            
            <nav className="p-4 space-y-2">
              <button
                onClick={() => {
                  setMobileMenuOpen(false);
                  router.push('/dashboard');
                }}
                className="w-full flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-accent transition-colors text-left min-h-[44px]"
              >
                <Home className="w-5 h-5" />
                <span>{t('workspace.list')}</span>
              </button>
              
              <button
                onClick={() => {
                  setMobileMenuOpen(false);
                  toggleSearchDialog();
                }}
                className="w-full flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-accent transition-colors text-left min-h-[44px]"
              >
                <Search className="w-5 h-5" />
                <span>{t('common.search')}</span>
              </button>
              
              <button
                onClick={() => {
                  setMobileMenuOpen(false);
                  setSettingsOpen(true);
                }}
                className="w-full flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-accent transition-colors text-left min-h-[44px]"
              >
                <Settings className="w-5 h-5" />
                <span>{t('settings.title')}</span>
              </button>
            </nav>
          </div>
        </div>
      )}

      <main className="flex-1 overflow-hidden pt-2">{children}</main>

      {shareOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
          <div className="w-full max-w-2xl rounded-3xl border border-border bg-card/94 shadow-[var(--shadow-lg)] backdrop-blur-xl">
            <div className="p-4 border-b border-border flex items-center justify-between">
              <h3 className="text-base font-semibold">{t('share.title')}</h3>
              <button
                className="p-2 rounded-lg hover:bg-accent"
                onClick={() => setShareOpen(false)}
                aria-label={t('common.close')}
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="grid gap-4 p-4 md:grid-cols-[1.1fr_0.9fr]">
              <div className="space-y-4">
                <div className="rounded-2xl border border-border bg-background/45 p-4 shadow-[var(--shadow-sm)]">
                  <div className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
                    <Share2 className="h-4 w-4" />
                    {t('share.title')}
                  </div>
                  <div className="space-y-3">
                    <div>
                      <label className="text-sm text-muted-foreground">{t('share.permission')}</label>
                      <select
                        value={sharePermission}
                        onChange={(e) => setSharePermission(e.target.value as 'full' | 'partial')}
                        className="mt-1 w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 shadow-[var(--shadow-sm)]"
                      >
                        <option value="partial">{t('share.partial')}</option>
                        <option value="full">{t('share.full')}</option>
                      </select>
                    </div>
                    <div>
                      <label className="text-sm text-muted-foreground">{t('share.expire')}</label>
                      <select
                        value={String(shareExpireHours)}
                        onChange={(e) => setShareExpireHours(Number(e.target.value))}
                        className="mt-1 w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 shadow-[var(--shadow-sm)]"
                      >
                        <option value="24">{t('share.expire24h')}</option>
                        <option value="168">{t('share.expire7d')}</option>
                        <option value="0">{t('share.neverExpire')}</option>
                      </select>
                    </div>
                    <div>
                      <label className="text-sm text-muted-foreground">{t('share.link')}</label>
                      <div className="mt-1 flex items-center gap-2">
                        <input
                          type="text"
                          readOnly
                          value={shareLink}
                          placeholder={sharing ? t('share.generatingLink') : ''}
                          className="w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 text-sm font-mono text-foreground shadow-[var(--shadow-sm)]"
                        />
                        <button
                          onClick={() => void handleCopyShareLink()}
                          disabled={sharing || !shareLink || copyingShareLink}
                          className="shrink-0 px-3 py-2 rounded-xl border border-border hover:bg-accent text-sm disabled:opacity-60"
                        >
                          {copyingShareLink ? <Loader2 className="w-4 h-4 animate-spin" /> : t('share.copyLink')}
                        </button>
                      </div>
                    </div>
                    <div className="text-xs text-muted-foreground">{t('share.loginRequiredHint')}</div>
                    <div className="flex justify-end gap-2">
                      <button
                        onClick={() => setShareOpen(false)}
                        className="px-3 py-2 rounded-xl border border-border hover:bg-accent text-sm"
                      >
                        {t('common.cancel')}
                      </button>
                      <button
                        onClick={() => setShareRefreshTick((value) => value + 1)}
                        disabled={sharing}
                        className="px-3 py-2 rounded-xl bg-primary text-primary-foreground hover:opacity-90 text-sm disabled:opacity-60 shadow-[var(--shadow-sm)]"
                      >
                        {sharing ? <Loader2 className="w-4 h-4 animate-spin" /> : t('share.generateLink')}
                      </button>
                    </div>
                  </div>
                </div>

                <div className="rounded-2xl border border-border bg-background/45 p-4 shadow-[var(--shadow-sm)]">
                  <div className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
                    <UserPlus className="h-4 w-4" />
                    Invite Member
                  </div>
                  <div className="space-y-3">
                    <input
                      type="email"
                      value={inviteEmail}
                      onChange={(e) => setInviteEmail(e.target.value)}
                      placeholder="member@example.com"
                      className="w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 text-sm shadow-[var(--shadow-sm)]"
                    />
                    <select
                      value={inviteRole}
                      onChange={(e) => setInviteRole(e.target.value as 'viewer' | 'editor')}
                      className="w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 text-sm shadow-[var(--shadow-sm)]"
                    >
                      <option value="viewer">Viewer</option>
                      <option value="editor">Editor</option>
                    </select>
                    <button
                      onClick={() => void handleInviteMember()}
                      disabled={invitingMember || !inviteEmail.trim()}
                      className="w-full rounded-xl bg-primary px-3 py-2.5 text-sm text-primary-foreground disabled:opacity-60"
                    >
                      {invitingMember ? <Loader2 className="mx-auto h-4 w-4 animate-spin" /> : 'Invite member'}
                    </button>
                  </div>
                </div>
              </div>

              <div className="space-y-4">
                <div className="rounded-2xl border border-border bg-background/45 p-4 shadow-[var(--shadow-sm)]">
                  <div className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
                    <Shield className="h-4 w-4" />
                    Workspace Access
                  </div>
                  <select
                    value={shareAccessLevel}
                    onChange={(e) => void handleUpdateAccessLevel(e.target.value as 'private' | 'link' | 'public')}
                    className="w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 text-sm shadow-[var(--shadow-sm)]"
                  >
                    <option value="private">Private</option>
                    <option value="link">Link only</option>
                    <option value="public">Public</option>
                  </select>
                  {loadingShareSettings && (
                    <div className="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
                      <Loader2 className="h-3 w-3 animate-spin" />
                      Loading sharing settings...
                    </div>
                  )}
                </div>

                <div className="rounded-2xl border border-border bg-background/45 p-4 shadow-[var(--shadow-sm)]">
                  <div className="mb-3 flex items-center gap-2 text-sm font-medium text-foreground">
                    <Users className="h-4 w-4" />
                    Members
                  </div>
                  <div className="space-y-2">
                    {shareMembers.length === 0 ? (
                      <div className="rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                        No members yet.
                      </div>
                    ) : (
                      shareMembers.map((member) => (
                        <div
                          key={member.id}
                          className="flex items-center justify-between rounded-xl border border-border bg-background/55 px-3 py-2 text-sm"
                        >
                          <div className="min-w-0">
                            <div className="truncate font-medium text-foreground">
                              {member.email || member.user_id || member.id}
                            </div>
                            <div className="text-xs text-muted-foreground">
                              {member.access_level} · {member.invite_status}
                            </div>
                          </div>
                          <button
                            onClick={() => void handleRemoveMember(member.id)}
                            disabled={removingMemberId === member.id}
                            className="rounded-lg px-2 py-1 text-xs text-destructive hover:bg-destructive/10 disabled:opacity-60"
                          >
                            {removingMemberId === member.id ? <Loader2 className="h-3 w-3 animate-spin" /> : 'Remove'}
                          </button>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      <SearchDialog />
      <APIAccessModal open={apiAccessOpen} workspace={currentWorkspace} onOpenChange={setAPIAccessOpen} />
      <SettingsDrawer open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
}
