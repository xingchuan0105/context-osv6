"use client";

import Link from "next/link";

import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";
import { useUiPreferences } from "../../lib/ui-preferences";

export default function LegalFooterLinks({ locale: localeProp }: { locale?: UiLocale }) {
  const { locale: uiLocale } = useUiPreferences();
  const locale = localeProp ?? uiLocale;
  const currentYear = new Date().getFullYear();

  return (
    <footer className="legal-footer-links">
      <div className="legal-footer-content">
        <Link href="/legal/terms">{formatUiMessage(locale, "legalFooterTerms")}</Link>
        <span className="legal-footer-separator">·</span>
        <Link href="/legal/privacy">{formatUiMessage(locale, "legalFooterPrivacy")}</Link>
        <span className="legal-footer-separator">·</span>
        <Link href="/legal/licenses">{formatUiMessage(locale, "legalFooterLicenses")}</Link>
      </div>
      <div className="legal-footer-copyright">© {currentYear} Context-OS</div>
    </footer>
  );
}
