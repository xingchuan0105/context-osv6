"use client";

import { useEffect, useState } from "react";
import styles from "./UsageMeter.module.css";
import { formatCompactToken, formatCountdown } from "../../lib/billing/format";
import type { UsageWindowBucket, LimitHits } from "../../lib/billing/api";

export type UsageMeterProps = {
  variant: "full" | "compact";
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
}: {
  title: string;
  bucket: UsageWindowBucket;
  isSoftHit: boolean;
  isHardHit: boolean;
  compact: boolean;
}) {
  const countdown = useCountdown(bucket.reset_at);
  const fillClass = isHardHit ? styles.barFill + " " + styles.danger
                  : isSoftHit ? styles.barFill + " " + styles.warning
                  : styles.barFill;
  return (
    <div className={`${styles.card} ${compact ? styles.compact : ""}`}>
      <h3 className={styles.title}>{title}</h3>
      <div className={styles.numbers}>
        <span className={styles.used}>{formatCompactToken(bucket.used)}</span>
        {" / "}
        <span className={styles.limit}>{formatCompactToken(bucket.limit)}</span>
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
      <div className={styles.resetText}>预计 {countdown} 后重置</div>
      {isSoftHit && !compact && (
        <div className={styles.warningText}>⚠️ 已超过软上限，建议控制节奏</div>
      )}
    </div>
  );
}

export function UsageMeter({ variant, planId, rolling5h, rolling7d, softLimitHit, hardLimitHit }: UsageMeterProps) {
  const compact = variant === "compact";
  return (
    <>
      <BucketCard
        title={compact ? "5h" : "5 小时窗口"}
        bucket={rolling5h}
        isSoftHit={softLimitHit.rolling_5h}
        isHardHit={hardLimitHit.rolling_5h}
        compact={compact}
      />
      <BucketCard
        title={compact ? "7d" : "7 天窗口"}
        bucket={rolling7d}
        isSoftHit={softLimitHit.rolling_7d}
        isHardHit={hardLimitHit.rolling_7d}
        compact={compact}
      />
    </>
  );
}
