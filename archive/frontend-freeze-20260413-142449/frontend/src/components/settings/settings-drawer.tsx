'use client';

import Image from 'next/image';
import { useEffect, useRef, useState } from 'react';
import { X, Moon, User, Lock, Info, QrCode, Settings, AlertTriangle, CreditCard, LogOut } from 'lucide-react';
import { useTheme } from 'next-themes';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/stores/useAppStore';
import { authApi } from '@/lib/api/client';
import { WECHAT_LOGIN_UI_ENABLED } from '@/lib/feature-flags';
import { toast } from '@/components/ui/toaster';
import { ThemeToggle } from '@/components/theme-toggle';
import { LanguageToggle } from '@/components/language-toggle';
import { BillingPanel } from '@/components/settings/billing-panel';

interface SettingsDrawerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SettingsDrawer({ open, onOpenChange }: SettingsDrawerProps) {
  const { theme, resolvedTheme } = useTheme();
  const { t, i18n } = useTranslation();
  const [qrEnlarged, setQrEnlarged] = useState(false);
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  const [activeSection, setActiveSection] = useState<'main' | 'profile' | 'password' | 'billing' | 'wechat' | 'cancel'>('main');
  const [profileData, setProfileData] = useState({ full_name: '', email: '' });
  const [passwordData, setPasswordData] = useState({ oldPassword: '', newPassword: '', confirmPassword: '' });
  const [cancelEmail, setCancelEmail] = useState('');
  const [cancelCode, setCancelCode] = useState('');
  const [cancelTicket, setCancelTicket] = useState('');
  const [cancelCodeSent, setCancelCodeSent] = useState(false);
  const [saving, setSaving] = useState(false);

  const { user } = useAppStore();

  useEffect(() => {
    if (user) {
      setProfileData({
        full_name: user.full_name || '',
        email: user.email || '',
      });
      setCancelEmail(user.email || '');
    }
  }, [user]);

  const handleProfileSave = async () => {
    if (!profileData.full_name.trim()) {
      toast.error(t('validation.required'));
      return;
    }
    setSaving(true);
    try {
      const response = await authApi.updateProfile({ full_name: profileData.full_name });
      if (response.success) {
        toast.success(t('settings.profileUpdated'));
        setActiveSection('main');
      } else {
        throw new Error(response.error || t('errors.serverError'));
      }
    } catch (err: any) {
      toast.error(err.message || t('errors.networkError'));
    } finally {
      setSaving(false);
    }
  };

  const handlePasswordChange = async () => {
    if (!passwordData.oldPassword) {
      toast.error(t('validation.required'));
      return;
    }
    if (!passwordData.newPassword) {
      toast.error(t('validation.required'));
      return;
    }
    if (passwordData.newPassword.length < 6) {
      toast.error(t('validation.passwordMinLength'));
      return;
    }
    if (passwordData.newPassword !== passwordData.confirmPassword) {
      toast.error(t('validation.passwordMismatch'));
      return;
    }

    setSaving(true);
    try {
      const response = await authApi.changePassword(passwordData.oldPassword, passwordData.newPassword);
      if (response.success) {
        toast.success(t('settings.passwordChanged'));
        setPasswordData({ oldPassword: '', newPassword: '', confirmPassword: '' });
        setActiveSection('main');
      } else {
        throw new Error(response.error || t('errors.serverError'));
      }
    } catch (err: any) {
      toast.error(err.message || t('errors.networkError'));
    } finally {
      setSaving(false);
    }
  };

  useEffect(() => {
    if (!open) {
      return;
    }

    closeButtonRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        if (qrEnlarged) {
          setQrEnlarged(false);
        } else {
          onOpenChange(false);
        }
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onOpenChange, open, qrEnlarged]);

  if (!open) return null;

