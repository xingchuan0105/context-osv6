"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import type { UiLocale } from "../../lib/i18n/config";
import { formatUiMessage, type UiMessageKey } from "../../lib/i18n/messages";
import { AppModal } from "../ui/app-modal";
import styles from "./product-guide-modal.module.css";

export type GuideSection =
  | "overview"
  | "llm"
  | "workspace"
  | "share"
  | "client"
  | "billing"
  | "settings"
  | "graph";

const SECTIONS: GuideSection[] = [
  "overview",
  "llm",
  "workspace",
  "share",
  "client",
  "billing",
  "settings",
  "graph",
];

const NAV_KEY: Record<GuideSection, UiMessageKey> = {
  overview: "productGuide.nav.overview",
  llm: "productGuide.nav.llm",
  workspace: "productGuide.nav.workspace",
  share: "productGuide.nav.share",
  client: "productGuide.nav.client",
  billing: "productGuide.nav.billing",
  settings: "productGuide.nav.settings",
  graph: "productGuide.nav.graph",
};

type ProductGuideModalProps = {
  open: boolean;
  onClose: () => void;
  locale: UiLocale;
  initialSection?: GuideSection;
};

/**
 * First-run / product-map modal: left topic rail + linked module copy
 * (wiki / Obsidian-style graph of product surfaces).
 */
export function ProductGuideModal({
  open,
  onClose,
  locale,
  initialSection = "overview",
}: ProductGuideModalProps) {
  const [section, setSection] = useState<GuideSection>(initialSection);

  useEffect(() => {
    if (open) {
      setSection(initialSection);
    }
  }, [open, initialSection]);

  return (
    <AppModal
      open={open}
      size="lg"
      title={formatUiMessage(locale, "productGuide.title")}
      closeLabel={formatUiMessage(locale, "appModal.close")}
      fullPageHref="/help"
      fullPageLabel={formatUiMessage(locale, "productGuide.fullHelp")}
      testId="product-guide-modal"
      onClose={onClose}
    >
      <div className={styles.layout}>
        <nav className={styles.nav} aria-label={formatUiMessage(locale, "productGuide.title")}>
          {SECTIONS.map((id) => (
            <button
              key={id}
              type="button"
              className={section === id ? styles.navButtonActive : styles.navButton}
              data-testid={`product-guide-nav-${id}`}
              onClick={() => setSection(id)}
            >
              {formatUiMessage(locale, NAV_KEY[id])}
            </button>
          ))}
        </nav>
        <div className={styles.body}>{renderSection(section, locale, onClose)}</div>
      </div>
    </AppModal>
  );
}

