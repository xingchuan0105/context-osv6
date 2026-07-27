"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";

import { useAuth } from "../lib/auth/context";
import { formatUiMessage } from "../lib/i18n/messages";
import type { UiLocale } from "../lib/i18n/config";
import {
  SettingsQuickModal,
  type SettingsQuickTab,
} from "./settings/settings-quick-modal";

/**
 * Unified account menu shared by the dashboard header and the workspace top
 * bar. Styles come from globals.css (.dashboard-account-menu* /
 * .dashboard-header-settings) so both surfaces render identically.
 */
export function AccountMenu({ locale }: { locale: UiLocale }) {
  const auth = useAuth();
  const router = useRouter();
  const [menuOpen, setMenuOpen] = useState(false);
  const [quickTab, setQuickTab] = useState<SettingsQuickTab | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);

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

  function openQuick(tab: SettingsQuickTab) {
    setMenuOpen(false);
    setQuickTab(tab);
  }

  return (
    <>
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
            <button
              className="dashboard-account-menu-item"
              role="menuitem"
              type="button"
              onClick={() => openQuick("profile")}
            >
              {formatUiMessage(locale, "dashboardProfileLink")}
            </button>
            <button
              className="dashboard-account-menu-item"
              role="menuitem"
              type="button"
              onClick={() => openQuick("billing")}
            >
              {formatUiMessage(locale, "dashboardBillingLink")}
            </button>
            <button
              className="dashboard-account-menu-item"
              role="menuitem"
              type="button"
              onClick={() => openQuick("appearance")}
            >
              {formatUiMessage(locale, "dashboardAppearanceLink")}
            </button>
            <Link
              className="dashboard-account-menu-item"
              href="/settings?tab=security"
              role="menuitem"
              onClick={() => setMenuOpen(false)}
            >
              {formatUiMessage(locale, "settingsQuickModal.securityLink")}
            </Link>
            <Link
              className="dashboard-account-menu-item"
              href="/settings?tab=notifications"
              role="menuitem"
              onClick={() => setMenuOpen(false)}
            >
              {formatUiMessage(locale, "settingsQuickModal.notificationsLink")}
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
      <SettingsQuickModal
        locale={locale}
        open={quickTab !== null}
        tab={quickTab}
        onClose={() => setQuickTab(null)}
      />
    </>
  );
}
