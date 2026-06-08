"use client";

import { useEffect, useState } from "react";
import styles from "./UsageMeter.module.css";
import { formatCompactToken, formatCountdown, formatLimitToken } from "../../lib/billing/format";
import type { UsageWindowBucket, LimitHits } from "../../lib/billing/api";
import { formatUiMessage, type UiLocale } from "../../lib/i18n/messages";

export type UsageMeterProps = {
  variant: "full" | "compact";
  locale: UiLocale;
  planId: "free" | "plus" | "pro";
  rolling5h: UsageWindowBucket;
  rolling7d: UsageWindowBucket;
  softLimitHit: LimitHits;
  hardLimitHit: LimitHits;
};

function useCountdown(resetAt: string) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(id);
  }, []);
  const target = new Date(resetAt).getTime();
  return formatCountdown(target - now);
}

function BucketCard({
  title,
  bucket,
  isSoftHit,
  isHardHit,
  compact,
  locale,
}: {
  title: string;
  bucket: UsageWindowBucket;
  isSoftHit: boolean;
  isHardHit: boolean;
  compact: boolean;
  locale: UiLocale;
}) {
  const countdown = useCountdown(bucket.reset_at);
  const unlimitedLabel = formatUiMessage(locale, "usageUnlimited");
  const fillClass = isHardHit
    ? styles.barFill + " " + styles.danger
    : isSoftHit
      ? styles.barFill + " " + styles.warning
      : styles.barFill;
  return (
    <div className={`${styles.card} ${compact ? styles.compact : ""}`}>
      <h3 className={styles.title}>{title}</h3>
      <div className={styles.numbers}>
        <span className={styles.used}>{formatCompactToken(bucket.used)}</span>
        {" / "}
        <span className={styles.limit}>{formatLimitToken(bucket.limit, unlimitedLabel)}</span>
      </div>
      <div
        className={styles.bar}
        role="progressbar"
        aria-valuenow={bucket.percentage}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div className={fillClass} style={{ width: `${bucket.percentage}%` }} />
      </div>
      <div className={styles.resetText}>
        {formatUiMessage(locale, "usageEstimatedReset", { time: countdown })}
      </div>
      {isSoftHit && !compact && (
        <div className={styles.warningText}>{formatUiMessage(locale, "usageSoftLimitWarning")}</div>
      )}
    </div>
  );
}

export function UsageMeter({
  variant,
  locale,
  planId,
  rolling5h,
  rolling7d,
  softLimitHit,
  hardLimitHit,
}: UsageMeterProps) {
  const compact = variant === "compact";
  return (
    <div
      data-testid="usage-meter"
      className={`${styles.meter}${compact ? ` ${styles.meterCompactRow}` : ""}`}
    >
      <BucketCard
        title={compact ? "5h" : formatUiMessage(locale, "usageWindow5h")}
        bucket={rolling5h}
        isSoftHit={softLimitHit.rolling_5h}
        isHardHit={hardLimitHit.rolling_5h}
        compact={compact}
        locale={locale}
      />
      <BucketCard
        title={compact ? "7d" : formatUiMessage(locale, "usageWindow7d")}
        bucket={rolling7d}
        isSoftHit={softLimitHit.rolling_7d}
        isHardHit={hardLimitHit.rolling_7d}
        compact={compact}
        locale={locale}
      />
    </div>
  );
}
