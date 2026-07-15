/** Product multiselect capability tags (not exclusive modes). Write is offline. */
export type WorkspaceCapability = "rag" | "search";

/** Derived wire/telemetry label; includes legacy `write` for historical messages only. */
export type WorkspaceAgentTypeLabel = "chat" | "rag" | "search" | "rag+search" | "write";

const CAPABILITY_ORDER: readonly WorkspaceCapability[] = ["rag", "search"];

export function toggleCapability(
  current: readonly WorkspaceCapability[],
  cap: WorkspaceCapability,
): WorkspaceCapability[] {
  return current.includes(cap)
    ? current.filter((c) => c !== cap)
    : normalizeCapabilities([...current, cap]);
}

/** Stable order: rag then search; dedupe; ignore unknown. */
export function normalizeCapabilities(input: unknown): WorkspaceCapability[] {
  if (!Array.isArray(input)) {
    return [];
  }
  const seen = new Set<WorkspaceCapability>();
  for (const item of input) {
    if (item === "rag" || item === "search") {
      seen.add(item);
    }
  }
  return CAPABILITY_ORDER.filter((c) => seen.has(c));
}

/** Map legacy single agent_type / mode to capability list. */
export function capabilitiesFromAgentType(mode: string | null | undefined): WorkspaceCapability[] {
  if (mode === "rag") {
    return ["rag"];
  }
  if (mode === "search") {
    return ["search"];
  }
  if (mode === "rag+search") {
    return ["rag", "search"];
  }
  return [];
}

export function deriveAgentTypeLabel(
  capabilities: readonly WorkspaceCapability[],
): Exclude<WorkspaceAgentTypeLabel, "write"> {
  const hasRag = capabilities.includes("rag");
  const hasSearch = capabilities.includes("search");
  if (hasRag && hasSearch) {
    return "rag+search";
  }
  if (hasRag) {
    return "rag";
  }
  if (hasSearch) {
    return "search";
  }
  return "chat";
}

export type ClientContext = {
  local_time: string;
  timezone: string;
};

/** Local wall clock with numeric offset for `user_context` base tool. */
export function buildClientContext(now: Date = new Date()): ClientContext {
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  const pad = (n: number) => String(n).padStart(2, "0");
  const offsetMin = -now.getTimezoneOffset();
  const sign = offsetMin >= 0 ? "+" : "-";
  const abs = Math.abs(offsetMin);
  const oh = pad(Math.floor(abs / 60));
  const om = pad(abs % 60);
  const local_time = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}T${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}${sign}${oh}:${om}`;
  return { local_time, timezone };
}
