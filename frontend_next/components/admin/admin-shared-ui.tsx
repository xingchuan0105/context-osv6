"use client";

import styles from "./admin-shared-ui.module.css";

const METRIC_DOT_TONE_CLASS = {
  primary: styles.metricDotPrimary,
  success: styles.metricDotSuccess,
  warning: styles.metricDotWarning,
  danger: styles.metricDotDanger,
} as const;

const METRIC_VALUE_TONE_CLASS = {
  primary: styles.metricValuePrimary,
  success: styles.metricValueSuccess,
  warning: styles.metricValueWarning,
  danger: styles.metricValueDanger,
} as const;

export function AdminPageHeading({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <header className={styles.headingBlock}>
      <h1 className={styles.headingTitle}>{title}</h1>
      <p className={styles.headingSubtitle}>{subtitle}</p>
    </header>
  );
}

export function AdminMetricCard({
  label,
  value,
  tone = "primary",
  detail,
}: {
  label: string;
  value: string;
  tone?: "primary" | "success" | "warning" | "danger";
  detail?: string;
}) {
  return (
    <section className={`app-inline-surface ${styles.metricCard}`}>
      <div className={styles.metricLabel}>
        <span className={`${styles.metricDot} ${METRIC_DOT_TONE_CLASS[tone]}`} />
        <span>{label}</span>
      </div>
      <strong className={`${styles.metricValue} ${METRIC_VALUE_TONE_CLASS[tone]}`}>{value}</strong>
      {detail ? <span className={styles.metricDetail}>{detail}</span> : null}
    </section>
  );
}

export function LoadingState({ copy }: { copy: string }) {
  return (
    <section className={`app-inline-surface ${styles.loadingState}`}>
      {copy}
    </section>
  );
}

export function EmptyState({ copy }: { copy: string }) {
  return (
    <section className={`app-inline-surface ${styles.emptyState}`}>
      {copy}
    </section>
  );
}

export function ErrorState({ message }: { message: string }) {
  return <p className="app-notice-banner">{message}</p>;
}
