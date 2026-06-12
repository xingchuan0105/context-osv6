"use client";

import { useEffect, useState } from "react";
import styles from "./UsageWarningToast.module.css";
import { formatCompactToken, formatCountdown, formatLimitToken } from "../../lib/billing/format";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";

export type UsageWarningToastProps = {
  threshold: 80 | 95;
  percentage: number;
  windowType: "5h" | "7d";
  locale: UiLocale;
  userId: string;
  used: number;
  limit: number;
  resetAt: string;
  onDismiss: () => void;
  onUpgradeClick?: () => void;
};

const DISMISS_KEY = (userId: string, windowType: string, threshold: number) =>
  `toast_dismissed_${userId}_${windowType}_${threshold}`;

function useCountdown(resetAt: string) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(id);
  }, []);
  return formatCountdown(new Date(resetAt).getTime() - now);
}

export function UsageWarningToast({
  threshold,
  percentage,
  windowType,
  locale,
  userId,
  used,
  limit,
  resetAt,
  onDismiss,
  onUpgradeClick,
}: UsageWarningToastProps) {
  const [hidden, setHidden] = useState(true);

  useEffect(() => {
    const key = DISMISS_KEY(userId, windowType, threshold);
    if (localStorage.getItem(key) === "true") {
      setHidden(true);
      return;
    }
    setHidden(false);
  }, [userId, windowType, threshold]);

  const countdown = useCountdown(resetAt);
  if (hidden) return null;

  const handleDismiss = () => {
    localStorage.setItem(DISMISS_KEY(userId, windowType, threshold), "true");
    setHidden(true);
    onDismiss();
  };

  const urgency = threshold === 95 ? styles.urgent : styles.elevated;
  const unlimitedLabel = formatUiMessage(locale, "usageUnlimited");

  return (
    <div className={`${styles.toast} ${urgency}`} role="alert">
      <div className={styles.body}>
        <strong>{formatUiMessage(locale, "toastUsageAt", { window: windowType, pct: percentage })}</strong>
        <span className={styles.numbers}>
          {" "}
          ({formatCompactToken(used)} / {formatLimitToken(limit, unlimitedLabel)})
        </span>
        <div className={styles.subline}>
          {formatUiMessage(locale, "toastResetsIn", { time: countdown })}{" "}
          {onUpgradeClick && (
            <button type="button" className={styles.upgradeLink} onClick={onUpgradeClick}>
              {formatUiMessage(locale, "toastUpgradeCta")}
            </button>
          )}
        </div>
      </div>
      <button
        type="button"
        className={styles.closeButton}
        aria-label={formatUiMessage(locale, "toastClose")}
        onClick={handleDismiss}
      >
        ×
      </button>
    </div>
  );
}