function renderSection(section: GuideSection, locale: UiLocale, onClose: () => void) {
  switch (section) {
    case "overview":
      return (
        <>
          <p className={styles.lead}>{formatUiMessage(locale, "productGuide.overview.body")}</p>
          <h3 className={styles.heading}>{formatUiMessage(locale, "productGuide.nav.overview")}</h3>
          <ol className={styles.steps}>
            <li>{formatUiMessage(locale, "productGuide.overview.step1")}</li>
            <li>{formatUiMessage(locale, "productGuide.overview.step2")}</li>
            <li>{formatUiMessage(locale, "productGuide.overview.step3")}</li>
          </ol>
          <div className={styles.links}>
            <Link className="app-link" href="/settings?tab=providers" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.llm.linkProviders")}
            </Link>
            <Link className="app-link" href="/desktop" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.client.link")}
            </Link>
          </div>
        </>
      );
    case "llm":
      return (
        <>
          <h3 className={styles.heading}>{formatUiMessage(locale, "productGuide.llm.title")}</h3>
          <div className={styles.cardGrid}>
            <article className={styles.card}>
              <h4 className={styles.cardTitle}>
                {formatUiMessage(locale, "productGuide.llm.byokTitle")}
              </h4>
              <p className={styles.cardBody}>
                {formatUiMessage(locale, "productGuide.llm.byokBody")}
              </p>
              <Link className="app-link" href="/settings?tab=providers" onClick={onClose}>
                {formatUiMessage(locale, "productGuide.llm.linkProviders")}
              </Link>
            </article>
            <article className={styles.card}>
              <h4 className={styles.cardTitle}>
                {formatUiMessage(locale, "productGuide.llm.platformTitle")}
              </h4>
              <p className={styles.cardBody}>
                {formatUiMessage(locale, "productGuide.llm.platformBody")}
              </p>
              <Link className="app-link" href="/pricing#topup" onClick={onClose}>
                {formatUiMessage(locale, "productGuide.llm.linkBilling")}
              </Link>
            </article>
          </div>
        </>
      );
    case "workspace":
      return (
        <>
          <h3 className={styles.heading}>
            {formatUiMessage(locale, "productGuide.workspace.title")}
          </h3>
          <p className={styles.lead}>{formatUiMessage(locale, "productGuide.workspace.body")}</p>
          <div className={styles.links}>
            <Link className="app-link" href="/dashboard" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.workspace.link")}
            </Link>
          </div>
        </>
      );
    case "share":
      return (
        <>
          <h3 className={styles.heading}>{formatUiMessage(locale, "productGuide.share.title")}</h3>
          <p className={styles.lead}>{formatUiMessage(locale, "productGuide.share.body")}</p>
          <p className={styles.lead}>{formatUiMessage(locale, "productGuide.share.useCases")}</p>
          <div className={styles.links}>
            <Link className="app-link" href="/dashboard/analytics" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.share.linkAnalytics")}
            </Link>
            <Link className="app-link" href="/pricing" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.billing.linkPricing")}
            </Link>
          </div>
        </>
      );
    case "client":
      return (
        <>
          <h3 className={styles.heading}>{formatUiMessage(locale, "productGuide.client.title")}</h3>
          <p className={styles.lead}>{formatUiMessage(locale, "productGuide.client.body")}</p>
          <div className={styles.links}>
            <Link className="app-link" href="/desktop" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.client.link")}
            </Link>
            <Link className="app-link" href="/help/api-access" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.api")}
            </Link>
          </div>
        </>
      );
    case "billing":
      return (
        <>
          <h3 className={styles.heading}>
            {formatUiMessage(locale, "productGuide.billing.title")}
          </h3>
          <div className={styles.cardGrid}>
            <article className={styles.card}>
              <h4 className={styles.cardTitle}>
                {formatUiMessage(locale, "productGuide.billing.memberTitle")}
              </h4>
              <p className={styles.cardBody}>
                {formatUiMessage(locale, "productGuide.billing.memberBody")}
              </p>
            </article>
            <article className={styles.card}>
              <h4 className={styles.cardTitle}>
                {formatUiMessage(locale, "productGuide.billing.topupTitle")}
              </h4>
              <p className={styles.cardBody}>
                {formatUiMessage(locale, "productGuide.billing.topupBody")}
              </p>
            </article>
          </div>
          <div className={styles.links}>
            <Link className="app-link" href="/pricing" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.billing.linkPricing")}
            </Link>
            <Link className="app-link" href="/pricing#topup" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.llm.linkBilling")}
            </Link>
          </div>
        </>
      );
    case "settings":
      return (
        <>
          <h3 className={styles.heading}>
            {formatUiMessage(locale, "productGuide.settings.title")}
          </h3>
          <p className={styles.lead}>{formatUiMessage(locale, "productGuide.settings.body")}</p>
          <div className={styles.links}>
            <Link className="app-link" href="/settings" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.settings.link")}
            </Link>
            <Link className="app-link" href="/settings?tab=providers" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.providers")}
            </Link>
          </div>
        </>
      );
    case "graph":
      return (
        <>
          <h3 className={styles.heading}>{formatUiMessage(locale, "productGuide.graph.title")}</h3>
          <p className={styles.lead}>{formatUiMessage(locale, "productGuide.graph.hint")}</p>
          <div className={styles.graph}>
            <Link className={styles.chip} href="/help" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.help")}
            </Link>
            <Link className={styles.chip} href="/help/api-access" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.api")}
            </Link>
            <Link className={styles.chip} href="/pricing" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.pricing")}
            </Link>
            <Link className={styles.chip} href="/desktop" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.desktop")}
            </Link>
            <Link className={styles.chip} href="/dashboard/analytics" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.analytics")}
            </Link>
            <Link className={styles.chip} href="/settings?tab=providers" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.providers")}
            </Link>
            <Link className={styles.chip} href="/pricing#topup" onClick={onClose}>
              {formatUiMessage(locale, "productGuide.graph.billing")}
            </Link>
          </div>
        </>
      );
    default:
      return null;
  }
}
