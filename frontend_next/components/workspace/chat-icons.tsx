/** Shared 1.5–1.75 stroke line icons for chat chrome (U12). */

import type { ReactNode } from "react";

type IconProps = {
  className?: string;
  title?: string;
};

const STROKE = 1.7;

function BaseIcon({ className, title, children }: IconProps & { children: ReactNode }) {
  return (
    <svg
      aria-hidden={title ? undefined : true}
      className={className}
      fill="none"
      role={title ? "img" : undefined}
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={STROKE}
      viewBox="0 0 24 24"
    >
      {title ? <title>{title}</title> : null}
      {children}
    </svg>
  );
}

export function IconCopy({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <rect height="12" rx="2" width="12" x="8" y="8" />
      <path d="M6 16V6a2 2 0 0 1 2-2h10" />
    </BaseIcon>
  );
}

export function IconEdit({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M4 20h4l10.5-10.5a2.1 2.1 0 0 0-3-3L5 17v3Z" />
      <path d="m13.5 6.5 3 3" />
    </BaseIcon>
  );
}

export function IconNote({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M7 4h7l4 4v12H7V4Z" />
      <path d="M14 4v4h4M9 12h6M9 16h4" />
    </BaseIcon>
  );
}

export function IconRegenerate({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M4.5 12a7.5 7.5 0 0 1 12.7-5.4L19 4v5h-5" />
      <path d="M19.5 12a7.5 7.5 0 0 1-12.7 5.4L5 20v-5h5" />
    </BaseIcon>
  );
}

export function IconThumbUp({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M7 11v9H4.5A1.5 1.5 0 0 1 3 18.5v-6A1.5 1.5 0 0 1 4.5 11H7Z" />
      <path d="M7 11 10.2 4.8A2 2 0 0 1 12 4h.4a2 2 0 0 1 1.9 2.5L13.8 11H19a2 2 0 0 1 2 2.3l-1 6A2 2 0 0 1 18 21H7" />
    </BaseIcon>
  );
}

export function IconThumbDown({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M17 13V4h2.5A1.5 1.5 0 0 1 21 5.5v6A1.5 1.5 0 0 1 19.5 13H17Z" />
      <path d="M17 13 13.8 19.2A2 2 0 0 1 12 20h-.4a2 2 0 0 1-1.9-2.5L10.2 13H5a2 2 0 0 1-2-2.3l1-6A2 2 0 0 1 6 3h11" />
    </BaseIcon>
  );
}

export function IconSend({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M12 18V6" />
      <path d="m7.5 10.5 4.5-4.5 4.5 4.5" />
    </BaseIcon>
  );
}

export function IconStop({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <rect height="10" rx="1.5" width="10" x="7" y="7" />
    </BaseIcon>
  );
}

export function IconChevronUp({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="m7 14 5-5 5 5" />
    </BaseIcon>
  );
}

export function IconChatEmpty({ className }: IconProps) {
  return (
    <BaseIcon className={className}>
      <path d="M5 6.5A2.5 2.5 0 0 1 7.5 4h9A2.5 2.5 0 0 1 19 6.5v7A2.5 2.5 0 0 1 16.5 16H12l-4 3.5V16H7.5A2.5 2.5 0 0 1 5 13.5v-7Z" />
    </BaseIcon>
  );
}
