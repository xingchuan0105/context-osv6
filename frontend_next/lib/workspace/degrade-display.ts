/**
 * Degrade reason codes that are internal plumbing — never show raw strings to end users.
 */
const HIDDEN_DEGRADE_REASONS = new Set([
  "tool_unavailable",
  "tool_degraded",
]);

/** Stable codes safe to surface (still machine ids; filter empties + hidden + dedupe). */
export function userVisibleDegradeReasons(reasons: readonly string[]): string[] {
  const out: string[] = [];
  for (const raw of reasons) {
    const code = String(raw ?? "").trim();
    if (!code || HIDDEN_DEGRADE_REASONS.has(code)) {
      continue;
    }
    if (!out.includes(code)) {
      out.push(code);
    }
  }
  return out;
}

export function isHiddenDegradeReason(reason: string): boolean {
  return HIDDEN_DEGRADE_REASONS.has(String(reason ?? "").trim());
}
