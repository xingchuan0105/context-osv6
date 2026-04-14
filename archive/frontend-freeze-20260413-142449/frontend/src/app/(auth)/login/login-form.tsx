'use client';

import { useState, useEffect, useCallback, useMemo } from 'react';
import Link from 'next/link';
import { useRouter, useSearchParams } from 'next/navigation';
import { Mail, Lock, Loader2, ArrowLeft, Check, Timer } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { authApi } from '@/lib/api/auth';
import { useAppStore } from '@/stores/useAppStore';
import { Button } from '@/components/ui/button';
import { toast } from '@/components/ui/toaster';
import { Input } from '@/components/ui/input';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { FormField } from '@/components/ui/form';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { WechatQR } from '@/components/auth/wechat-qr';
import { getTranslatedError, isWechatNotConfiguredError } from '@/lib/errors';
import { WECHAT_LOGIN_UI_ENABLED } from '@/lib/feature-flags';

// Password reset flow views
type ResetView = 'email' | 'verify' | 'newPassword';
type View = 'login' | 'forgot' | 'wechat-bind';

interface FormErrors {
  form?: string;
  email?: string;
  password?: string;
  code?: string;
  newPassword?: string;
  confirmPassword?: string;
}

const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const CODE_LENGTH = 6;
const RESEND_COOLDOWN = 60; // seconds

