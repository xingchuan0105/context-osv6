"use client";

import Link from "next/link";

import {
  DesktopDownloadButton,
  DesktopReleaseDetails,
} from "@/components/desktop/DesktopDownloadButton";
import styles from "@/components/desktop/desktop.module.css";
import { MarketingShell } from "@/components/marketing-chrome";
import { brandHomeHref } from "@/components/product-chrome-footer";
import { APP_PATHS } from "@/lib/site-map";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

export default function DesktopProductPage() {
  const { locale } = useUiPreferences();
  const hub = brandHomeHref();
  const hubExternal = /^https?:\/\//i.test(hub);

  return (
    <MarketingShell active="desktop">
      <main className="app-page-shell" style={{ background: "hsl(var(--surface-muted))" }}>
        <div className={styles.marketingPage}>
          <header className={styles.marketingHeader}>
            <h1 className="app-page-title">{formatUiMessage(locale, "desktop.productTitle")}</h1>
            <p className="app-page-subtitle">{formatUiMessage(locale, "desktop.productSubtitle")}</p>
          </header>

          {/* Two-column: benefits | download + install — less whitespace, aligned CTAs */}
          <div className={styles.marketingGrid}>
            <section className={styles.card}>
              <h2 className={styles.sectionTitle}>
                {formatUiMessage(locale, "desktop.benefitsTitle")}
              </h2>
              <ul className={styles.buyFeatures}>
                <li>{formatUiMessage(locale, "desktop.feature1")}</li>
                <li>{formatUiMessage(locale, "desktop.feature2")}</li>
                <li>{formatUiMessage(locale, "desktop.feature3")}</li>
                <li>{formatUiMessage(locale, "desktop.feature4")}</li>
                <li>{formatUiMessage(locale, "desktop.feature5")}</li>
                <li>{formatUiMessage(locale, "desktop.feature6")}</li>
              </ul>
            </section>

            <div className={styles.marketingAside}>
              <section className={styles.card}>
                <h2 className={styles.sectionTitle}>
                  {formatUiMessage(locale, "desktop.ctaTitle")}
                </h2>
                <div className={styles.ctaRow} data-testid="desktop-cta-row">
                  <DesktopDownloadButton className={`app-button-primary ${styles.ctaPill}`} compact />
                  <Link
                    href={APP_PATHS.pricing}
                    className={`app-button-secondary ${styles.ctaPill}`}
                  >
                    {formatUiMessage(locale, "desktop.buyCta")}
                  </Link>
                  <Link href="/help/api-access" className={`app-button-ghost ${styles.ctaPill}`}>
                    {formatUiMessage(locale, "desktop.learnMore")}
                  </Link>
                </div>
                <DesktopReleaseDetails />
              </section>

              <section className={styles.card}>
                <h2 className={styles.sectionTitle}>
                  {formatUiMessage(locale, "desktop.installTitle")}
                </h2>
                <ol className={styles.installList}>
                  <li>{formatUiMessage(locale, "desktop.installStep1")}</li>
                  <li>{formatUiMessage(locale, "desktop.installStep2")}</li>
                  <li>{formatUiMessage(locale, "desktop.installStep3")}</li>
                </ol>
                <p className={styles.smartScreenHint}>
                  {formatUiMessage(locale, "desktop.smartScreenHint")}
                </p>
              </section>
            </div>
          </div>

          <p className={styles.marketingFooter}>
            {hubExternal ? (
              <a className="app-link" href={hub} rel="noopener noreferrer">
                {formatUiMessage(locale, "desktop.backToHub")}
              </a>
            ) : (
              <Link className="app-link" href={hub}>
                {formatUiMessage(locale, "desktop.backToHub")}
              </Link>
            )}
            <Link className="app-link" href={APP_PATHS.dashboard}>
              {formatUiMessage(locale, "desktop.openSaaS")}
            </Link>
          </p>
        </div>
      </main>
    </MarketingShell>
  );
}
