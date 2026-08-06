import { ApiError } from "./client";
import { DEFAULT_LOCALE, type UiLocale } from "../i18n/config";
import { formatUiMessage } from "../i18n/messages";

function fallbackAuthError(locale: UiLocale) {
  return formatUiMessage(locale, "authErrorServiceUnavailable");
}

export function describeAuthError(fallback: string, error: unknown, locale: UiLocale = DEFAULT_LOCALE) {
  if (!(error instanceof ApiError)) {
    if (error instanceof Error) {
      return error.message.trim() || fallback;
    }
    return fallback;
  }

  switch (error.code) {
    case "account_not_registered":
    case "email_not_registered":
      return formatUiMessage(locale, "authErrorAccountNotRegistered");
    case "invalid_password":
      return formatUiMessage(locale, "authErrorInvalidPassword");
    case "invalid_credentials":
      return formatUiMessage(locale, "authErrorInvalidCredentials");
    case "email_exists":
      return formatUiMessage(locale, "authErrorEmailExists");
    case "password_reset_unavailable":
      return formatUiMessage(locale, "authErrorPasswordResetUnavailable");
    case "invalid_reset_ticket":
      return formatUiMessage(locale, "authErrorInvalidResetTicket");
    case "service_unavailable":
      return fallbackAuthError(locale);
    case "validation_error":
      return error.message.trim() || fallback;
    case "invalid_terms_version":
      return formatUiMessage(locale, "authErrorInvalidTermsVersion");
    case "invalid_privacy_version":
      return formatUiMessage(locale, "authErrorInvalidPrivacyVersion");
    case "invalid_context":
      return formatUiMessage(locale, "authErrorInvalidLegalContext");
    case "consent_required":
      return formatUiMessage(locale, "authErrorConsentRequired");
    case "payer_funds_required":
      return formatUiMessage(locale, "settings.billing.payerFundsRequired");
    case "share_owner_daily_budget_exceeded":
      return error.message.trim() || fallback;
    default:
      return error.message.trim() || fallback;
  }
}

/** Product billing / paywall codes with top-up guidance when appropriate. */
export function describeProductError(
  fallback: string,
  error: unknown,
  locale: UiLocale = DEFAULT_LOCALE,
): string {
  if (error instanceof ApiError && error.code === "payer_funds_required") {
    return `${formatUiMessage(locale, "settings.billing.payerFundsRequired")} ${formatUiMessage(
      locale,
      "settings.billing.payerFundsTopUpHint",
    )}`;
  }
  return describeAuthError(fallback, error, locale);
}
