"use client";

import Link from "next/link";

import { formatUiMessage } from "../../../../lib/i18n/messages";
import { useUiPreferences } from "../../../../lib/ui-preferences";
import styles from "./success.module.css";

export default function UpgradeSuccessPage() {
  const { locale } = useUiPreferences();

  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <div className={styles.icon}>🎉</div>
        <h1 className={styles.title}>{formatUiMessage(locale, "upgradeSuccessTitle")}</h1>
        <p className={styles.subtitle}>{formatUiMessage(locale, "upgradeSuccessSubtitle")}</p>
        <div className={styles.actions}>
          <Link href="/dashboard" className={styles.primaryButton}>
            {formatUiMessage(locale, "upgradeSuccessBack")}
          </Link>
          <Link href="/settings/usage" className={styles.secondaryButton}>
            {formatUiMessage(locale, "upgradeSuccessViewUsage")}
          </Link>
        </div>
      </div>
    </div>
  );
}
