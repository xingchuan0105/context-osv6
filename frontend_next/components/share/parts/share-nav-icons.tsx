import type { ReactNode } from "react";

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

/** Lucide-style stroke icons for the share nav rail (no extra dependency). */
export function IconShareControls({ className }: IconProps) {
  return (
    <Base className={className}>
      <circle cx="18" cy="5" r="3" />
      <circle cx="6" cy="12" r="3" />
      <circle cx="18" cy="19" r="3" />
      <path d="m8.59 13.51 6.83 3.98M15.41 6.51l-6.82 3.98" />
    </Base>
  );
}

export function IconInvite({ className }: IconProps) {
  return (
    <Base className={className}>
      <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
      <circle cx="9" cy="7" r="4" />
      <path d="M19 8v6M22 11h-6" />
    </Base>
  );
}

export function IconApi({ className }: IconProps) {
  return (
    <Base className={className}>
      <path d="m16 18 6-6-6-6M8 6l-6 6 6 6" />
    </Base>
  );
}

export function IconTraffic({ className }: IconProps) {
  return (
    <Base className={className}>
      <path d="M3 3v18h18" />
      <path d="M7 15v3M12 10v8M17 6v12" />
    </Base>
  );
}

export function IconOwnerProfile({ className }: IconProps) {
  return (
    <Base className={className}>
      <circle cx="12" cy="8" r="4" />
      <path d="M4 21a8 8 0 0 1 16 0" />
    </Base>
  );
}
