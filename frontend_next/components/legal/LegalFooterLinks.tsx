"use client";

import Link from "next/link";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";

export default function LegalFooterLinks() {
  const { locale } = useUiPreferences();
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
