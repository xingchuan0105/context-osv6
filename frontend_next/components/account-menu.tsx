"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";

import { useAuth } from "../lib/auth/context";
import { formatUiMessage } from "../lib/i18n/messages";
import type { UiLocale } from "../lib/i18n/config";
import { useUiPreferences } from "../lib/ui-preferences";
import {
  SettingsQuickModal,
  type SettingsQuickTab,
} from "./settings/settings-quick-modal";

/**
 * Unified account menu (dashboard + workspace top bar).
 * W3: user card, widen panel, logout stays here; appearance/language flyouts.
 */
export function AccountMenu({ locale }: { locale: UiLocale }) {
  const auth = useAuth();
  const router = useRouter();
  const { theme, setTheme, setLocale } = useUiPreferences();
  const [menuOpen, setMenuOpen] = useState(false);
  const [flyout, setFlyout] = useState<"theme" | "locale" | null>(null);
  const [quickTab, setQuickTab] = useState<SettingsQuickTab | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!menuOpen) {
      setFlyout(null);
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

  const displayName =
    auth.user?.full_name?.trim() ||
    auth.user?.email?.split("@")[0] ||
    formatUiMessage(locale, "dashboardAccountLink");
  const email = auth.user?.email ?? "";

  return (
    <>
      <div className="dashboard-account-menu" ref={menuRef}>
        <button
          aria-expanded={menuOpen}
          aria-haspopup="menu"
          aria-label={formatUiMessage(locale, "dashboardAccountLink")}
          className="dashboard-header-settings top-bar-capsule"
          data-testid="dashboard-account-menu-trigger"
          type="button"
          onClick={() => setMenuOpen((open) => !open)}
        >
          <span className="dashboard-account-trigger-label">
            {formatUiMessage(locale, "dashboardAccountLink")}
          </span>
        </button>
        {menuOpen ? (
          <div
            className="dashboard-account-menu-panel"
            data-testid="dashboard-account-menu"
            role="menu"
          >
            <div className="dashboard-account-user-card" data-testid="account-user-card">
              <div className="dashboard-account-user-avatar" aria-hidden="true">
                {(displayName[0] || "U").toUpperCase()}
              </div>
              <div className="dashboard-account-user-meta">
                <strong className="dashboard-account-user-name">{displayName}</strong>
                {email ? (
                  <span className="dashboard-account-user-email">{email}</span>
                ) : null}
              </div>
            </div>

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
              onClick={() => openQuick("profile")}
            >
              {formatUiMessage(locale, "dashboardProfileLink")}
            </button>
            <button
              className="dashboard-account-menu-item"
              role="menuitem"
              type="button"
              onClick={() => openQuick("preferences")}
            >
              {formatUiMessage(locale, "dashboardAppearanceLink")}
            </button>

            <div className="dashboard-account-flyout-wrap">
              <button
                className="dashboard-account-menu-item"
                role="menuitem"
                type="button"
                onClick={() => setFlyout((f) => (f === "theme" ? null : "theme"))}
              >
                {formatUiMessage(locale, "settings.appearance.themeLabel")} ▸
              </button>
              {flyout === "theme" ? (
                <div className="dashboard-account-flyout" role="menu">
                  {(["system", "light", "dark"] as const).map((value) => (
                    <button
                      className="dashboard-account-menu-item"
                      key={value}
                      role="menuitem"
                      type="button"
                      onClick={() => {
                        setTheme(value);
                        setFlyout(null);
                      }}
                    >
                      {theme === value ? "✓ " : ""}
                      {formatUiMessage(
                        locale,
                        value === "system"
                          ? "settings.appearance.theme.system"
                          : value === "light"
                            ? "settings.appearance.theme.light"
                            : "settings.appearance.theme.dark",
                      )}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>

            <div className="dashboard-account-flyout-wrap">
              <button
                className="dashboard-account-menu-item"
                role="menuitem"
                type="button"
                onClick={() => setFlyout((f) => (f === "locale" ? null : "locale"))}
              >
                {formatUiMessage(locale, "settings.appearance.localeLabel")} ▸
              </button>
              {flyout === "locale" ? (
                <div className="dashboard-account-flyout" role="menu">
                  <button
                    className="dashboard-account-menu-item"
                    role="menuitem"
                    type="button"
                    onClick={() => {
                      setLocale("zh-CN");
                      setFlyout(null);
                    }}
                  >
                    {locale === "zh-CN" ? "✓ " : ""}
                    {formatUiMessage(locale, "workspaceLanguageChinese")}
                  </button>
                  <button
                    className="dashboard-account-menu-item"
                    role="menuitem"
                    type="button"
                    onClick={() => {
                      setLocale("en");
                      setFlyout(null);
                    }}
                  >
                    {locale === "en" ? "✓ " : ""}
                    {formatUiMessage(locale, "workspaceLanguageEnglish")}
                  </button>
                </div>
              ) : null}
            </div>

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
              href="/settings"
              role="menuitem"
              onClick={() => setMenuOpen(false)}
            >
              {locale === "zh-CN" ? "所有设置" : "All settings"}
            </Link>
            <Link
              className="dashboard-account-menu-item"
              href="/help"
              role="menuitem"
              onClick={() => setMenuOpen(false)}
            >
              {locale === "zh-CN" ? "帮助" : "Help"}
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
