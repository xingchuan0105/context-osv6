import type { ReactNode } from "react";

import type { SettingsTab } from "./settings-tabs";

type IconProps = {
  className?: string;
};

function Base({ className, children }: IconProps & { children: ReactNode }) {
  return (
    <svg
      aria-hidden="true"
      className={className}
      fill="none"
      height="18"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth="1.75"
      viewBox="0 0 24 24"
      width="18"
    >
      {children}
    </svg>
  );
}

/** Lucide-style stroke icons (no extra dependency). */
export function IconMembership({ className }: IconProps) {
  return (
    <Base className={className}>
      <path d="M12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2z" />
    </Base>
  );
}

export function IconProfile({ className }: IconProps) {
  return (
    <Base className={className}>
      <path d="M20 21a8 8 0 0 0-16 0" />
      <circle cx="12" cy="8" r="4" />
    </Base>
  );
}

export function IconProviders({ className }: IconProps) {
  return (
    <Base className={className}>
      <rect height="7" rx="1.5" width="7" x="3" y="3" />
      <rect height="7" rx="1.5" width="7" x="14" y="3" />
      <rect height="7" rx="1.5" width="7" x="3" y="14" />
      <rect height="7" rx="1.5" width="7" x="14" y="14" />
    </Base>
  );
}

export function IconPreferences({ className }: IconProps) {
  return (
    <Base className={className}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
    </Base>
  );
}

export function IconSecurity({ className }: IconProps) {
  return (
    <Base className={className}>
      <rect height="11" rx="2" width="14" x="5" y="11" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    </Base>
  );
}

export function IconTheme({ className }: IconProps) {
  return (
    <Base className={className}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
    </Base>
  );
}

export function IconLanguage({ className }: IconProps) {
  return (
    <Base className={className}>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
    </Base>
  );
}

export function IconSettings({ className }: IconProps) {
  return (
    <Base className={className}>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </Base>
  );
}

export function IconHelp({ className }: IconProps) {
  return (
    <Base className={className}>
      <circle cx="12" cy="12" r="9" />
      <path d="M9.1 9a3 3 0 1 1 4.4 2.6c-.7.4-1.5 1-1.5 2" />
      <path d="M12 17h.01" />
    </Base>
  );
}

export function IconLogout({ className }: IconProps) {
  return (
    <Base className={className}>
      <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
      <path d="M16 17l5-5-5-5" />
      <path d="M21 12H9" />
    </Base>
  );
}

export function settingsTabIcon(tab: SettingsTab, className?: string) {
  switch (tab) {
    case "billing":
      return <IconMembership className={className} />;
    case "profile":
      return <IconProfile className={className} />;
    case "providers":
      return <IconProviders className={className} />;
    case "preferences":
      return <IconPreferences className={className} />;
    case "security":
      return <IconSecurity className={className} />;
  }
}
