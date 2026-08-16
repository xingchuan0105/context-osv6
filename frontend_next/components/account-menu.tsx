"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";

import { useQuery } from "@tanstack/react-query";

import { useAuth } from "../lib/auth/context";
import { probeAdminAccess } from "../lib/admin/client";
import { formatUiMessage } from "@/lib/i18n/messages";
import type { UiLocale } from "@/lib/i18n/config";
import { cloudLogout, getCloudSession } from "@/lib/desktop/tauri-cloud";
import { isTauri } from "@/lib/runtime/tauri-ipc";
import { getSubscription } from "../lib/settings/client";
import { useUiPreferences } from "../lib/ui-preferences";
import {
  SettingsQuickModal,
  type SettingsQuickTab,
} from "./settings/settings-quick-modal";
import {
  IconHelp,
  IconLanguage,
  IconLogout,
  IconMembership,
  IconPreferences,
  IconProfile,
  IconSecurity,
  IconSettings,
} from "./settings/settings-nav-icons";

/**
 * Unified account menu (dashboard + workspace top bar).
 * W3: user card, widen panel, logout stays here; appearance/language flyouts.
 */
export function AccountMenu({ locale }: { locale: UiLocale }) {
  const auth = useAuth();
  const router = useRouter();
  const { theme, setTheme, setLocale } = useUiPreferences();
  const [mode, setMode] = useState<"unknown" | "web" | "desktop">("unknown");
  const [menuOpen, setMenuOpen] = useState(false);
  const [flyout, setFlyout] = useState<"theme" | "locale" | null>(null);
  const [quickTab, setQuickTab] = useState<SettingsQuickTab | null>(null);
  const [logoutError, setLogoutError] = useState("");
  const menuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setMode(isTauri() ? "desktop" : "web");
  }, []);

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
    setLogoutError("");
    if (mode === "desktop") {
      // Desktop identity is the cloud session: logout = cloud logout, the
      // shell reloads back into the cloud login gate (never web /login).
      try {
        await cloudLogout();
      } catch (error) {
        setLogoutError(error instanceof Error ? error.message : String(error));
        return;
      }
      setMenuOpen(false);
      window.location.reload();
      return;
    }
    setMenuOpen(false);
    await auth.logout();
    router.replace("/login");
  }

  function openQuick(tab: SettingsQuickTab) {
    setMenuOpen(false);
    setQuickTab(tab);
  }

  // Desktop: identity comes from the cloud session; the auto-provisioned
  // local B2C account is data-plane infrastructure, never a user identity.
  const cloudSessionQuery = useQuery({
    queryKey: ["account-menu-cloud-session"],
    enabled: mode === "desktop",
    staleTime: 60_000,
    queryFn: () => getCloudSession(),
  });
  const identityUser =
    mode === "desktop" ? cloudSessionQuery.data?.user ?? null : auth.user;
  const displayName =
    identityUser?.full_name?.trim() ||
    identityUser?.email?.split("@")[0] ||
    formatUiMessage(locale, "dashboardAccountLink");
  const email = identityUser?.email ?? "";
  const isWeb = mode === "web";
  const subscriptionQuery = useQuery({
    queryKey: ["account-menu-subscription", auth.token],
    enabled: isWeb && Boolean(auth.token),
    staleTime: 60_000,
    queryFn: async () => {
      try {
        return await getSubscription(auth.token as string);
      } catch {
        return null;
      }
    },
  });
  // Platform-admin entry: backend roles (super/ops/finance_admin) → 403 probe.
  const adminProbe = useQuery({
    queryKey: ["account-menu-admin-access", auth.token],
    enabled: isWeb && Boolean(auth.token),
    staleTime: 5 * 60_000,
    retry: false,
    queryFn: () => probeAdminAccess(auth.token as string),
  });
  const planId = subscriptionQuery.data?.plan_id?.trim().toLowerCase() || "free";
  const planBadge =
    planId === "pro" || planId.startsWith("pro")
      ? "Pro"
      : planId === "plus" || planId.startsWith("plus")
        ? "Plus"
        : "Free";

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
              <div className="dashboard-account-user-card-main">
                <div className="dashboard-account-user-avatar" aria-hidden="true">
                  {(displayName[0] || "U").toUpperCase()}
                </div>
                <div className="dashboard-account-user-meta">
                  <div className="dashboard-account-user-name-row">
                    <strong className="dashboard-account-user-name">{displayName}</strong>
                    {isWeb ? (
                      <span
                        className="dashboard-account-plan-badge"
                        data-plan={planBadge.toLowerCase()}
                        data-testid="account-plan-badge"
                      >
                        {planBadge}
                      </span>
                    ) : null}
                  </div>
                  {email ? (
                    <span className="dashboard-account-user-email">{email}</span>
                  ) : null}
                </div>
              </div>
              {/* Red-box CTA: open membership/usage quick surface (upgrade lives on /pricing). */}
              <button
                className="dashboard-account-membership-btn"
                data-testid="account-membership-cta"
                type="button"
                onClick={() => openQuick("billing")}
              >
                {planBadge === "Free"
                  ? formatUiMessage(locale, "accountMenu.upgradeMembership")
                  : formatUiMessage(locale, "accountMenu.manageMembership")}
              </button>
            </div>

            <button
              className="dashboard-account-menu-item"
              role="menuitem"
              type="button"
              onClick={() => openQuick("billing")}
            >
              <IconMembership className="dashboard-account-menu-icon" />
              {formatUiMessage(locale, "dashboardBillingLink")}
            </button>
            <button
              className="dashboard-account-menu-item"
              role="menuitem"
              type="button"
              onClick={() => openQuick("profile")}
            >
              <IconProfile className="dashboard-account-menu-icon" />
              {formatUiMessage(locale, "dashboardProfileLink")}
            </button>

            <div className="dashboard-account-flyout-wrap">
              <button
                className="dashboard-account-menu-item"
                role="menuitem"
                type="button"
                onClick={() => setFlyout((f) => (f === "theme" ? null : "theme"))}
              >
                <IconPreferences className="dashboard-account-menu-icon" />
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
                <IconLanguage className="dashboard-account-menu-icon" />
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
              <IconSecurity className="dashboard-account-menu-icon" />
              {formatUiMessage(locale, "settingsQuickModal.securityLink")}
            </Link>
            <Link
              className="dashboard-account-menu-item"
              href="/settings"
              role="menuitem"
              onClick={() => setMenuOpen(false)}
            >
              <IconSettings className="dashboard-account-menu-icon" />
              {formatUiMessage(locale, "accountMenu.allSettings")}
            </Link>
            {adminProbe.data === true ? (
              <Link
                className="dashboard-account-menu-item"
                data-testid="account-menu-admin"
                href="/admin"
                role="menuitem"
                onClick={() => setMenuOpen(false)}
              >
                <IconSecurity className="dashboard-account-menu-icon" />
                {formatUiMessage(locale, "accountMenu.adminConsole")}
              </Link>
            ) : null}
            <Link
              className="dashboard-account-menu-item"
              href="/help"
              role="menuitem"
              onClick={() => setMenuOpen(false)}
            >
              <IconHelp className="dashboard-account-menu-icon" />
              {formatUiMessage(locale, "accountMenu.help")}
            </Link>

            {logoutError ? (
              <p
                role="alert"
                style={{
                  margin: 0,
                  padding: "0 0.75rem",
                  fontSize: "0.8rem",
                  color: "hsl(var(--destructive))",
                }}
              >
                {logoutError}
              </p>
            ) : null}
            <button
              className="dashboard-account-menu-item dashboard-account-menu-danger"
              data-testid="dashboard-logout"
              role="menuitem"
              type="button"
              onClick={() => void handleLogout()}
            >
              <IconLogout className="dashboard-account-menu-icon" />
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
