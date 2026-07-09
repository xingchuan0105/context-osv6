"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { ProtectedRouteGate } from "@/components/auth-gates";
import styles from "@/components/desktop/desktop.module.css";
import {
  fetchMyLicenses,
  licenseKindDisplay,
  licenseStatusLabel,
  type LicenseSummary,
} from "@/lib/desktop/license-client";
import { useAuth } from "@/lib/auth/context";

export default function LicensesPage() {
  const auth = useAuth();
  const [licenses, setLicenses] = useState<LicenseSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!auth.token) return;

    let cancelled = false;
    setLoading(true);
    setError("");

    void fetchMyLicenses(auth.token)
      .then((response) => {
        if (!cancelled) {
          setLicenses(response.licenses);
        }
      })
      .catch((fetchError) => {
        if (!cancelled) {
          setError(fetchError instanceof Error ? fetchError.message : "加载授权失败");
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [auth.token]);

  return (
    <ProtectedRouteGate>
      <main className="app-page-shell">
        <div className="app-page-center" style={{ maxWidth: "42rem" }}>
          <header className="app-page-heading">
            <h1 className="app-page-title">我的授权</h1>
            <p className="app-page-subtitle">管理 AVRag Desktop 授权与已激活设备</p>
          </header>

          {loading ? <p className={styles.subtitle}>加载中…</p> : null}
          {error ? (
            <p className={styles.errorBox} role="alert">
              {error}
            </p>
          ) : null}

          {!loading && !error && licenses.length === 0 ? (
            <p className={styles.subtitle}>暂无授权记录。</p>
          ) : null}

          <div className={styles.licenseList}>
            {licenses.map((license) => (
              <Link key={license.id} href={`/licenses/${license.id}`} className={styles.licenseCard}>
                <div className={styles.metaRow}>
                  <strong>{licenseKindDisplay(license.kind)}</strong>
                  <span>{licenseStatusLabel(license.status)}</span>
                </div>
                <div className={styles.metaRow}>
                  <span className={styles.metaLabel}>{license.key}</span>
                  <span>
                    {license.machines_count ?? 0}/{license.max_machines ?? 1} 设备
                  </span>
                </div>
              </Link>
            ))}
          </div>
        </div>
      </main>
    </ProtectedRouteGate>
  );
}
