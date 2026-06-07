"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { UsageMeter } from "../../../../components/billing/UsageMeter";
import { UsageTrendChart } from "../../../../components/billing/UsageTrendChart";
import { UsageForecastCard } from "../../../../components/billing/UsageForecastCard";
import { billingApi } from "../../../../lib/billing/api";
import type {
  UsageForecastResponse,
  UsageHistoryResponse,
  UsageWindowResponse,
} from "../../../../lib/billing/api";
import { formatUiMessage } from "../../../../lib/i18n/messages";
import { useUiPreferences } from "../../../../lib/ui-preferences";
import styles from "./usage.module.css";

export function UsageDashboardClient() {
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [window, setWindow] = useState<UsageWindowResponse | null>(null);
  const [history, setHistory] = useState<UsageHistoryResponse | null>(null);
  const [forecast, setForecast] = useState<UsageForecastResponse | null>(null);

  useEffect(() => {
    void Promise.all([
      billingApi.getUsageWindow(),
      billingApi.getUsageHistory(7),
      billingApi.getUsageForecast(),
    ]).then(([windowData, historyData, forecastData]) => {
      setWindow(windowData);
      setHistory(historyData);
      setForecast(forecastData);
    });
  }, []);

  if (!window || !history || !forecast) {
    return <div className={styles.page}>{formatUiMessage(locale, "usageLoading")}</div>;
  }

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>{formatUiMessage(locale, "usageTitle")}</h1>
        {forecast.current_plan === "free" && (
          <button
            type="button"
            className={styles.upgradeButton}
            onClick={() => router.push("/upgrade/paywall?from=usage")}
          >
            {formatUiMessage(locale, "upgradeButton")}
          </button>
        )}
      </header>

      <section className={styles.section}>
        <p className={styles.currentPlan}>
          {formatUiMessage(locale, "usageCurrentPlanLabel")}{" "}
          <strong>{forecast.current_plan.toUpperCase()}</strong>
          {forecast.current_plan === "free" && (
            <span className={styles.upgradeHint}>{formatUiMessage(locale, "usageFreeUpgradeHint")}</span>
          )}
        </p>
      </section>

      <section className={styles.meters}>
        <UsageMeter
          variant="full"
          locale={locale}
          planId={window.plan_id}
          rolling5h={window.rolling_5h}
          rolling7d={window.rolling_7d}
          softLimitHit={window.soft_limit_hit}
          hardLimitHit={window.hard_limit_hit}
        />
      </section>

      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>{formatUiMessage(locale, "usageTrendTitle")}</h2>
        <UsageTrendChart daily={history.daily} locale={locale} />
      </section>

      <UsageForecastCard
        locale={locale}
        suggestion_zh={forecast.suggestion_zh}
        suggestion_en={forecast.suggestion_en}
        upgrade_recommended={forecast.upgrade_recommended}
        projected_30d_tokens={forecast.projected_30d_tokens}
        current_limit_7d={forecast.current_limit_7d}
      />
    </div>
  );
}
