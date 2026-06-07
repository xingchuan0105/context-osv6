"use client";

import { useEffect, useState } from "react";
import styles from "./UsageWarningToast.module.css";
import { formatCompactToken, formatCountdown } from "../../lib/billing/format";

export type UsageWarningToastProps = {
  threshold: 80 | 95;
  windowType: "5h" | "7d";
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
  windowType,
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

  return (
    <div className={`${styles.toast} ${urgency}`} role="alert">
      <div className={styles.body}>
        <strong>
          {windowType} 用量已用 {threshold}%
        </strong>
        <span className={styles.numbers}>
          {" "}
          ({formatCompactToken(used)} / {formatCompactToken(limit)})
        </span>
        <div className={styles.subline}>
          还有 {countdown} 重置。{" "}
          {onUpgradeClick && (
            <button type="button" className={styles.upgradeLink} onClick={onUpgradeClick}>
              升级 Plus 解锁 6× 用量 →
            </button>
          )}
        </div>
      </div>
      <button type="button" className={styles.closeButton} aria-label="关闭" onClick={handleDismiss}>
        ×
      </button>
    </div>
  );
}
