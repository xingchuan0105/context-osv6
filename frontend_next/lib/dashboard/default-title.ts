export type DashboardLocale = "zh-CN" | "en";

type WorkspaceCreateCounters = {
  counts: Record<string, number>;
};

const WORKSPACE_CREATE_COUNTERS_STORAGE_KEY = "avrag.workspace-create-counters.v1";
const WORKSPACE_CREATE_COUNTERS_KEY = new Map<string, number>();

function workspaceLocaleKey(locale: DashboardLocale) {
  return locale;
}

function workspaceCreateCounterKey(locale: DashboardLocale) {
  return workspaceLocaleKey(locale);
}

/**
 * Default title for newly created workspaces (W7 / ADR-0010 #20).
 *
 * zh: 「新建工作区N」 · en: 「New Workspace N」
 *
 * Bare「1」investigation (2026-08-06): product create paths only call this helper
 * (`dashboard-surface` + `use-workspace-data.createWorkspaceFlow`). Neither path
 * ever emitted a digit-only title. Screenshot bare「1」is therefore not from the
 * current generator — likely manual rename, stale DB seed, or a non-web client.
 * Backend stores the client-supplied name as-is (no second renamer).
 */
export function formatDefaultWorkspaceTitle(
  locale: DashboardLocale,
  _date: string,
  duplicateIndex = 0,
) {
  const n = duplicateIndex + 1;
  if (locale === "zh-CN") {
    return `新建工作区${n}`;
  }
  return `New Workspace ${n}`;
}

function readWorkspaceCreateCounters(): WorkspaceCreateCounters {
  if (typeof window === "undefined") {
    return { counts: Object.fromEntries(WORKSPACE_CREATE_COUNTERS_KEY.entries()) };
  }

  const raw = window.localStorage.getItem(WORKSPACE_CREATE_COUNTERS_STORAGE_KEY);
  if (!raw) {
    return { counts: Object.fromEntries(WORKSPACE_CREATE_COUNTERS_KEY.entries()) };
  }

  try {
    const parsed = JSON.parse(raw) as WorkspaceCreateCounters;
    if (!parsed || typeof parsed !== "object" || !parsed.counts) {
      return { counts: Object.fromEntries(WORKSPACE_CREATE_COUNTERS_KEY.entries()) };
    }

    return parsed;
  } catch {
    return { counts: Object.fromEntries(WORKSPACE_CREATE_COUNTERS_KEY.entries()) };
  }
}

function writeWorkspaceCreateCounters(counters: WorkspaceCreateCounters) {
  WORKSPACE_CREATE_COUNTERS_KEY.clear();
  for (const [key, value] of Object.entries(counters.counts)) {
    WORKSPACE_CREATE_COUNTERS_KEY.set(key, value);
  }

  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(
    WORKSPACE_CREATE_COUNTERS_STORAGE_KEY,
    JSON.stringify(counters),
  );
}

export function getDefaultWorkspaceTitle(locale: DashboardLocale, date: string) {
  const key = workspaceCreateCounterKey(locale);
  const count = readWorkspaceCreateCounters().counts[key] ?? 0;

  return formatDefaultWorkspaceTitle(locale, date, count);
}

export function markDefaultWorkspaceTitleUsed(locale: DashboardLocale, _date: string) {
  const key = workspaceCreateCounterKey(locale);
  const counters = readWorkspaceCreateCounters();
  const count = counters.counts[key] ?? 0;
  counters.counts[key] = count + 1;
  writeWorkspaceCreateCounters(counters);
}

export function resetDefaultWorkspaceTitleCounters() {
  WORKSPACE_CREATE_COUNTERS_KEY.clear();

  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.removeItem(WORKSPACE_CREATE_COUNTERS_STORAGE_KEY);
}
