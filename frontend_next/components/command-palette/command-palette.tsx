"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { formatUiMessage, type UiMessageKey } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./command-palette.module.css";

type CommandItem = {
  id: string;
  group: "nav" | "billing" | "help";
  labelKey: UiMessageKey;
  href: string;
  keywords: string;
};

const COMMANDS: CommandItem[] = [
  {
    id: "dashboard",
    group: "nav",
    labelKey: "commandPalette.item.dashboard",
    href: "/dashboard",
    keywords: "dashboard workspaces home 工作台",
  },
  {
    id: "share-traffic",
    group: "nav",
    labelKey: "commandPalette.item.shareTraffic",
    href: "/dashboard/analytics",
    keywords: "share analytics traffic views 分享 访问",
  },
  {
    id: "settings",
    group: "nav",
    labelKey: "commandPalette.item.settings",
    href: "/settings",
    keywords: "settings profile preferences 设置",
  },
  {
    id: "providers",
    group: "nav",
    labelKey: "commandPalette.item.providers",
    href: "/settings?tab=providers",
    keywords: "providers byok llm key 模型 密钥",
  },
  {
    id: "billing",
    group: "billing",
    labelKey: "commandPalette.item.billing",
    href: "/settings?tab=billing",
    keywords: "billing wallet balance 账单 余额",
  },
  {
    id: "pricing",
    group: "billing",
    labelKey: "commandPalette.item.pricing",
    href: "/pricing",
    keywords: "pricing plan membership upgrade 定价 会员 升级",
  },
  {
    id: "topup",
    group: "billing",
    labelKey: "commandPalette.item.topup",
    href: "/pricing#topup",
    keywords: "topup recharge wallet 充值 余额",
  },
  {
    id: "desktop",
    group: "help",
    labelKey: "commandPalette.item.desktop",
    href: "/desktop",
    keywords: "desktop client download mcp 客户端 下载",
  },
  {
    id: "help",
    group: "help",
    labelKey: "commandPalette.item.help",
    href: "/help",
    keywords: "help guide docs 帮助 上手",
  },
  {
    id: "api-access",
    group: "help",
    labelKey: "commandPalette.item.apiAccess",
    href: "/help/api-access",
    keywords: "api agent mcp cli access 接入",
  },
];

const GROUP_ORDER: Array<CommandItem["group"]> = ["nav", "billing", "help"];

const GROUP_LABEL: Record<CommandItem["group"], UiMessageKey> = {
  nav: "commandPalette.group.nav",
  billing: "commandPalette.group.billing",
  help: "commandPalette.group.help",
};

function matches(item: CommandItem, query: string, label: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) {
    return true;
  }
  return (
    label.toLowerCase().includes(q) ||
    item.keywords.toLowerCase().includes(q) ||
    item.href.toLowerCase().includes(q)
  );
}

/**
 * PRODUCT_IA P2-3: Cmd/Ctrl+K jump palette to canonical product routes.
 */
export function CommandPaletteHost() {
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const filtered = useMemo(() => {
    return COMMANDS.filter((item) =>
      matches(item, query, formatUiMessage(locale, item.labelKey)),
    );
  }, [locale, query]);

  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
    setActiveIndex(0);
  }, []);

  const run = useCallback(
    (item: CommandItem) => {
      close();
      router.push(item.href);
    },
    [close, router],
  );

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const isToggle =
        (event.key === "k" || event.key === "K") && (event.metaKey || event.ctrlKey);
      if (isToggle) {
        // Ignore when typing in editable fields unless palette already open.
        const target = event.target as HTMLElement | null;
        const tag = target?.tagName?.toLowerCase();
        const editing =
          tag === "input" ||
          tag === "textarea" ||
          tag === "select" ||
          target?.isContentEditable;
        if (editing && !open) {
          return;
        }
        event.preventDefault();
        setOpen((current) => !current);
        return;
      }
    }

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const id = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  useEffect(() => {
    setActiveIndex(0);
  }, [query, open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previousOverflow;
    };
  }, [open]);

  if (!open) {
    return null;
  }

  function onPanelKeyDown(event: React.KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, Math.max(filtered.length - 1, 0)));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const item = filtered[activeIndex];
      if (item) {
        run(item);
      }
    }
  }

  let flatIndex = -1;

  return (
    <div
      className={styles.backdrop}
      data-testid="command-palette"
      role="presentation"
      onClick={close}
    >
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={formatUiMessage(locale, "commandPalette.title")}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={onPanelKeyDown}
      >
        <input
          ref={inputRef}
          className={styles.input}
          data-testid="command-palette-input"
          placeholder={formatUiMessage(locale, "commandPalette.placeholder")}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <div className={styles.list} role="listbox">
          {filtered.length === 0 ? (
            <p className={styles.empty}>{formatUiMessage(locale, "commandPalette.empty")}</p>
          ) : (
            GROUP_ORDER.map((group) => {
              const items = filtered.filter((item) => item.group === group);
              if (items.length === 0) {
                return null;
              }
              return (
                <div key={group}>
                  <p className={styles.groupLabel}>
                    {formatUiMessage(locale, GROUP_LABEL[group])}
                  </p>
                  {items.map((item) => {
                    flatIndex += 1;
                    const index = flatIndex;
                    const active = index === activeIndex;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        role="option"
                        aria-selected={active}
                        className={active ? styles.itemActive : styles.item}
                        data-testid={`command-palette-item-${item.id}`}
                        onMouseEnter={() => setActiveIndex(index)}
                        onClick={() => run(item)}
                      >
                        <span>{formatUiMessage(locale, item.labelKey)}</span>
                        <span className={styles.itemHint}>{item.href}</span>
                      </button>
                    );
                  })}
                </div>
              );
            })
          )}
        </div>
        <div className={styles.footer}>
          <span>{formatUiMessage(locale, "commandPalette.title")}</span>
          <span>{formatUiMessage(locale, "commandPalette.hint")}</span>
        </div>
      </div>
    </div>
  );
}
