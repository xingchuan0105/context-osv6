"use client";

import Link from "next/link";
import { useMemo, useState, type ReactNode } from "react";

import styles from "./nav-rail.module.css";

export type NavRailItem = {
  id: string;
  label: string;
  icon?: ReactNode;
  /** Link mode (route navigation) when set; otherwise button + onSelect. */
  href?: string;
};

type NavRailProps = {
  items: NavRailItem[];
  activeId: string;
  ariaLabel: string;
  onSelect?: (id: string) => void;
  /** Optional search filter row (settings rail). */
  searchPlaceholder?: string;
  searchAriaLabel?: string;
  searchTestId?: string;
  testId?: string;
};

/**
 * Grok-style left rail for settings-like surfaces: quiet grouped list,
 * active item = soft card. Shared by settings page/quick modal and share
 * modal/center so every 设置页/功能页 has the same 左导航右内容 pattern.
 */
export function NavRail({
  items,
  activeId,
  ariaLabel,
  onSelect,
  searchPlaceholder,
  searchAriaLabel,
  searchTestId,
  testId,
}: NavRailProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return items;
    }
    return items.filter((item) => item.label.toLowerCase().includes(needle));
  }, [items, query]);

  return (
    <div className={styles.navRail} data-testid={testId}>
      {searchPlaceholder ? (
        <label className={styles.searchLabel}>
          <span className={styles.srOnly}>{searchAriaLabel ?? searchPlaceholder}</span>
          <input
            className={`app-input ${styles.searchInput}`}
            data-testid={searchTestId}
            placeholder={searchPlaceholder}
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
      ) : null}
      <nav aria-label={ariaLabel} className={styles.navList}>
        {filtered.map((item) => {
          const className = `${styles.navItem}${item.id === activeId ? ` ${styles.navItemActive}` : ""}`;
          const inner = (
            <>
              {item.icon ? <span className={styles.navItemIcon}>{item.icon}</span> : null}
              <span>{item.label}</span>
            </>
          );
          return item.href ? (
            <Link
              aria-current={item.id === activeId ? "page" : undefined}
              className={className}
              href={item.href}
              key={item.id}
            >
              {inner}
            </Link>
          ) : (
            <button
              aria-current={item.id === activeId ? "page" : undefined}
              className={className}
              key={item.id}
              type="button"
              onClick={() => onSelect?.(item.id)}
            >
              {inner}
            </button>
          );
        })}
      </nav>
    </div>
  );
}
