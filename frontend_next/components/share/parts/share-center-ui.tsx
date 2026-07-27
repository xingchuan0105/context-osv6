"use client";

import styles from "./share-center-ui.module.css";

export function SectionHeader({
  subtitle,
  title,
}: {
  subtitle: string;
  title: string;
}) {
  return (
    <div className={styles.sectionHeader}>
      <h2 className={`app-page-title ${styles.sectionTitle}`}>
        {title}
      </h2>
      <p className={`app-page-subtitle ${styles.sectionSubtitle}`}>
        {subtitle}
      </p>
    </div>
  );
}

export function InsightMetricCard({
  title,
  value,
}: {
  title: string;
  value: string;
}) {
  return (
    <section
      className={`app-inline-surface ${styles.metricCard}`}
    >
      <h3 className={styles.metricTitle}>
        {title}
      </h3>
      <p className={styles.metricValue}>
        {value}
      </p>
    </section>
  );
}

export function shareStatusBadgeClass(status: import("./share-center-utils").ShareStatus | null) {
  if (status === "active") {
    return styles.statusBadgeActive;
  }

  if (status === "expired") {
    return styles.statusBadgeExpired;
  }

  return styles.statusBadgeIdle;
}
