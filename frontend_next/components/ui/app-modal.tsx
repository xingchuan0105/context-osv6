"use client";

import Link from "next/link";
import { useEffect, type ReactNode } from "react";

import styles from "./app-modal.module.css";

export type AppModalSize = "sm" | "md" | "lg" | "xl";

export type AppModalProps = {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  size?: AppModalSize;
  /**
   * "rail": body becomes a two-column grid (NavRail | content) with no body
   * padding — Grok-style settings modal. Children should be [NavRail, content].
   */
  bodyVariant?: "default" | "rail";
  /** Optional full-page deep link shown in the footer. */
  fullPageHref?: string;
  fullPageLabel?: string;
  footer?: ReactNode;
  testId?: string;
  closeLabel?: string;
};

const sizeClass: Record<AppModalSize, string> = {
  sm: styles.cardSm,
  md: styles.cardMd,
  lg: styles.cardLg,
  xl: styles.cardXl,
};

export function AppModal({
  open,
  title,
  onClose,
  children,
  size = "md",
  bodyVariant = "default",
  fullPageHref,
  fullPageLabel,
  footer,
  testId,
  closeLabel = "Close",
}: AppModalProps) {
  useEffect(() => {
    if (!open) {
      return;
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    }

    document.addEventListener("keydown", onKeyDown);
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousOverflow;
    };
  }, [open, onClose]);

  if (!open) {
    return null;
  }

  const showFooter = Boolean(footer) || Boolean(fullPageHref && fullPageLabel);

  return (
    <div
      className={styles.backdrop}
      data-testid={testId}
      role="presentation"
      onClick={onClose}
    >
      <section
        aria-label={title}
        aria-modal="true"
        className={`${styles.card} ${sizeClass[size]}`}
        role="dialog"
        onClick={(event) => event.stopPropagation()}
      >
        <header className={styles.header}>
          <h2 className={styles.title}>{title}</h2>
          <button
            aria-label={closeLabel}
            className={styles.closeButton}
            type="button"
            onClick={onClose}
          >
            <svg
              aria-hidden="true"
              className={styles.closeIcon}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path d="M6 6l12 12M18 6 6 18" strokeLinecap="round" strokeWidth="1.8" />
            </svg>
          </button>
        </header>
        <div className={`${styles.body}${bodyVariant === "rail" ? ` ${styles.bodyRail}` : ""}`}>
          {children}
        </div>
        {showFooter ? (
          <footer className={styles.footer}>
            {fullPageHref && fullPageLabel ? (
              <Link className={`app-link app-link-muted ${styles.footerLink}`} href={fullPageHref}>
                {fullPageLabel}
              </Link>
            ) : (
              <span />
            )}
            {footer}
          </footer>
        ) : null}
      </section>
    </div>
  );
}
