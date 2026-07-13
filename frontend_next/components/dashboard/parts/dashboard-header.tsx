"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";

import { ContextOsMark } from "../../context-os-mark";
import { brandHomeHref } from "../../product-chrome-footer";
import { useAuth } from "../../../lib/auth/context";
import { type DashboardLocale } from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";

export function DashboardHeader({
  avatarInitial: _avatarInitial,
  locale,
}: {
  /** Reserved for optional avatar badge; product keeps account text only. */
  avatarInitial: string;
  locale: DashboardLocale;
}) {
  const auth = useAuth();
  const router = useRouter();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);

  const brandHref = brandHomeHref();
  const brandIsExternal = /^https?:\/\//i.test(brandHref);

  useEffect(() => {
    if (!menuOpen) {
      return;
    }
    function onPointerDown(event: MouseEvent) {
      if (!menuRef.current?.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setMenuOpen(false);
      }
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [menuOpen]);

  async function handleLogout() {
    setMenuOpen(false);
    await auth.logout();
    router.replace("/login");
  }

  return (
    <header className="dashboard-header">
      <div className="dashboard-brand">
        {brandIsExternal ? (
          <a
            className="dashboard-brand-link"
            href={brandHref}
            rel="noopener noreferrer"
            target="_blank"
            title={formatUiMessage(locale, "productChrome.brandHome")}
          >
            <ContextOsMark className="dashboard-brand-mark" />
          </a>
        ) : (
          <Link
            className="dashboard-brand-link"
            href={brandHref}
            title={formatUiMessage(locale, "productChrome.brandHome")}
          >
            <ContextOsMark className="dashboard-brand-mark" />
          </Link>
        )}
        <div>
          <Link className="dashboard-brand-title" href="/dashboard">
            Context-OS
          </Link>
          <div className="dashboard-brand-subtitle">{formatUiMessage(locale, "dashboardBrandSubtitle")}</div>
        </div>
      </div>
      <div className="dashboard-header-links">
        <div className="dashboard-account-menu" ref={menuRef}>
          <button
            aria-expanded={menuOpen}
            aria-haspopup="menu"
            aria-label={formatUiMessage(locale, "dashboardAccountLink")}
            className="dashboard-header-settings"
            data-testid="dashboard-account-menu-trigger"
            type="button"
            onClick={() => setMenuOpen((open) => !open)}
          >
            <svg
              aria-hidden="true"
              className="dashboard-header-icon"
              fill="none"
              stroke="currentColor"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="1.8"
              viewBox="0 0 24 24"
            >
              <path d="M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z" />
              <path d="M4 20c0-3.3 3.6-6 8-6s8 2.7 8 6" strokeLinecap="round" />
            </svg>
            <span>{formatUiMessage(locale, "dashboardAccountLink")}</span>
          </button>
          {menuOpen ? (
            <div
              className="dashboard-account-menu-panel"
              data-testid="dashboard-account-menu"
              role="menu"
            >
              <Link
                className="dashboard-account-menu-item"
                href="/settings?tab=profile"
                role="menuitem"
                onClick={() => setMenuOpen(false)}
              >
                {formatUiMessage(locale, "dashboardProfileLink")}
              </Link>
              <Link
                className="dashboard-account-menu-item"
                href="/settings?tab=billing"
                role="menuitem"
                onClick={() => setMenuOpen(false)}
              >
                {formatUiMessage(locale, "dashboardBillingLink")}
              </Link>
              <button
                className="dashboard-account-menu-item dashboard-account-menu-danger"
                data-testid="dashboard-logout"
                role="menuitem"
                type="button"
                onClick={() => void handleLogout()}
              >
                {formatUiMessage(locale, "dashboardLogout")}
              </button>
            </div>
          ) : null}
        </div>
      </div>
    </header>
  );
}
