/** Shared status icons (check / warning / error / clock / circle / bulb) — stroke line style. */

import type { ReactNode } from "react";

type IconProps = {
  className?: string;
  title?: string;
};

const STROKE = 1.8;

function BaseIcon({ className, title, children }: IconProps & { children: ReactNode }) {
  return (
    <svg
      aria-hidden={title ? undefined : true}
      className={className}
      fill="none"
      height="1em"
      role={title ? "img" : undefined}
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={STROKE}
      viewBox="0 0 24 24"
      width="1em"
    >
      {title ? <title>{title}</title> : null}
      {children}
    </svg>
  );
}

export function IconStatusCheck({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="m4.5 12.5 5 5 10-11" />
    </BaseIcon>
  );
}

export function IconStatusWarning({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M12 4 21 20H3Z" />
      <path d="M12 10v4" />
      <path d="M12 17h.01" />
    </BaseIcon>
  );
}

export function IconStatusError({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="m9 9 6 6M15 9l-6 6" />
    </BaseIcon>
  );
}

export function IconStatusClock({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7v5l3 2" />
    </BaseIcon>
  );
}

export function IconStatusCircle({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <circle cx="12" cy="12" r="7" />
    </BaseIcon>
  );
}

export function IconStatusBulb({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M9 18h6M10 21h4" />
      <path d="M12 3a6 6 0 0 0-3.4 10.9c.8.6 1.4 1.5 1.4 2.6V17h4v-.5c0-1.1.6-2 1.4-2.6A6 6 0 0 0 12 3Z" />
    </BaseIcon>
  );
}
