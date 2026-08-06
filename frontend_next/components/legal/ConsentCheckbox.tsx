"use client";

import Link from "next/link";
import { useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";

interface ConsentCheckboxProps {
  onConsentChange: (consented: boolean) => void;
  required?: boolean;
}

export default function ConsentCheckbox({
  onConsentChange,
  required = true,
}: ConsentCheckboxProps) {
  const { locale } = useUiPreferences();
  const [consented, setConsented] = useState(false);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const isChecked = e.target.checked;
    setConsented(isChecked);
    onConsentChange(isChecked);
  };

  return (
    <div className="consent-checkbox">
      <label className="consent-label">
        <input
          type="checkbox"
          checked={consented}
          onChange={handleChange}
          required={required}
          className="consent-input"
        />
        <span className="consent-text">
          {formatUiMessage(locale, "legalConsentPrefix")}
          <Link href="/legal/terms" target="_blank" className="consent-link">
            {formatUiMessage(locale, "legalConsentTerms")}
          </Link>
          {formatUiMessage(locale, "legalConsentAnd")}
          <Link href="/legal/privacy" target="_blank" className="consent-link">
            {formatUiMessage(locale, "legalConsentPrivacy")}
          </Link>
        </span>
      </label>
    </div>
  );
}
