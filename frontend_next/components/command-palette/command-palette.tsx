"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { listWorkspaces, type DashboardWorkspace } from "../../lib/dashboard/client";
import { formatUiMessage, type UiMessageKey } from "../../lib/i18n/messages";
import {
  searchProductIndex,
  type GlobalSearchResponse,
} from "../../lib/search/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./command-palette.module.css";

type PaletteGroup = "sessions" | "workspaces" | "sources" | "nav" | "billing" | "help";

type PaletteItem = {
  id: string;
  group: PaletteGroup;
  label: string;
  href: string;
  keywords: string;
};

type StaticCommand = {
  id: string;
  group: Exclude<PaletteGroup, "sessions" | "workspaces" | "sources">;
  labelKey: UiMessageKey;
  href: string;
  keywords: string;
};

const STATIC_COMMANDS: StaticCommand[] = [
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

const GROUP_ORDER: PaletteGroup[] = [
  "sessions",
  "workspaces",
  "sources",
  "nav",
  "billing",
  "help",
];

const GROUP_LABEL: Record<PaletteGroup, UiMessageKey> = {
  sessions: "commandPalette.group.sessions",
  workspaces: "commandPalette.group.workspaces",
  sources: "commandPalette.group.sources",
  nav: "commandPalette.group.nav",
  billing: "commandPalette.group.billing",
  help: "commandPalette.group.help",
};

const RECENT_KEY = "context-os.command-palette.recent-workspaces.v1";
const RECENT_LIMIT = 8;
const WORKSPACE_LIST_CAP = 40;
const SEARCH_MIN_CHARS = 2;
const SEARCH_DEBOUNCE_MS = 220;

function readRecentIds(): string[] {
  if (typeof window === "undefined") {
    return [];
  }
  try {
    const raw = window.localStorage.getItem(RECENT_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter((id): id is string => typeof id === "string" && id.length > 0);
  } catch {
    return [];
  }
}

function pushRecentId(workspaceId: string) {
  if (typeof window === "undefined" || !workspaceId) {
    return;
  }
  const next = [workspaceId, ...readRecentIds().filter((id) => id !== workspaceId)].slice(
    0,
    RECENT_LIMIT,
  );
  try {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // ignore
  }
}

function matches(item: PaletteItem, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) {
    return true;
  }
  return (
    item.label.toLowerCase().includes(q) ||
    item.keywords.toLowerCase().includes(q) ||
    item.href.toLowerCase().includes(q)
  );
}

function workspaceTitle(ws: DashboardWorkspace): string {
  return (ws.title || ws.name || ws.workspace_id).trim();
}

/**
 * PRODUCT_IA: Cmd/Ctrl+K — Canonical routes, recent workspaces, global search (sessions/sources).
 */
export function CommandPaletteHost() {
  const router = useRouter();
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const [workspaces, setWorkspaces] = useState<DashboardWorkspace[]>([]);
  const [searchHits, setSearchHits] = useState<GlobalSearchResponse | null>(null);
  const [searchLoading, setSearchLoading] = useState(false);
  const [recentIds, setRecentIds] = useState<string[]>([]);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const staticItems: PaletteItem[] = useMemo(
    () =>
      STATIC_COMMANDS.map((cmd) => ({
        id: cmd.id,
        group: cmd.group,
        label: formatUiMessage(locale, cmd.labelKey),
        href: cmd.href,
        keywords: cmd.keywords,
      })),
    [locale],
  );

  const workspaceItems: PaletteItem[] = useMemo(() => {
    const byId = new Map(workspaces.map((ws) => [ws.workspace_id, ws]));
    const ordered: DashboardWorkspace[] = [];
    for (const id of recentIds) {
      const hit = byId.get(id);
      if (hit) {
        ordered.push(hit);
        byId.delete(id);
      }
    }
    for (const ws of workspaces) {
      if (byId.has(ws.workspace_id)) {
        ordered.push(ws);
      }
    }

    // When searching via API, prefer API workspace hits merged with local list titles.
    if (searchHits?.workspaces?.length) {
      const fromSearch = searchHits.workspaces.map((ws) => {
        const id = ws.id;
        const title = (ws.title || ws.name || id).trim();
        return {
          id: `ws-${id}`,
          group: "workspaces" as const,
          label: title,
          href: `/dashboard/${id}`,
          keywords: `${title} ${id} workspace 工作区`,
        };
      });
      return fromSearch.slice(0, WORKSPACE_LIST_CAP);
    }

    return ordered.slice(0, WORKSPACE_LIST_CAP).map((ws) => {
      const title = workspaceTitle(ws);
      const recent = recentIds.includes(ws.workspace_id);
      return {
        id: `ws-${ws.workspace_id}`,
        group: "workspaces" as const,
        label: recent
          ? formatUiMessage(locale, "commandPalette.workspaceRecent", { title })
          : title,
        href: `/dashboard/${ws.workspace_id}`,
        keywords: `${title} ${ws.workspace_id} workspace 工作区`,
      };
    });
  }, [locale, recentIds, searchHits, workspaces]);

  const sessionItems: PaletteItem[] = useMemo(() => {
    if (!searchHits?.sessions?.length) {
      return [];
    }
    return searchHits.sessions.slice(0, 20).map((session) => {
      const title =
        session.title?.trim() ||
        formatUiMessage(locale, "commandPalette.sessionUntitled");
      return {
        id: `sess-${session.id}`,
        group: "sessions" as const,
        label: formatUiMessage(locale, "commandPalette.sessionLabel", { title }),
        href: `/dashboard/${session.workspace_id}?session=${encodeURIComponent(session.id)}`,
        keywords: `${title} ${session.id} ${session.workspace_id} session 会话`,
      };
    });
  }, [locale, searchHits]);

  const sourceItems: PaletteItem[] = useMemo(() => {
    if (!searchHits?.sources?.length) {
      return [];
    }
    return searchHits.sources.slice(0, 15).map((source) => {
      const name = (source.file_name || source.title || source.id).trim();
      const wsName = source.workspace_name?.trim();
      return {
        id: `src-${source.id}`,
        group: "sources" as const,
        label: wsName
          ? formatUiMessage(locale, "commandPalette.sourceLabelWithWs", {
              name,
              workspace: wsName,
            })
          : formatUiMessage(locale, "commandPalette.sourceLabel", { name }),
        href: `/dashboard/${source.workspace_id}`,
        keywords: `${name} ${source.id} ${source.workspace_id} source document 文档 来源`,
      };
    });
  }, [locale, searchHits]);

  const filtered = useMemo(() => {
    const q = query.trim();
    // Empty query: recent workspaces + static nav.
    if (!q) {
      const recentOnly = workspaceItems.filter((item) =>
        recentIds.some((id) => item.id === `ws-${id}`),
      );
      return [...recentOnly, ...staticItems];
    }
    // API search hits are authoritative (FTS may match message body, not title) —
    // do not re-filter sessions/sources/workspaces from searchHits client-side.
    if (searchHits) {
      return [
        ...sessionItems,
        ...workspaceItems,
        ...sourceItems,
        ...staticItems.filter((item) => matches(item, q)),
      ];
    }
    // Debouncing / no hits yet: local workspace list + static, title/keyword match.
    return [...workspaceItems, ...staticItems].filter((item) => matches(item, q));
  }, [
    query,
    recentIds,
    searchHits,
    sessionItems,
    sourceItems,
    staticItems,
    workspaceItems,
  ]);

  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
    setActiveIndex(0);
    setSearchHits(null);
  }, []);

  const run = useCallback(
    (item: PaletteItem) => {
      if (item.id.startsWith("ws-")) {
        pushRecentId(item.id.slice(3));
        setRecentIds(readRecentIds());
      } else if (item.id.startsWith("sess-")) {
        const match = item.href.match(/\/dashboard\/([^?]+)/);
        if (match?.[1]) {
          pushRecentId(match[1]);
          setRecentIds(readRecentIds());
        }
      }
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
      }
    }

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    setRecentIds(readRecentIds());
    const id = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  useEffect(() => {
    if (!open || !auth.token) {
      return;
    }
    let cancelled = false;
    void listWorkspaces(auth.token)
      .then((response) => {
        if (!cancelled) {
          setWorkspaces(response.workspaces);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setWorkspaces([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [auth.token, open]);

  // Debounced global search for sessions / sources / workspaces.
  useEffect(() => {
    if (!open || !auth.token) {
      return;
    }
    const q = query.trim();
    if (q.length < SEARCH_MIN_CHARS) {
      setSearchHits(null);
      setSearchLoading(false);
      return;
    }
    let cancelled = false;
    setSearchLoading(true);
    const timer = window.setTimeout(() => {
      void searchProductIndex(auth.token as string, q)
        .then((hits) => {
          if (!cancelled) {
            setSearchHits(hits);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setSearchHits(null);
          }
        })
        .finally(() => {
          if (!cancelled) {
            setSearchLoading(false);
          }
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [auth.token, open, query]);

  useEffect(() => {
    setActiveIndex(0);
  }, [query, open, filtered.length]);

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
  const showSearching =
    searchLoading && query.trim().length >= SEARCH_MIN_CHARS && filtered.length === 0;

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
          {showSearching ? (
            <p className={styles.empty}>
              {formatUiMessage(locale, "commandPalette.loadingSearch")}
            </p>
          ) : filtered.length === 0 ? (
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
                        <span>{item.label}</span>
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
