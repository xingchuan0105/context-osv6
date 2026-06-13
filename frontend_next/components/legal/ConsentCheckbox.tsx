'use client';

import { useState } from 'react';
import Link from 'next/link';

interface ConsentCheckboxProps {
  onConsentChange: (consented: boolean) => void;
  required?: boolean;
}

export default function ConsentCheckbox({
  onConsentChange,
  required = true,
}: ConsentCheckboxProps) {
  const [consented, setConsented] = useState(false);
  const [error, setError] = useState('');

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const isChecked = e.target.checked;
    setConsented(isChecked);
    setError('');
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
      {/* 注：版本号 / 同意时间由父组件在 submit 时附带，不通过 hidden 字段传递。
          原 hidden 输入每次 re-render 会刷新 accepted_at，且不参与 form submit，移除避免漂移。 */}
    </div>
  );
}
