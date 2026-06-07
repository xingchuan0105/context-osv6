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
import { ApiError } from "../../../../lib/auth/client";
import {
  isPricingRevampEnabled,
  isPricingRevampEnabledSSR,
  isPricingRevampFeatureDisabledError,
} from "../../../../lib/billing/featureFlag";
import { formatUiMessage } from "../../../../lib/i18n/messages";
import { useUiPreferences } from "../../../../lib/ui-preferences";
import styles from "./usage.module.css";

type DashboardState =
  | { kind: "loading" }
  | { kind: "ready"; window: UsageWindowResponse; history: UsageHistoryResponse; forecast: UsageForecastResponse }
  | { kind: "error"; messageKey: "usageErrorLoad" };

export function UsageDashboardClient() {
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [state, setState] = useState<DashboardState>({ kind: "loading" });

  useEffect(() => {
    if (!isPricingRevampEnabledSSR()) {
      router.replace("/settings");
      return;
    }

    let cancelled = false;

    async function loadDashboard() {
      const enabled = await isPricingRevampEnabled();
      if (cancelled) {
        return;
      }
      if (!enabled) {
        router.replace("/settings");
        return;
      }

      try {
        const [windowData, historyData, forecastData] = await Promise.all([
          billingApi.getUsageWindow(),
          billingApi.getUsageHistory(7),
          billingApi.getUsageForecast(),
        ]);
        if (cancelled) {
          return;
        }
        setState({
          kind: "ready",
          window: windowData,
          history: historyData,
          forecast: forecastData,
        });
      } catch (error) {
        if (cancelled) {
          return;
        }
        if (
          (error instanceof ApiError && error.code === "feature_disabled") ||
          isPricingRevampFeatureDisabledError(error)
        ) {
          router.replace("/settings");
          return;
        }
        setState({
          kind: "error",
          messageKey: "usageErrorLoad",
        });
      }
    }

    void loadDashboard();

    return () => {
      cancelled = true;
    };
  }, [router]);

  if (state.kind === "loading") {
    return <div className={styles.page}>{formatUiMessage(locale, "usageLoading")}</div>;
  }

  if (state.kind === "error") {
    return (
      <div className={styles.page}>
        <p className={styles.errorBanner}>{formatUiMessage(locale, state.messageKey)}</p>
        <button type="button" className={styles.upgradeButton} onClick={() => router.push("/dashboard")}>
          {formatUiMessage(locale, "usageErrorBackDashboard")}
        </button>
      </div>
    );
  }

  const { window, history, forecast } = state;

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