export function LoginForm() {
  const { t } = useTranslation();
  const router = useRouter();
  const searchParams = useSearchParams();
  const { setUser } = useAppStore();
  const redirectPath = useMemo(() => {
    const next = (searchParams.get('next') || '').trim();
    if (next.startsWith('/') && !next.startsWith('//')) {
      return next;
    }
    return '/dashboard';
  }, [searchParams]);

  const [view, setView] = useState<View>('login');
  const [resetView, setResetView] = useState<ResetView>('email');
  
  // Login form
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  
  // Password reset form
  const [resetEmail, setResetEmail] = useState('');
  const [code, setCode] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [resetTicket, setResetTicket] = useState('');
  
  // State
  const [emailSent, setEmailSent] = useState(false);
  const [errors, setErrors] = useState<FormErrors>({});
  const [loading, setLoading] = useState(false);
  
  // Countdown for resend
  const [countdown, setCountdown] = useState(0);

  // WeChat QR login state
  const [wechatQrOpen, setWechatQrOpen] = useState(false);
  const [wechatLoginId, setWechatLoginId] = useState('');
  const [wechatQrUrl, setWechatQrUrl] = useState('');
  const [wechatBindTicket, setWechatBindTicket] = useState('');

  // Countdown timer effect
  useEffect(() => {
    if (countdown <= 0) return;
    const timer = setTimeout(() => setCountdown(countdown - 1), 1000);
    return () => clearTimeout(timer);
  }, [countdown]);

  const validateEmail = (value: string) => {
    if (!value) {
      return t('validation.required');
    }
    if (!emailRegex.test(value)) {
      return t('validation.invalidEmail');
    }
    return undefined;
  };

  const validateCode = (value: string) => {
    if (!value) {
      return t('validation.required');
    }
    if (!/^\d{6}$/.test(value)) {
      return t('auth.enterVerificationCode');
    }
    return undefined;
  };

  const validateNewPasswords = () => {
    if (!newPassword) {
      return { newPassword: t('validation.required') };
    }
    if (newPassword.length < 6) {
      return { newPassword: t('validation.passwordMinLength') };
    }
    if (newPassword !== confirmPassword) {
      return { confirmPassword: t('auth.passwordsDoNotMatch') };
    }
    return {};
  };

  const clearFormError = () => {
    setErrors((prev) => ({ ...prev, form: undefined }));
  };

  const resolveErrorMessage = useCallback((raw: string | undefined, fallbackKey: string): string => {
    if (!raw) return t(fallbackKey);
    if (isWechatNotConfiguredError(raw)) {
      return t('auth.wechatNotConfiguredHelp');
    }
    const translated = getTranslatedError(raw, t);
    return translated || t(fallbackKey);
  }, [t]);

  // ========== Login handlers ==========
  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    const emailError = validateEmail(email);
    const passwordError = !password ? t('validation.required') : undefined;
    if (emailError || passwordError) {
      setErrors({ email: emailError, password: passwordError });
      return;
    }

    setLoading(true);
    setErrors({});

    try {
      const response = await authApi.login(email, password);
      if (response.success && response.data) {
        localStorage.setItem('token', response.data.token);
        setUser(response.data.user);
        toast.success(t('auth.loginSuccess'));
        router.replace(redirectPath);
      } else {
        setErrors({ form: resolveErrorMessage(response.error, 'auth.loginFailed') });
      }
    } catch (err: any) {
      setErrors({ form: resolveErrorMessage(err?.message, 'errors.networkError') });
    } finally {
      setLoading(false);
    }
  };

  // ========== WeChat QR login handlers ==========
  const handleStartWechatQR = async () => {
    setLoading(true);
    setErrors({});

    try {
      const response = await authApi.wechatStartQR();
      if (response.success && response.data?.login_id) {
        setWechatLoginId(response.data.login_id);
        setWechatQrUrl(response.data.qr_url || '');
        setWechatQrOpen(true);
      } else {
        setErrors({ form: resolveErrorMessage(response.error, 'common.error') });
      }
    } catch (err: any) {
      setErrors({ form: resolveErrorMessage(err?.message, 'errors.networkError') });
    } finally {
      setLoading(false);
    }
  };

  const handleWechatSuccess = (token: string) => {
    localStorage.setItem('token', token);
    // Fetch user info after successful login
    authApi.me().then((response) => {
      if (response.success && response.data?.user) {
        setUser(response.data.user);
        toast.success(t('auth.loginSuccess'));
        router.replace(redirectPath);
      }
    });
  };

  const handleWechatNeedBind = (bindTicket: string) => {
    setWechatQrOpen(false);
    setWechatBindTicket(bindTicket);
    setView('wechat-bind');
  };

  const handleWechatError = (error: string) => {
    setErrors({ form: resolveErrorMessage(error, 'auth.exchangeFailed') });
    setWechatQrOpen(false);
  };

  // ========== WeChat bind handlers ==========
  const handleWechatBindExisting = async (e: React.FormEvent) => {
    e.preventDefault();
    const emailError = validateEmail(email);
    const passwordError = !password ? t('validation.required') : undefined;
    if (emailError || passwordError) {
      setErrors({ email: emailError, password: passwordError });
      return;
    }

    setLoading(true);
    setErrors({});

    try {
      const response = await authApi.wechatBindExisting(wechatBindTicket, email, password);
      if (response.success && response.data?.token) {
        localStorage.setItem('token', response.data.token);
        setUser(response.data.user);
        toast.success(t('auth.bindSuccess'));
        router.replace(redirectPath);
      } else {
        setErrors({ form: resolveErrorMessage(response.error, 'auth.bindFailed') });
      }
    } catch (err: any) {
      setErrors({ form: resolveErrorMessage(err?.message, 'errors.networkError') });
    } finally {
      setLoading(false);
    }
  };

  // ========== Password reset handlers ==========
  const handleSendCode = async () => {
    const emailError = validateEmail(resetEmail);
    if (emailError) {
      setErrors({ email: emailError });
      return;
    }

    setLoading(true);
    setErrors({});

    try {
      const response = await authApi.sendResetCode(resetEmail);
      if (response.success) {
        setEmailSent(true);
        setCountdown(RESEND_COOLDOWN);
        toast.success(t('auth.resetCodeSent'));
      } else {
        setErrors({ form: resolveErrorMessage(response.error, 'common.error') });
      }
    } catch (err: any) {
      setErrors({ form: resolveErrorMessage(err?.message, 'errors.networkError') });
    } finally {
      setLoading(false);
    }
  };

  const handleVerifyCode = async () => {
    const codeError = validateCode(code);
    if (codeError) {
      setErrors({ code: codeError });
      return;
    }

    setLoading(true);
    setErrors({});

    try {
      const response = await authApi.verifyResetCode(resetEmail, code);
      if (response.success && response.data?.reset_ticket) {
        setResetTicket(response.data.reset_ticket);
        setResetView('newPassword');
        toast.success(t('auth.tokenValid'));
      } else {
        setErrors({ code: resolveErrorMessage(response.error, 'auth.resetCodeInvalid') });
      }
    } catch (err: any) {
      setErrors({ code: resolveErrorMessage(err?.message, 'errors.networkError') });
    } finally {
      setLoading(false);
    }
  };

  const handleResendCode = async () => {
    if (countdown > 0 || loading) return;
    
    setLoading(true);
    setErrors({});

    try {
      const response = await authApi.sendResetCode(resetEmail);
      if (response.success) {
        setCountdown(RESEND_COOLDOWN);
        toast.success(t('auth.codeResent'));
      } else {
        setErrors({ form: resolveErrorMessage(response.error, 'common.error') });
      }
    } catch (err: any) {
      setErrors({ form: resolveErrorMessage(err?.message, 'errors.networkError') });
    } finally {
      setLoading(false);
    }
  };

  const handleResetPassword = async (e: React.FormEvent) => {
    e.preventDefault();
    const passwordErrors = validateNewPasswords();
    if (passwordErrors.newPassword || passwordErrors.confirmPassword) {
      setErrors(passwordErrors);
      return;
    }

    setLoading(true);
    setErrors({});

    try {
      const response = await authApi.confirmResetPassword(resetTicket, newPassword);
      if (response.success) {
        toast.success(t('auth.passwordResetSuccess'));
        resetToLogin();
      } else {
        setErrors({ form: resolveErrorMessage(response.error, 'common.error') });
      }
    } catch (err: any) {
      setErrors({ form: resolveErrorMessage(err?.message, 'errors.networkError') });
    } finally {
      setLoading(false);
    }
  };

  const resetToLogin = () => {
    setView('login');
    setResetView('email');
    setErrors({});
    setEmailSent(false);
    setCode('');
    setNewPassword('');
    setConfirmPassword('');
    setResetEmail('');
    setResetTicket('');
    setCountdown(0);
    setWechatQrOpen(false);
    setWechatLoginId('');
    setWechatQrUrl('');
    setWechatBindTicket('');
    setEmail('');
    setPassword('');
  };

  // Handle code input - only allow digits and limit to 6 chars
  const handleCodeChange = useCallback((value: string) => {
    const digitsOnly = value.replace(/\D/g, '').slice(0, CODE_LENGTH);
    setCode(digitsOnly);
    clearFormError();
  }, []);

  return (
    <div className="flex min-h-screen flex-col items-center justify-center bg-background p-4">
      <Card className="w-full max-w-md">
        <CardHeader className="space-y-4 text-center">
          <div>
            <CardTitle className="text-2xl">
              {view === 'login' 
                ? t('auth.login') 
                : view === 'wechat-bind'
                  ? t('auth.bindExisting')
                  : resetView === 'email' 
                    ? t('auth.forgotPassword')
                    : resetView === 'verify'
                      ? t('auth.verifyCode')
                      : t('auth.resetPassword')
              }
            </CardTitle>
            <CardDescription>
              {view === 'login' 
                ? t('auth.welcomeBack')
                : view === 'wechat-bind'
                  ? t('auth.scanSuccess')
                  : resetView === 'email'
                    ? t('auth.enterEmail')
                    : resetView === 'verify'
                      ? t('auth.enterVerificationCode')
                      : t('auth.enterNewPassword')
              }
            </CardDescription>
          </div>
        </CardHeader>
        <CardContent>
          {/* ========== Login View ========== */}
          {view === 'login' && (
            <form onSubmit={handleLogin} className="space-y-4" noValidate>
              {errors.form && (
                <div className="whitespace-pre-line rounded-lg border border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive" role="alert">
                  {errors.form}
                </div>
              )}

              <FormField name="login-email" label={t('auth.email')} error={errors.email}>
                <div className="relative">
                  <Mail className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="login-email"
                    name="email"
                    type="email"
                    inputMode="email"
                    autoComplete="email"
                    placeholder={t('auth.email')}
                    value={email}
                    onChange={(e) => {
                      clearFormError();
                      setEmail(e.target.value);
                    }}
                    onBlur={() => setErrors((prev) => ({ ...prev, email: validateEmail(email) }))}
                    className="pl-10"
                    disabled={loading}
                    aria-invalid={Boolean(errors.email)}
                    aria-describedby={errors.email ? 'login-email-error' : undefined}
                    required
                  />
                </div>
              </FormField>

              <FormField name="login-password" label={t('auth.password')} error={errors.password}>
                <div className="relative">
                  <Lock className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="login-password"
                    name="password"
                    type="password"
                    autoComplete="current-password"
                    placeholder={t('auth.password')}
                    value={password}
                    onChange={(e) => {
                      clearFormError();
                      setPassword(e.target.value);
                    }}
                    onBlur={() => setErrors((prev) => ({ ...prev, password: !password ? t('validation.required') : undefined }))}
                    className="pl-10"
                    disabled={loading}
                    aria-invalid={Boolean(errors.password)}
                    aria-describedby={errors.password ? 'login-password-error' : undefined}
                    required
                  />
                </div>
              </FormField>

              <Button type="submit" className="w-full" disabled={loading}>
                {loading ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t('auth.logining')}
                  </>
                ) : (
                  t('auth.login')
                )}
              </Button>

              {WECHAT_LOGIN_UI_ENABLED && (
                <>
                  {/* Divider */}
                  <div className="relative my-4">
                    <div className="absolute inset-0 flex items-center">
                      <div className="w-full border-t"></div>
                    </div>
                    <div className="relative flex justify-center text-xs uppercase">
                      <span className="bg-background px-2 text-muted-foreground">
                        {t('common.or')}
                      </span>
                    </div>
                  </div>

                  {/* WeChat Login Button */}
                  <Button
                    type="button"
                    variant="outline"
                    className="w-full"
                    onClick={handleStartWechatQR}
                    disabled={loading}
                  >
                    {/* Simple WeChat icon SVG */}
                    <svg className="mr-2 h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M8.691 2.188C3.891 2.188 0 5.476 0 9.53c0 2.212 1.17 4.203 3.002 5.55a.59.59 0 0 1 .213.665l-.39 1.48c-.019.07-.048.141-.048.213 0 .163.13.295.29.295a.326.326 0 0 0 .167-.054l1.903-1.114a.864.864 0 0 1 .717-.098 10.16 10.16 0 0 0 2.837.403c.276 0 .543-.027.811-.05-.857-2.578.157-4.972 1.932-6.446 1.703-1.415 3.882-1.98 5.853-1.838-.576-3.583-4.196-6.348-8.596-6.348zM5.785 5.991c.642 0 1.162.529 1.162 1.18a1.17 1.17 0 0 1-1.162 1.178A1.17 1.17 0 0 1 4.623 7.17c0-.651.52-1.18 1.162-1.18zm5.813 0c.642 0 1.162.529 1.162 1.18a1.17 1.17 0 0 1-1.162 1.178 1.17 1.17 0 0 1-1.162-1.178c0-.651.52-1.18 1.162-1.18zm5.34 2.867c-1.797-.052-3.746.512-5.28 1.786-1.72 1.428-2.687 3.72-1.78 6.22.942 2.453 3.666 4.229 6.884 4.229.826 0 1.622-.12 2.361-.336a.722.722 0 0 1 .598.082l1.584.926a.272.272 0 0 0 .14.045c.134 0 .24-.111.24-.247 0-.06-.023-.12-.038-.177l-.327-1.233a.582.582 0 0 1-.023-.156.49.49 0 0 1 .201-.398C23.024 18.48 24 16.82 24 14.98c0-3.21-2.931-5.837-6.656-6.088V8.89zm-2.944 4.134c.535 0 .969.44.969.982a.976.976 0 0 1-.969.983.976.976 0 0 1-.969-.983c0-.542.434-.982.97-.982zm4.844 0c.535 0 .969.44.969.982a.976.976 0 0 1-.969.983.976.976 0 0 1-.969-.983c0-.542.434-.982.969-.982z"/>
                    </svg>
                    {t('auth.wechatLogin')}
                  </Button>
                </>
              )}
            </form>
          )}

          {/* ========== Forgot Password - Step 1: Enter Email ========== */}
          {view === 'forgot' && resetView === 'email' && (
            <div className="space-y-4">
              {(errors.form || errors.email) && (
                <div className="whitespace-pre-line rounded-lg border border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive" role="alert">
                  {errors.form || errors.email}
                </div>
              )}

              <FormField name="forgot-email" label={t('auth.email')} error={errors.email}>
                <div className="relative">
                  <Mail className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    id="forgot-email"
                    name="email"
                    type="email"
                    inputMode="email"
                    autoComplete="email"
                    placeholder={t('auth.email')}
                    value={resetEmail}
                    onChange={(e) => {
                      clearFormError();
                      setResetEmail(e.target.value);
                    }}
                    onBlur={() => setErrors((prev) => ({ ...prev, email: validateEmail(resetEmail) }))}
                    className="pl-10"
                    disabled={loading}
                    aria-invalid={Boolean(errors.email)}
                    aria-describedby={errors.email ? 'forgot-email-error' : undefined}
                    required
                  />
                </div>
              </FormField>

              <Button onClick={handleSendCode} className="w-full" disabled={loading || emailSent}>
                {loading ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t('common.loading')}
                  </>
                ) : emailSent ? (
                  <>
                    <Check className="mr-2 h-4 w-4" />
                    {t('auth.emailSent')}
                  </>
                ) : (
                  t('auth.sendResetCode')
                )}
              </Button>

              {emailSent && (
                <div className="space-y-4 border-t border-border pt-4">
                  <p className="text-center text-sm text-muted-foreground">{t('auth.resetCodeSent')}</p>
                  
                  <FormField name="verification-code" label={t('auth.verificationCode')} error={errors.code}>
                    <Input
                      id="verification-code"
                      name="code"
                      type="text"
                      inputMode="numeric"
                      autoComplete="one-time-code"
                      placeholder={t('auth.enterVerificationCode')}
                      value={code}
                      onChange={(e) => handleCodeChange(e.target.value)}
                      maxLength={CODE_LENGTH}
                      aria-invalid={Boolean(errors.code)}
                      aria-describedby={errors.code ? 'verification-code-error' : undefined}
                      required
                    />
                  </FormField>

                  <div className="flex gap-2">
                    <Button 
                      onClick={handleVerifyCode} 
                      className="flex-1" 
                      disabled={loading || code.length !== CODE_LENGTH}
                    >
                      {loading ? (
                        <>
                          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                          {t('common.loading')}
                        </>
                      ) : (
                        t('auth.verifyCode')
                      )}
                    </Button>
                    
                    <Button 
                      variant="outline" 
                      onClick={handleResendCode}
                      disabled={loading || countdown > 0}
                      title={countdown > 0 ? t('auth.resetCodeResendCooldown') : t('auth.resendCode')}
                    >
                      {countdown > 0 ? (
                        <>
                          <Timer className="mr-2 h-4 w-4" />
                          {countdown}
                        </>
                      ) : (
                        t('auth.resendCode')
                      )}
                    </Button>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* ========== Forgot Password - Step 3: New Password ========== */}
          {view === 'forgot' && resetView === 'newPassword' && (
            <form onSubmit={handleResetPassword} className="space-y-4" noValidate>
              {errors.form && (
                <div className="whitespace-pre-line rounded-lg border border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive" role="alert">
                  {errors.form}
                </div>
              )}

              <FormField name="new-password" label={t('auth.newPassword')} error={errors.newPassword}>
                <Input
                  id="new-password"
                  name="newPassword"
                  type="password"
                  autoComplete="new-password"
                  placeholder={t('auth.newPasswordPlaceholder')}
                  value={newPassword}
                  onChange={(e) => {
                    clearFormError();
                    setNewPassword(e.target.value);
                  }}
                  aria-invalid={Boolean(errors.newPassword)}
                  aria-describedby={errors.newPassword ? 'new-password-error' : undefined}
                  required
                />
              </FormField>

              <FormField name="confirm-password" label={t('auth.confirmPassword')} error={errors.confirmPassword}>
                <Input
                  id="confirm-password"
                  name="confirmPassword"
                  type="password"
                  autoComplete="new-password"
                  placeholder={t('auth.confirmPasswordPlaceholder')}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  aria-invalid={Boolean(errors.confirmPassword)}
                  aria-describedby={errors.confirmPassword ? 'confirm-password-error' : undefined}
                  required
                />
              </FormField>

              <Button type="submit" className="w-full" disabled={loading}>
                {loading ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t('common.loading')}
                  </>
                ) : (
                  t('auth.resetPassword')
                )}
              </Button>
            </form>
          )}

          {/* ========== WeChat Bind View ========== */}
          {WECHAT_LOGIN_UI_ENABLED && view === 'wechat-bind' && (
            <div className="space-y-4">
              {(errors.form || errors.email || errors.password) && (
                <div className="whitespace-pre-line rounded-lg border border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive" role="alert">
                  {errors.form || errors.email || errors.password}
                </div>
              )}

              <div className="rounded-lg bg-muted p-4 text-center">
                <p className="text-sm text-muted-foreground">
                  {t('auth.scanSuccess')}
                </p>
              </div>

              <div className="space-y-3">
                <Button
                  variant="outline"
                  className="w-full"
                  onClick={() => {
                    setEmail('');
                    setPassword('');
                    setErrors({});
                  }}
                >
                  {t('auth.bindExisting')}
                </Button>
                <Button
                  variant="outline"
                  className="w-full"
                  onClick={() => {
                    setEmail('');
                    setPassword('');
                    setErrors({});
                  }}
                >
                  {t('auth.bindCreate')}
                </Button>
              </div>

              {/* Bind Existing Account Form */}
              <form onSubmit={handleWechatBindExisting} className="space-y-4" noValidate>
                <FormField name="bind-email" label={t('auth.email')} error={errors.email}>
                  <div className="relative">
                    <Mail className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      id="bind-email"
                      name="email"
                      type="email"
                      inputMode="email"
                      autoComplete="email"
                      placeholder={t('auth.email')}
                      value={email}
                      onChange={(e) => {
                        clearFormError();
                        setEmail(e.target.value);
                      }}
                      onBlur={() => setErrors((prev) => ({ ...prev, email: validateEmail(email) }))}
                      className="pl-10"
                      disabled={loading}
                      aria-invalid={Boolean(errors.email)}
                      aria-describedby={errors.email ? 'bind-email-error' : undefined}
                      required
                    />
                  </div>
                </FormField>

                <FormField name="bind-password" label={t('auth.password')} error={errors.password}>
                  <div className="relative">
                    <Lock className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      id="bind-password"
                      name="password"
                      type="password"
                      autoComplete="current-password"
                      placeholder={t('auth.password')}
                      value={password}
                      onChange={(e) => {
                        clearFormError();
                        setPassword(e.target.value);
                      }}
                      onBlur={() => setErrors((prev) => ({ ...prev, password: !password ? t('validation.required') : undefined }))}
                      className="pl-10"
                      disabled={loading}
                      aria-invalid={Boolean(errors.password)}
                      aria-describedby={errors.password ? 'bind-password-error' : undefined}
                      required
                    />
                  </div>
                </FormField>

                <div className="flex gap-2">
                  <Button type="submit" className="flex-1" disabled={loading}>
                    {loading ? (
                      <>
                        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        {t('common.loading')}
                      </>
                    ) : (
                      t('auth.bindExisting')
                    )}
                  </Button>
                </div>
              </form>
            </div>
          )}

          {/* ========== Footer links ========== */}
          <div className="mt-4 text-center text-sm text-muted-foreground">
            {view === 'login' ? (
              <>
                <button type="button" onClick={() => { setView('forgot'); setResetView('email'); }} className="text-primary hover:underline">
                  {t('auth.forgotPassword')}
                </button>
                <span className="mx-2">·</span>
                {t('auth.noAccount')}
                <Link href="/register" className="ml-1 text-primary hover:underline">
                  {t('auth.register')}
                </Link>
              </>
            ) : view === 'wechat-bind' ? (
              <button type="button" onClick={resetToLogin} className="mx-auto flex items-center justify-center gap-1 text-primary hover:underline">
                <ArrowLeft className="h-4 w-4" />
                {t('auth.backToLogin')}
              </button>
            ) : (
              <button type="button" onClick={resetToLogin} className="mx-auto flex items-center justify-center gap-1 text-primary hover:underline">
                <ArrowLeft className="h-4 w-4" />
                {t('auth.backToLogin')}
              </button>
            )}
          </div>
        </CardContent>
      </Card>

      {WECHAT_LOGIN_UI_ENABLED && (
        <Dialog open={wechatQrOpen} onOpenChange={setWechatQrOpen}>
          <DialogContent className="sm:max-w-md">
            <DialogHeader>
              <DialogTitle>{t('auth.wechatLogin')}</DialogTitle>
            </DialogHeader>
            {wechatLoginId && (
              <WechatQR
                loginId={wechatLoginId}
                qrUrl={wechatQrUrl}
                onSuccess={handleWechatSuccess}
                onNeedBind={handleWechatNeedBind}
                onError={handleWechatError}
              />
            )}
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}
