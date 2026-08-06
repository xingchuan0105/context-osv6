"use client";

import { useEffect, useState, type ReactNode } from "react";

import styles from "./desktop.module.css";
import {
  IconStatusCheck,
  IconStatusCircle,
  IconStatusClock,
  IconStatusError,
  IconStatusWarning,
} from "../status-icons";
import {
  getLicenseStatus,
  type LicenseStatus,
  type LicenseStatusKind,
} from "@/lib/desktop/tauri-license";
import { formatUiMessage } from "@/lib/i18n/messages";
import type { UiLocale } from "@/lib/i18n/config";
import { useUiPreferences } from "@/lib/ui-preferences";

function statusLabel(kind: LicenseStatusKind, status: LicenseStatus, locale: UiLocale): string {
  switch (kind) {
    case "active":
      return formatUiMessage(locale, "desktop.status.active");
    case "trial":
      return formatUiMessage(locale, "desktop.status.trial", {
        days: String(status.days_remaining ?? 0),
      });
    case "expired":
      return formatUiMessage(locale, "desktop.status.expired");
    case "revoked":
      return formatUiMessage(locale, "desktop.status.revoked");
    case "unactivated":
      return formatUiMessage(locale, "desktop.status.unactivated");
    case "offline_grace":
      return formatUiMessage(locale, "desktop.status.offlineGrace", {
        days: String(status.offline_grace_days ?? 0),
      });
    default:
      return formatUiMessage(locale, "desktop.status.unactivated");
  }
}

const STATUS_VISUAL: Record<LicenseStatusKind, { icon: ReactNode; className: string }> = {
  active: { icon: <IconStatusCheck />, className: styles.statusActive },
  trial: { icon: <IconStatusClock />, className: styles.statusTrial },
  expired: { icon: <IconStatusWarning />, className: styles.statusError },
  revoked: { icon: <IconStatusError />, className: styles.statusError },
  unactivated: { icon: <IconStatusCircle />, className: styles.statusMuted },
  offline_grace: { icon: <IconStatusWarning />, className: styles.statusTrial },
};

type DesktopStatusBadgeProps = {
  onClick?: () => void;
};

export function DesktopStatusBadge({ onClick }: DesktopStatusBadgeProps) {
  const { locale } = useUiPreferences();
  const [status, setStatus] = useState<LicenseStatus | null>(null);

  useEffect(() => {
    const check = () => {
      void getLicenseStatus()
        .then(setStatus)
        .catch(() => setStatus({ kind: "unactivated", dev_mode: false }));
    };

    check();
    const interval = window.setInterval(check, 60_000);
    return () => window.clearInterval(interval);
  }, []);

  if (!status) {
    return null;
  }

  const visual = STATUS_VISUAL[status.kind];

  return (
    <button
      type="button"
      className={styles.statusBadge}
      aria-label={formatUiMessage(locale, "desktop.status.ariaLabel")}
      data-testid="desktop-status-badge"
      onClick={onClick}
    >
      <span aria-hidden="true" className={visual.className}>
        {visual.icon}
      </span>
      <span className={visual.className}>{statusLabel(status.kind, status, locale)}</span>
    </button>
  );
}
