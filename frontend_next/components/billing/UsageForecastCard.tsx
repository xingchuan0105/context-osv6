"use client";

import { useLocale } from "next-intl";
import styles from "./UsageForecastCard.module.css";
import { formatCompactToken } from "../../lib/billing/format";

export type UsageForecastCardProps = {
  suggestion_zh: string;
  suggestion_en: string;
  upgrade_recommended: boolean;
  projected_30d_tokens: number;
  current_limit_7d: number;
};

export function UsageForecastCard({
  suggestion_zh,
  suggestion_en,
  upgrade_recommended,
  projected_30d_tokens,
  current_limit_7d,
}: UsageForecastCardProps) {
  const locale = useLocale();
  const suggestion = locale === "en" ? suggestion_en : suggestion_zh;
  return (
    <div className={`${styles.card} ${upgrade_recommended ? styles.warn : ""}`}>
      <div className={styles.icon}>{upgrade_recommended ? "💡" : "✅"}</div>
      <div className={styles.body}>
        <p className={styles.message}>{suggestion}</p>
        <p className={styles.detail}>
          预计 30 天用量 {formatCompactToken(projected_30d_tokens)} / 7d 限额{" "}
          {formatCompactToken(current_limit_7d)}
        </p>
      </div>
    </div>
  );
}
