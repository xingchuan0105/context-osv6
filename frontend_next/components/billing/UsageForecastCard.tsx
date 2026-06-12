"use client";

import styles from "./UsageForecastCard.module.css";
import { formatCompactToken, formatLimitToken } from "../../lib/billing/format";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";

export type UsageForecastCardProps = {
  locale: UiLocale;
  suggestion_zh: string;
  suggestion_en: string;
  upgrade_recommended: boolean;
  projected_30d_tokens: number;
  current_limit_7d: number;
};

export function UsageForecastCard({
  locale,
  suggestion_zh,
  suggestion_en,
  upgrade_recommended,
  projected_30d_tokens,
  current_limit_7d,
}: UsageForecastCardProps) {
  const suggestion = locale === "en" ? suggestion_en : suggestion_zh;
  const unlimitedLabel = formatUiMessage(locale, "usageUnlimited");
  return (
    <div className={`${styles.card} ${upgrade_recommended ? styles.warn : ""}`}>
      <div className={styles.icon}>{upgrade_recommended ? "💡" : "✅"}</div>
      <div className={styles.body}>
        <p className={styles.message}>{suggestion}</p>
        <p className={styles.detail}>
          {formatUiMessage(locale, "usageForecastDetail", {
            projected: formatCompactToken(projected_30d_tokens),
            limit: formatLimitToken(current_limit_7d, unlimitedLabel),
          })}
        </p>
      </div>
    </div>
  );
}
