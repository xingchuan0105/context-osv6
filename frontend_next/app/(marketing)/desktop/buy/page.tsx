"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";

import styles from "@/components/desktop/desktop.module.css";
import {
  createLicenseCheckout,
  fetchMyLicenses,
} from "@/lib/desktop/license-client";
import { useAuth } from "@/lib/auth/context";

const DESKTOP_TIERS = [
  {
    id: "standard",
    name: "Standard",
    priceUsd: "$39",
    priceCny: "¥299",
    seats: 1,
    features: ["1 台设备", "v1 终身免费升级", "社区支持"],
    highlight: false,
  },
  {
    id: "pro",
    name: "Pro",
    priceUsd: "$99",
    priceCny: "¥699",
    seats: 3,
    features: ["3 台设备", "v1 终身免费升级", "优先支持"],
    highlight: true,
  },
] as const;

export default function DesktopBuyPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const auth = useAuth();
  const deviceId = searchParams.get("device_id") ?? "";
  const checkoutSuccess = searchParams.get("success") === "1";
  const [purchasedKey, setPurchasedKey] = useState<string | null>(null);
  const [loadingTier, setLoadingTier] = useState<string | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!checkoutSuccess || !auth.token) return;

    let cancelled = false;

    void fetchMyLicenses(auth.token)
      .then((response) => {
        if (cancelled) return;

        const planId = searchParams.get("plan_id");
        const matched = response.licenses.find((license) =>
          planId ? license.kind.toLowerCase().includes(planId) : true,
        );

        if (matched?.key) {
          setPurchasedKey(matched.key);
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [auth.token, checkoutSuccess, searchParams]);

  async function handlePurchase(tierId: string) {
    if (!auth.isAuthenticated || !auth.token) {
      const returnUrl = `/desktop/buy${deviceId ? `?device_id=${encodeURIComponent(deviceId)}` : ""}`;
      router.push(`/login?next=${encodeURIComponent(returnUrl)}`);
      return;
    }

    setLoadingTier(tierId);
    setError("");

    try {
      const checkout = await createLicenseCheckout(auth.token, {
        plan_id: tierId,
        provider: "creem",
        device_id: deviceId || undefined,
      });

      window.location.assign(checkout.checkout_url);
    } catch (checkoutError) {
      setError(checkoutError instanceof Error ? checkoutError.message : "结账失败，请稍后重试");
      setLoadingTier(null);
    }
  }

  function copyKey() {
    if (!purchasedKey) return;
    void navigator.clipboard.writeText(purchasedKey);
  }

  return (
    <main className="app-page-shell" style={{ background: "#f6f5f4" }}>
      <div className="app-page-center" style={{ maxWidth: "42rem" }}>
        <header className="app-page-heading" style={{ textAlign: "center" }}>
          <h1 className="app-page-title">AVRag Desktop</h1>
          <p className="app-page-subtitle">本地 AI 知识助手</p>
          {deviceId ? (
            <p className="app-page-subtitle" style={{ fontSize: "var(--font-size-label)" }}>
              设备 ID: {deviceId.slice(0, 12)}…
            </p>
          ) : null}
        </header>

        {error ? (
          <p className={styles.errorBox} role="alert">
            {error}
          </p>
        ) : null}

        {!purchasedKey ? (
          <>
            <div className={styles.buyGrid}>
              {DESKTOP_TIERS.map((tier) => (
                <article
                  key={tier.id}
                  className={`${styles.buyCard} ${tier.highlight ? styles.buyCardHighlight : ""}`}
                >
                  <h2 className={styles.buyTier}>{tier.name}</h2>
                  <p className={styles.buyPrice}>
                    {tier.priceUsd} / {tier.priceCny}
                  </p>
                  <ul className={styles.buyFeatures}>
                    {tier.features.map((feature) => (
                      <li key={feature}>{feature}</li>
                    ))}
                  </ul>
                  <button
                    type="button"
                    className={tier.highlight ? "app-button-primary" : "app-button-secondary"}
                    onClick={() => void handlePurchase(tier.id)}
                    disabled={loadingTier != null}
                  >
                    {loadingTier === tier.id ? "处理中…" : "购买"}
                  </button>
                </article>
              ))}
            </div>

            <p className={styles.subtitle} style={{ textAlign: "center", marginTop: "1.25rem" }}>
              支付方式：信用卡（Creem）/ 支付宝
            </p>
          </>
        ) : (
          <section className={styles.card}>
            <header className={styles.header}>
              <div className={styles.successIcon} aria-hidden="true">
                ✓
              </div>
              <h2 className={styles.title}>购买成功</h2>
            </header>

            <p className={styles.subtitle}>你的授权码：</p>
            <div className="app-button-row" style={{ marginBottom: "1rem" }}>
              <input className="app-input" readOnly value={purchasedKey} style={{ flex: 1 }} />
              <button type="button" className="app-button-secondary" onClick={copyKey}>
                复制
              </button>
            </div>

            <a
              href={`avrag-desktop://activate?key=${encodeURIComponent(purchasedKey)}`}
              className="app-button-primary"
              style={{ display: "inline-flex", marginBottom: "0.75rem" }}
            >
              在 AVRag Desktop 中激活
            </a>

            <p className={styles.subtitle}>或手动输入授权码激活</p>
          </section>
        )}
      </div>
    </main>
  );
}
