"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import { formatUiMessage } from "../lib/i18n/messages";
import { useUiPreferences } from "../lib/ui-preferences";
import { ContextOsMark } from "./context-os-mark";
import { brandHomeHref } from "./product-chrome-footer";

type AuthFrameProps = {
  title: string;
  subtitle: string;
  children?: ReactNode;
};

export function AuthFrame({ title, subtitle, children }: AuthFrameProps) {
  const { locale } = useUiPreferences();
  const brandHref = brandHomeHref();
  const brandIsExternal = /^https?:\/\//i.test(brandHref);
  const brandLabel = formatUiMessage(locale, "productChrome.brandHome");

  const brandMark = brandIsExternal ? (
    <a
      className="app-auth-brand-link"
      data-testid="auth-brand-home"
      href={brandHref}
      rel="noopener noreferrer"
      title={brandLabel}
    >
      <ContextOsMark size={56} className="app-auth-mark" />
      <span className="app-auth-brand-label">{brandLabel}</span>
    </a>
  ) : (
    <Link
      className="app-auth-brand-link"
      data-testid="auth-brand-home"
      href={brandHref}
      title={brandLabel}
    >
      <ContextOsMark size={56} className="app-auth-mark" />
      <span className="app-auth-brand-label">{brandLabel}</span>
    </Link>
  );

  return (
    <main className="app-auth-shell">
      <section className="app-surface-card app-auth-card">
        <div className="app-auth-header">
          {brandMark}
          <h1 className="app-auth-title">{title}</h1>
          <p className="app-page-subtitle app-auth-subtitle">{subtitle}</p>
        </div>
        {children}
      </section>
    </main>
  );
}

type AppShellProps = {
  title: string;
  subtitle: string;
  children?: ReactNode;
};

export function AppPageFrame({ title, subtitle, children }: AppShellProps) {
  return (
    <main className="app-page-shell">
      <div className="app-page-center">
        <header className="app-page-heading">
          <h1 className="app-page-title">{title}</h1>
          <p className="app-page-subtitle">{subtitle}</p>
        </header>
        {children}
      </div>
    </main>
  );
}

export function DashboardShellPlaceholder() {
  return (
    <main className="dashboard-shell">
      <header className="dashboard-header">
        <div className="brand-row">
          <ContextOsMark size={36} />
          <div className="brand-copy">
            <div className="brand-title">Context OS</div>
            <div className="brand-subtitle">Dashboard foundation placeholder</div>
          </div>
        </div>
        <div className="app-button-row">
          <button className="app-button-ghost" type="button">
            Search
          </button>
          <button className="app-button-secondary" type="button">
            List
          </button>
          <button className="app-button-primary" type="button">
            New Workspace
          </button>
        </div>
      </header>
      <section className="app-page-shell">
        <div className="app-page-center">
          <div className="placeholder-grid">
            <div className="app-inline-surface">Workspace tabs and toolbar go here.</div>
            <div className="app-inline-surface">Card/list views will be migrated next.</div>
            <div className="app-inline-surface">Overlay search and create modal stay in scope.</div>
          </div>
        </div>
      </section>
    </main>
  );
}

export function WorkspaceShellPlaceholder() {
  return (
    <main className="workspace-shell">
      <header className="workspace-topbar">
        <div className="brand-row">
          <ContextOsMark size={36} />
          <div className="brand-copy">
            <div className="brand-title">Untitled Workspace</div>
            <div className="brand-subtitle">Three-column shell foundation placeholder</div>
          </div>
        </div>
        <div className="app-button-row">
          <button className="app-button-secondary" type="button">
            Analyze
          </button>
          <button className="app-button-secondary" type="button">
            Share
          </button>
          <button className="app-button-primary" type="button">
            API
          </button>
        </div>
      </header>
      <section className="workspace-grid">
        <aside className="workspace-panel" />
        <section className="workspace-panel" />
        <aside className="workspace-panel" />
      </section>
    </main>
  );
}
