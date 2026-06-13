'use client';

import { useState } from 'react';
import Link from 'next/link';

interface ConsentCheckboxProps {
  onConsentChange: (consented: boolean) => void;
  required?: boolean;
  termsVersion?: string;
  privacyVersion?: string;
}

export default function ConsentCheckbox({
  onConsentChange,
  required = true,
  termsVersion = '2026-06-13',
  privacyVersion = '2026-06-13',
}: ConsentCheckboxProps) {
  const [consented, setConsented] = useState(false);
  const [error, setError] = useState('');

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const isChecked = e.target.checked;
    setConsented(isChecked);
    setError('');
    onConsentChange(isChecked);
  };

  const handleSubmit = () => {
    if (required && !consented) {
      setError('请先阅读并同意用户协议与隐私政策');
      return false;
    }
    return true;
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
          我已阅读并同意
          <Link href="/legal/terms" target="_blank" className="consent-link">
            《用户服务协议》
          </Link>
          与
          <Link href="/legal/privacy" target="_blank" className="consent-link">
            《隐私政策》
          </Link>
        </span>
      </label>
      {error && <p className="consent-error">{error}</p>}
      <input type="hidden" name="terms_version" value={termsVersion} />
      <input type="hidden" name="privacy_version" value={privacyVersion} />
      <input type="hidden" name="accepted_at" value={new Date().toISOString()} />
    </div>
  );
}
