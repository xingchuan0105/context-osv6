/**
 * Error Code to i18n Key Mapping (M6)
 * 
 * Maps backend error codes to translation keys for consistent error messaging.
 * This mapping is used in authentication flows (login, password reset, etc.).
 */

/** Error code to i18n key mapping */
export const ERROR_CODE_MAP: Record<string, string> = {
  // Password reset errors
  'ERR_RESET_CODE_INVALID': 'auth.resetCodeInvalid',
  'ERR_RESET_CODE_EXPIRED': 'auth.resetCodeExpired',
  'ERR_RESET_CODE_TOO_MANY_ATTEMPTS': 'auth.resetCodeTooManyAttempts',
  'ERR_RESET_CODE_COOLDOWN': 'auth.resetCodeResendCooldown',
  'ERR_RESET_TICKET_INVALID': 'auth.resetCodeInvalid',
  'ERR_RESET_TICKET_EXPIRED': 'auth.resetCodeExpired',
  
  // Authentication errors
  'ERR_INVALID_CREDENTIALS': 'auth.loginInvalidCredentials',
  'ERR_USER_NOT_FOUND': 'auth.loginUserNotFound',
  'ERR_INVALID_PASSWORD': 'auth.loginPasswordIncorrect',
  'ERR_USER_DISABLED': 'auth.loginAccountDisabled',
  'ERR_TOKEN_EXPIRED': 'errors.unauthorized',
  'ERR_TOKEN_INVALID': 'errors.unauthorized',

  // WeChat login errors
  'ERR_WECHAT_NOT_CONFIGURED': 'auth.wechatNotConfigured',
  
  // Rate limiting
  'ERR_RATE_LIMIT_EXCEEDED': 'errors.rateLimitExceeded',
  'ERR_TOO_MANY_REQUESTS': 'errors.rateLimitExceeded',
};

const ERROR_PATTERNS: Array<{ pattern: RegExp; i18nKey: string }> = [
  { pattern: /微信登录未配置/i, i18nKey: 'auth.wechatNotConfigured' },
  { pattern: /wechat.*not.*configured/i, i18nKey: 'auth.wechatNotConfigured' },
  { pattern: /invalid credentials/i, i18nKey: 'auth.loginInvalidCredentials' },
  { pattern: /user not found/i, i18nKey: 'auth.loginUserNotFound' },
  { pattern: /incorrect password/i, i18nKey: 'auth.loginPasswordIncorrect' },
];

/**
 * Get i18n key from error code or error message
 * @param error - Error code or error message from backend
 * @returns i18n key or original error if no match
 */
export function getErrorI18nKey(error: string): string {
  for (const [code, i18nKey] of Object.entries(ERROR_CODE_MAP)) {
    if (error.includes(code) || error.toLowerCase().includes(code.toLowerCase().replace('ERR_', ''))) {
      return i18nKey;
    }
  }
  for (const { pattern, i18nKey } of ERROR_PATTERNS) {
    if (pattern.test(error)) {
      return i18nKey;
    }
  }
  return error;
}

/**
 * Get error message using translation function
 * @param error - Error code or error message from backend
 * @param t - Translation function
 * @returns Translated error message
 */
export function getTranslatedError(error: string, t: (key: string) => string): string {
  const i18nKey = getErrorI18nKey(error);
  
  // Check if i18nKey exists in translations
  try {
    const translated = t(i18nKey);
    // If translation returns the same key, translation doesn't exist
    if (translated === i18nKey) {
      return error;
    }
    return translated;
  } catch {
    return error;
  }
}

export function isWechatNotConfiguredError(error: string): boolean {
  return (
    error.includes('ERR_WECHAT_NOT_CONFIGURED') ||
    /微信登录未配置/i.test(error) ||
    /wechat.*not.*configured/i.test(error)
  );
}

export default ERROR_CODE_MAP;
