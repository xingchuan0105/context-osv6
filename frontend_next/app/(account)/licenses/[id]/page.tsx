"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useEffect, useState } from "react";

import { ProtectedRouteGate } from "@/components/auth-gates";
import styles from "@/components/desktop/desktop.module.css";
import {
  deactivateLicenseMachine,
  fetchLicenseMachines,
  fetchMyLicenses,
  formatHeartbeatLabel,
  licenseKindDisplay,
  type LicenseMachine,
  type LicenseSummary,
} from "@/lib/desktop/license-client";
import { useAuth } from "@/lib/auth/context";

export default function LicenseDetailPage() {
  const auth = useAuth();
  const params = useParams<{ id: string }>();
  const licenseId = params.id;
  const [license, setLicense] = useState<LicenseSummary | null>(null);
  const [machines, setMachines] = useState<LicenseMachine[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [unbindingId, setUnbindingId] = useState<string | null>(null);

  useEffect(() => {
    if (!auth.token || !licenseId) return;

    let cancelled = false;
    setLoading(true);
    setError("");

    void Promise.all([
      fetchMyLicenses(auth.token),
      fetchLicenseMachines(auth.token, licenseId),
    ])
      .then(([licenseList, machineList]) => {
        if (cancelled) return;

        const current = licenseList.licenses.find((item) => item.id === licenseId) ?? null;
        setLicense(current);
        setMachines(machineList.machines);

        if (!current) {
          setError("未找到授权");
        }
      })
      .catch((fetchError) => {
        if (!cancelled) {
          setError(fetchError instanceof Error ? fetchError.message : "加载授权详情失败");
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
  }, [auth.token, licenseId]);

  async function handleUnbindMachine(machineId: string) {
    if (!auth.token) return;

    setUnbindingId(machineId);
    setError("");

    try {
      await deactivateLicenseMachine(auth.token, licenseId, machineId);
      setMachines((current) => current.filter((machine) => machine.id !== machineId));
    } catch (unbindError) {
      setError(unbindError instanceof Error ? unbindError.message : "解绑设备失败");
    } finally {
      setUnbindingId(null);
    }
  }

  if (loading) {
    return (
      <ProtectedRouteGate>
        <main className="app-page-shell">
          <div className="app-page-center" style={{ maxWidth: "42rem" }}>
            <p className={styles.subtitle}>加载中…</p>
          </div>
        </main>
      </ProtectedRouteGate>
    );
  }

  if (!license) {
    return (
      <ProtectedRouteGate>
        <main className="app-page-shell">
          <div className="app-page-center" style={{ maxWidth: "42rem" }}>
            <p>{error || `未找到授权 ${licenseId}`}</p>
            <Link href="/licenses" className="app-link">
              返回授权列表
            </Link>
          </div>
        </main>
      </ProtectedRouteGate>
    );
  }

  const seatsMax = license.max_machines ?? 1;
  const emptySlots = Math.max(seatsMax - machines.length, 0);

  return (
    <ProtectedRouteGate>
      <main className="app-page-shell">
        <div className="app-page-center" style={{ maxWidth: "42rem" }}>
          <header className="app-page-heading">
            <h1 className="app-page-title">{licenseKindDisplay(license.kind)}</h1>
            <p className="app-page-subtitle">{license.key}</p>
          </header>

          {error ? (
            <p className={styles.errorBox} role="alert">
              {error}
            </p>
          ) : null}

          <section className={styles.card} style={{ marginBottom: "1rem" }}>
            <h2 style={{ margin: "0 0 0.75rem", fontSize: "1rem" }}>
              已激活设备 ({machines.length}/{seatsMax})
            </h2>
            <div className={styles.machineList}>
              {machines.map((machine) => (
                <div key={machine.id} className={styles.machineRow}>
                  <div>
                    <div>● {machine.name ?? machine.fingerprint ?? machine.id}</div>
                    <div className={styles.metaLabel}>
                      最后心跳: {formatHeartbeatLabel(machine.last_heartbeat_at)}
                    </div>
                  </div>
                  <button
                    type="button"
                    className="app-button-ghost"
                    disabled={unbindingId === machine.id}
                    onClick={() => void handleUnbindMachine(machine.id)}
                  >
                    {unbindingId === machine.id ? "解绑中…" : "解绑"}
                  </button>
                </div>
              ))}
              {Array.from({ length: emptySlots }).map((_, index) => (
                <div key={`empty-${index}`} className={styles.machineRow}>
                  <span className={styles.metaLabel}>○ 空位</span>
                </div>
              ))}
            </div>
          </section>

          <section className={styles.card}>
            <h2 style={{ margin: "0 0 0.75rem", fontSize: "1rem" }}>授权信息</h2>
            <div className={styles.metaList}>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>产品</span>
                <span>{licenseKindDisplay(license.kind)}</span>
              </div>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>状态</span>
                <span>{license.status}</span>
              </div>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>购买日</span>
                <span>{license.created_at ?? "—"}</span>
              </div>
            </div>
          </section>

          <p style={{ marginTop: "1rem" }}>
            <Link href="/licenses" className="app-link">
              ← 返回授权列表
            </Link>
          </p>
        </div>
      </main>
    </ProtectedRouteGate>
  );
}
