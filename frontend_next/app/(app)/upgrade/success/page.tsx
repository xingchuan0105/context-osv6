import Link from "next/link";
import { getLocale } from "next-intl/server";

import { normalizeLocale } from "../../../lib/i18n/config";
import { formatUiMessage } from "../../../lib/i18n/messages";
import styles from "./success.module.css";

export const dynamic = "force-dynamic";

export default async function UpgradeSuccessPage() {
  const locale = normalizeLocale(await getLocale());

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