  return (
    <>
      <div className="fixed inset-0 z-50 bg-black/60" onClick={() => onOpenChange(false)} />

      <div
        role="dialog"
        aria-modal="true"
        aria-label={t('settings.title')}
        className="fixed right-0 top-0 z-50 flex h-full w-80 flex-col border-l border-border bg-card text-foreground animate-in slide-in-from-right duration-200"
      >
        <div className="flex items-center justify-between border-b border-border p-4">
          <h2 className="flex items-center gap-2 text-lg font-semibold">
            <Settings className="h-5 w-5" />
            {t('settings.title')}
          </h2>
          <button
            ref={closeButtonRef}
            onClick={() => onOpenChange(false)}
            className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            aria-label={t('common.cancel')}
            type="button"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="flex-1 overflow-auto p-4">
          {activeSection === 'main' && (
            <div className="space-y-6">
              <section>
                <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-muted-foreground">
                  <Moon className="h-4 w-4" />
                  {t('settings.appearance')}
                </h3>
                <ThemeToggle />
                {theme === 'system' && (
                  <p className="mt-2 text-sm text-muted-foreground">
                    {t('settings.currentSystem')}: {resolvedTheme === 'dark' ? t('theme.dark') : t('theme.light')}
                  </p>
                )}
              </section>

              <section>
                <LanguageToggle />
              </section>

              <section>
                <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-muted-foreground">
                  <User className="h-4 w-4" />
                  {t('settings.account')}
                </h3>
                <div className="space-y-2">
                  <button
                    onClick={() => setActiveSection('profile')}
                    className="w-full rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
                    type="button"
                  >
                    <div className="flex items-center gap-3">
                      <User className="h-4 w-4" />
                      {t('settings.profile')}
                    </div>
                  </button>
                  <button
                    onClick={() => setActiveSection('password')}
                    className="w-full rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
                    type="button"
                  >
                    <div className="flex items-center gap-3">
                      <Lock className="h-4 w-4" />
                      {t('settings.changePassword')}
                      </div>
                  </button>
                  <button
                    onClick={() => setActiveSection('billing')}
                    className="w-full rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
                    type="button"
                  >
                    <div className="flex items-center gap-3">
                      <CreditCard className="h-4 w-4" />
                      {t('settings.billing')}
                    </div>
                  </button>
                  {WECHAT_LOGIN_UI_ENABLED && (
                    <button
                      onClick={() => setActiveSection('wechat')}
                      className="w-full rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
                      type="button"
                    >
                      <div className="flex items-center gap-3">
                        <QrCode className="h-4 w-4" />
                        {t('auth.wechatNotBound')}
                      </div>
                    </button>
                  )}
                  <button
                    onClick={() => setActiveSection('cancel')}
                    className="w-full rounded-lg px-3 py-2.5 text-left text-destructive transition-colors hover:bg-destructive/10"
                    type="button"
                  >
                    <div className="flex items-center gap-3">
                      <AlertTriangle className="h-4 w-4" />
                      {t('auth.accountCancel')}
                    </div>
                  </button>
                </div>
              </section>

              <section>
                <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-muted-foreground">
                  <Info className="h-4 w-4" />
                  {t('settings.about')}
                </h3>
                <div className="space-y-2">
                  <div className="px-3 py-2.5 text-sm text-muted-foreground">Context OS v1.0.0</div>
                  <a
                    href="/help"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors hover:bg-accent"
                  >
                    <Info className="h-4 w-4" />
                    {t('settings.helpDoc')}
                  </a>
                </div>
              </section>

              <section>
                <h3 className="mb-3 flex items-center gap-2 text-sm font-medium text-muted-foreground">
                  <QrCode className="h-4 w-4" />
                  {t('settings.contactDeveloper')}
                </h3>
                <div className="flex justify-center">
                  <button onClick={() => setQrEnlarged(true)} className="group relative" type="button">
                    <div className="h-48 w-48 rounded-xl bg-white p-3 shadow-lg shadow-indigo-500/20 transition-transform group-hover:scale-105">
                      <Image src="/images/qrcode.png" alt={t('settings.contactDeveloper')} width={180} height={180} className="h-full w-full object-contain" />
                    </div>
                    <div className="absolute inset-0 flex items-center justify-center rounded-xl bg-black/60 opacity-0 transition-opacity group-hover:opacity-100">
                      <span className="text-sm font-medium text-white">{t('settings.clickToZoom')}</span>
                    </div>
                  </button>
                </div>
                <p className="mt-3 text-center text-sm text-muted-foreground">{t('settings.scanWechat')}</p>
              </section>
            </div>
          )}

          {activeSection === 'profile' && (
            <div className="space-y-4">
              <button
                onClick={() => setActiveSection('main')}
                className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
                type="button"
              >
                ← {t('common.back')}
              </button>
              <h3 className="text-base font-semibold">{t('settings.profile')}</h3>
              <div className="space-y-4">
                <div>
                  <label htmlFor="profile-full-name" className="mb-2 block text-sm text-muted-foreground">
                    {t('auth.fullName')}
                  </label>
                  <input
                    id="profile-full-name"
                    name="fullName"
                    type="text"
                    autoComplete="name"
                    value={profileData.full_name}
                    onChange={(e) => setProfileData({ ...profileData, full_name: e.target.value })}
                    className="w-full rounded-lg border border-border bg-background px-3 py-2.5 focus:outline-none focus:ring-2 focus:ring-primary"
                    placeholder={t('auth.fullName')}
                  />
                </div>
                <div>
                  <label htmlFor="profile-email" className="mb-2 block text-sm text-muted-foreground">
                    {t('auth.email')}
                  </label>
                  <input
                    id="profile-email"
                    name="email"
                    type="email"
                    autoComplete="email"
                    value={profileData.email}
                    disabled
                    className="w-full cursor-not-allowed rounded-lg border border-border bg-muted px-3 py-2.5 text-muted-foreground"
                  />
                  <p className="mt-1 text-sm text-muted-foreground">{t('settings.emailImmutable')}</p>
                </div>
                <button
                  onClick={handleProfileSave}
                  disabled={saving}
                  className="w-full rounded-lg bg-primary py-2.5 text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-50"
                  type="button"
                >
                  {saving ? t('settings.saveLoading') : t('common.save')}
                </button>
              </div>
            </div>
          )}

          {activeSection === 'password' && (
            <div className="space-y-4">
              <button
                onClick={() => setActiveSection('main')}
                className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
                type="button"
              >
                ← {t('common.back')}
              </button>
              <h3 className="text-base font-semibold">{t('settings.changePassword')}</h3>
              <div className="space-y-4">
                <div>
                  <label htmlFor="current-password" className="mb-2 block text-sm text-muted-foreground">
                    {t('auth.currentPassword')}
                  </label>
                  <input
                    id="current-password"
                    name="currentPassword"
                    type="password"
                    autoComplete="current-password"
                    value={passwordData.oldPassword}
                    onChange={(e) => setPasswordData({ ...passwordData, oldPassword: e.target.value })}
                    className="w-full rounded-lg border border-border bg-background px-3 py-2.5 focus:outline-none focus:ring-2 focus:ring-primary"
                    placeholder={t('auth.currentPassword')}
                  />
                </div>
                <div>
                  <label htmlFor="new-password-settings" className="mb-2 block text-sm text-muted-foreground">
                    {t('auth.newPassword')}
                  </label>
                  <input
                    id="new-password-settings"
                    name="newPassword"
                    type="password"
                    autoComplete="new-password"
                    value={passwordData.newPassword}
                    onChange={(e) => setPasswordData({ ...passwordData, newPassword: e.target.value })}
                    className="w-full rounded-lg border border-border bg-background px-3 py-2.5 focus:outline-none focus:ring-2 focus:ring-primary"
                    placeholder={t('auth.newPasswordPlaceholder')}
                  />
                </div>
                <div>
                  <label htmlFor="confirm-password-settings" className="mb-2 block text-sm text-muted-foreground">
                    {t('auth.confirmPassword')}
                  </label>
                  <input
                    id="confirm-password-settings"
                    name="confirmPassword"
                    type="password"
                    autoComplete="new-password"
                    value={passwordData.confirmPassword}
                    onChange={(e) => setPasswordData({ ...passwordData, confirmPassword: e.target.value })}
                    className="w-full rounded-lg border border-border bg-background px-3 py-2.5 focus:outline-none focus:ring-2 focus:ring-primary"
                    placeholder={t('auth.confirmPasswordPlaceholder')}
                  />
                </div>
                <button
                  onClick={handlePasswordChange}
                  disabled={saving}
                  className="w-full rounded-lg bg-primary py-2.5 text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-50"
                  type="button"
                >
                  {saving ? t('settings.updateLoading') : t('common.confirm')}
                </button>
              </div>
            </div>
          )}

          {activeSection === 'billing' && (
            <BillingPanel onBack={() => setActiveSection('main')} />
          )}

          {/* WeChat Binding View */}
          {WECHAT_LOGIN_UI_ENABLED && activeSection === 'wechat' && (
            <div className="space-y-4">
              <button
                onClick={() => setActiveSection('main')}
                className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
                type="button"
              >
                ← {t('common.back')}
              </button>
              <h3 className="text-base font-semibold">{t('auth.wechatNotBound')}</h3>
              <div className="rounded-lg bg-muted p-4 text-center">
                <QrCode className="mx-auto h-12 w-12 text-muted-foreground" />
                <p className="mt-2 text-sm text-muted-foreground">
                  {t('auth.scanQRCode')}
                </p>
              </div>
              <p className="text-sm text-muted-foreground">
                {t('auth.wechatNotBound')}
              </p>
            </div>
          )}

          {/* Account Cancellation View */}
          {activeSection === 'cancel' && (
            <div className="space-y-4">
              <button
                onClick={() => setActiveSection('main')}
                className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
                type="button"
              >
                ← {t('common.back')}
              </button>
              <h3 className="text-base font-semibold">{t('auth.accountCancel')}</h3>
              <div className="rounded-lg border border-destructive/20 bg-destructive/10 p-4">
                <p className="text-sm text-destructive">
                  {t('auth.cancelConfirm')}
                </p>
              </div>
              <div className="space-y-4">
                <div>
                  <label htmlFor="cancel-email" className="mb-2 block text-sm text-muted-foreground">
                    {t('auth.email')}
                  </label>
                  <input
                    id="cancel-email"
                    name="cancelEmail"
                    type="email"
                    autoComplete="email"
                    value={cancelEmail}
                    onChange={(e) => setCancelEmail(e.target.value)}
                    className="w-full rounded-lg border border-border bg-background px-3 py-2.5 focus:outline-none focus:ring-2 focus:ring-primary"
                    placeholder={t('auth.email')}
                  />
                </div>
                <div className="flex gap-2">
                  <input
                    id="cancel-code"
                    name="cancelCode"
                    type="text"
                    inputMode="numeric"
                    value={cancelCode}
                    onChange={(e) => setCancelCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                    className="w-full rounded-lg border border-border bg-background px-3 py-2.5 focus:outline-none focus:ring-2 focus:ring-primary"
                    placeholder={t('auth.verificationCode')}
                  />
                  <button
                    onClick={async () => {
                      if (!cancelEmail.trim()) {
                        toast.error(t('validation.required'));
                        return;
                      }
                      setSaving(true);
                      try {
                        const response = await authApi.accountCancelSendCode(
                          cancelEmail.trim(),
                          i18n.resolvedLanguage || 'zh'
                        );
                        if (!response.success) {
                          throw new Error(response.error || t('common.error'));
                        }
                        setCancelCodeSent(true);
                        toast.success(t('auth.resetCodeSent'));
                      } catch (err: any) {
                        toast.error(err.message || t('errors.networkError'));
                      } finally {
                        setSaving(false);
                      }
                    }}
                    disabled={saving}
                    className="whitespace-nowrap rounded-lg border border-border px-3 py-2.5 text-sm hover:bg-accent disabled:opacity-50"
                    type="button"
                  >
                    {t('auth.sendResetCode')}
                  </button>
                </div>
                <button
                  onClick={async () => {
                    if (!cancelEmail.trim() || !cancelCode.trim()) {
                      toast.error(t('validation.required'));
                      return;
                    }
                    setSaving(true);
                    try {
                      const response = await authApi.accountCancelVerifyCode(cancelEmail.trim(), cancelCode.trim());
                      const ticket = (response as any)?.data?.cancel_ticket || (response as any)?.data?.reset_ticket;
                      if (!response.success || !ticket) {
                        throw new Error((response as any)?.error || t('common.error'));
                      }
                      setCancelTicket(ticket);
                      toast.success(t('auth.verifyCode'));
                    } catch (err: any) {
                      toast.error(err.message || t('errors.networkError'));
                    } finally {
                      setSaving(false);
                    }
                  }}
                  disabled={saving || !cancelCodeSent}
                  className="w-full rounded-lg border border-border py-2.5 transition-colors hover:bg-accent disabled:opacity-50"
                  type="button"
                >
                  {t('auth.verifyCode')}
                </button>
                <button
                  onClick={async () => {
                    if (!cancelTicket) {
                      toast.error(t('auth.verifyCode'));
                      return;
                    }
                    if (!confirm(t('auth.cancelConfirm'))) {
                      return;
                    }
                    setSaving(true);
                    try {
                      const response = await authApi.accountCancel({
                        cancel_ticket: cancelTicket,
                        reset_ticket: cancelTicket,
                      });
                      if (response.success) {
                        toast.success(t('common.success'));
                        localStorage.removeItem('token');
                        window.location.href = '/login';
                      } else {
                        throw new Error(response.error || t('common.error'));
                      }
                    } catch (err: any) {
                      toast.error(err.message || t('errors.networkError'));
                    } finally {
                      setSaving(false);
                    }
                  }}
                  disabled={saving || !cancelTicket}
                  className="w-full rounded-lg bg-destructive py-2.5 text-destructive-foreground transition-opacity hover:opacity-90 disabled:opacity-50"
                  type="button"
                >
                  {saving ? t('common.loading') : t('auth.accountCancel')}
                </button>
              </div>
            </div>
          )}
        </div>

        <div className="border-t border-border p-4">
          <button
            onClick={() => {
              useAppStore.getState().clearUser();
              localStorage.removeItem('token');
              window.location.href = '/login';
            }}
            className="flex w-full items-center justify-center gap-2 rounded-lg border border-red-500/30 px-4 py-2 text-red-400 transition-colors hover:bg-red-500/10"
            type="button"
          >
            <LogOut className="h-4 w-4" />
            {t('auth.logout')}
          </button>
        </div>
      </div>

      {qrEnlarged && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/80 p-8" onClick={() => setQrEnlarged(false)}>
          <div className="relative w-full max-w-lg">
            <button onClick={() => setQrEnlarged(false)} className="absolute -top-12 right-0 text-white/70 hover:text-white" type="button">
              <X className="h-8 w-8" />
            </button>
            <div className="rounded-2xl bg-white p-6">
              <Image src="/images/qrcode.png" alt={t('settings.contactDeveloper')} width={600} height={600} className="h-auto w-full" />
            </div>
            <p className="mt-4 text-center text-white">{t('settings.scanWechat')}</p>
          </div>
        </div>
      )}
    </>
  );
}
