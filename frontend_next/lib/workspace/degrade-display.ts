/**
 * Degrade reason codes that are internal plumbing — never show raw strings to end users.
 */
const HIDDEN_DEGRADE_REASONS = new Set([
  "tool_unavailable",
  "tool_degraded",
]);

/** User-facing labels for stable degrade codes (zh / en). */
const DEGRADE_REASON_LABELS: Record<string, { zh: string; en: string }> = {
  fallback_to_summary: {
    zh: "改为摘要回答",
    en: "Fell back to a summary answer",
  },
  no_retrieval_evidence: {
    zh: "检索未找到足够依据",
    en: "Not enough retrieval evidence",
  },
  partial_evidence: {
    zh: "仅有部分检索依据",
    en: "Only partial retrieval evidence",
  },
  budget_exhausted: {
    zh: "本轮预算已用尽",
    en: "Turn budget exhausted",
  },
};

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

/**
 * Map machine degrade codes to short product language (PRODUCT_IA / copy-catalog).
 * Unknown codes are omitted rather than shown raw.
 */
export function labelDegradeReasons(
  reasons: readonly string[],
  locale: string,
): string[] {
  const isZh = locale === "zh-CN" || locale.startsWith("zh");
  const labels: string[] = [];
  for (const code of userVisibleDegradeReasons(reasons)) {
    const entry = DEGRADE_REASON_LABELS[code];
    if (!entry) {
      continue;
    }
    labels.push(isZh ? entry.zh : entry.en);
  }
  return labels;
}
