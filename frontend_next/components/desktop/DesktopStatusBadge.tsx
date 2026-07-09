"use client";

import { useEffect, useState } from "react";

import styles from "./desktop.module.css";
import {
  getLicenseStatus,
  type LicenseStatus,
  type LicenseStatusKind,
} from "@/lib/desktop/tauri-license";

const STATUS_CONFIG: Record<
  LicenseStatusKind,
  { icon: string; className: string; label: (status: LicenseStatus) => string }
> = {
  active: {
    icon: "✓",
    className: styles.statusActive,
    label: () => "已激活",
  },
  trial: {
    icon: "⏱",
    className: styles.statusTrial,
    label: (status) => `试用 ${status.days_remaining ?? 0}d`,
  },
  expired: {
    icon: "⚠",
    className: styles.statusError,
    label: () => "已过期",
  },
  revoked: {
    icon: "✗",
    className: styles.statusError,
    label: () => "已吊销",
  },
  unactivated: {
    icon: "○",
    className: styles.statusMuted,
    label: () => "未激活",
  },
  offline_grace: {
    icon: "⚠",
    className: styles.statusTrial,
    label: (status) => `离线宽限 ${status.offline_grace_days ?? 0}d`,
  },
};

type DesktopStatusBadgeProps = {
  onClick?: () => void;
};

export function DesktopStatusBadge({ onClick }: DesktopStatusBadgeProps) {
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

  const config = STATUS_CONFIG[status.kind];

  return (
    <button
      type="button"
      className={styles.statusBadge}
      aria-label="授权状态"
      onClick={onClick}
    >
      <span aria-hidden="true">{config.icon}</span>
      <span className={config.className}>{config.label(status)}</span>
    </button>
  );
}
