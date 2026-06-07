import Link from "next/link";

import styles from "./success.module.css";

export const dynamic = "force-dynamic";

export default function UpgradeSuccessPage() {
  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <div className={styles.icon}>🎉</div>
        <h1 className={styles.title}>升级成功</h1>
        <p className={styles.subtitle}>新档位已立即生效，祝你用得开心。</p>
        <div className={styles.actions}>
          <Link href="/dashboard" className={styles.primaryButton}>
            返回工作区
          </Link>
          <Link href="/settings/usage" className={styles.secondaryButton}>
            查看用量
          </Link>
        </div>
      </div>
    </div>
  );
}
