"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";

import { formatUiMessage } from "../lib/i18n/messages";
import type { UiLocale } from "../lib/i18n/config";
import { appNavHref } from "../lib/navigation/nav-config";
import { PlanEntry } from "./plan-entry";

/**
 * Top-bar share group (PRODUCT_IA §5): 分享 is T0; 访问 / API / 升级 fold
 * into its menu — they all serve distributing what you built.
 * App-wide variant (dashboard + deep tool pages): cross-workspace entries.
 * The workspace top bar renders its own object-level split-button variant.
 */
export function ShareAccessMenu({ locale }: { locale: UiLocale }) {
  const [menuOpen, setMenuOpen] = useState(false);
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

  return (
    <div className="dashboard-account-menu" ref={menuRef}>
      <button
        aria-expanded={menuOpen}
        aria-haspopup="menu"
        className="dashboard-header-settings top-bar-capsule"
        data-testid="app-topbar-share-menu"
        type="button"
        onClick={() => setMenuOpen((open) => !open)}
      >
        {formatUiMessage(locale, "workspaceDistribute")} ▾
      </button>
      {menuOpen ? (
        <div
          className="dashboard-account-menu-panel"
          data-testid="app-topbar-share-menu-panel"
          role="menu"
        >
          <Link
            className="dashboard-account-menu-item"
            data-testid="app-topbar-share-traffic"
            href={appNavHref("share-traffic")}
            role="menuitem"
            onClick={() => setMenuOpen(false)}
          >
            {formatUiMessage(locale, "dashboardShareTrafficNav")}
          </Link>
          <Link
            className="dashboard-account-menu-item"
            data-testid="app-topbar-share-api"
            href={appNavHref("api-access")}
            role="menuitem"
            onClick={() => setMenuOpen(false)}
          >
            {formatUiMessage(locale, "workspaceApi")}
          </Link>
          <PlanEntry
            className="dashboard-account-menu-item"
            locale={locale}
          />
        </div>
      ) : null}
    </div>
  );
}
