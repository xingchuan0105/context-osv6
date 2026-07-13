import type { ChatEvent } from "../../lib/contracts";
import { formatUiMessage, type UiMessageKey, UI_MESSAGES } from "../../lib/i18n/messages";
import type { ProgressEntry, UiProgressSnapshot } from "./types";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";

type ActivityEvent = Extract<ChatEvent, { event: "activity" }>;

function isProgressMessageKey(key: string): key is UiMessageKey {
  return Object.prototype.hasOwnProperty.call(UI_MESSAGES, key);
}

/**
 * Localize backend WorkFact activity titles.
 * Backend sends stable keys (`progress.*`) + raw query in detail / hits in counts.
 * Fallback: show server title as-is for legacy non-key messages.
 */
export function localizeProgressActivity(
  locale: "zh-CN" | "en",
  event: ActivityEvent,
): { title: string; detail: string | null } {
  const rawTitle = event.title?.trim() ?? "";
  const rawDetail = event.detail?.trim() || null;
  const hits =
    typeof event.counts?.hits === "number" && event.counts.hits > 0
      ? event.counts.hits
      : null;

  const isKey = rawTitle.startsWith("progress.") && isProgressMessageKey(rawTitle);

  let title = rawTitle;
  if (isKey) {
    if (rawTitle === "progress.write_draft_section" || rawTitle === "progress.write_refine_round") {
      title = formatUiMessage(locale, rawTitle, { section: rawDetail ?? "" });
    } else {
      title = formatUiMessage(locale, rawTitle);
    }
  }

  let detail: string | null = null;
  if (!isKey) {
    // Legacy free-text activities: pass title/detail through unchanged.
    detail = rawDetail;
  } else if (rawTitle === "progress.write_draft_section" || rawTitle === "progress.write_refine_round") {
    // section/round already folded into title
    detail = null;
  } else if (rawDetail && hits != null) {
    detail = formatUiMessage(locale, "progress.detail.queryWithHits", {
      query: rawDetail,
      n: hits,
    });
  } else if (rawDetail && rawTitle.endsWith(".empty")) {
    detail = formatUiMessage(locale, "progress.detail.emptyQuery", { query: rawDetail });
  } else if (rawDetail) {
    detail = formatUiMessage(locale, "progress.detail.query", { query: rawDetail });
  } else if (hits != null) {
    detail = formatUiMessage(locale, "progress.detail.hitsOnly", { n: hits });
  }

  return { title, detail };
}

/** Localize a stored progress entry (backend keeps i18n keys in turn_metadata). */
export function localizeProgressEntry(
  locale: "zh-CN" | "en",
  entry: ProgressEntry,
): ProgressEntry {
  const { title, detail } = localizeProgressActivity(locale, {
    event: "activity",
    request_id: "",
    phase: entry.phase,
    title: entry.title,
    detail: entry.detail,
    counts: entry.counts,
    sources_preview: entry.sourcesPreview.map((s) => ({
      id: s.id,
      label: s.label,
      href: s.href ?? null,
    })),
    timestamp: entry.timestamp,
  });
  return { ...entry, title, detail };
}

/**
 * Parse assistant `turn_metadata.progress` from the messages API into a UI snapshot.
 * Returns null when absent / invalid.
 */
export function progressSnapshotFromTurnMetadata(
  locale: "zh-CN" | "en",
  turnMetadata: unknown,
): UiProgressSnapshot | null {
  if (!turnMetadata || typeof turnMetadata !== "object") {
    return null;
  }
  const root = turnMetadata as Record<string, unknown>;
  const progress = root.progress;
  if (!progress || typeof progress !== "object") {
    return null;
  }
  const p = progress as Record<string, unknown>;
  const modeRaw = typeof p.mode === "string" ? p.mode : null;
  if (modeRaw !== "rag" && modeRaw !== "search" && modeRaw !== "write" && modeRaw !== "chat") {
    return null;
  }
  const mode = modeRaw as WorkspaceChatMode;
  const rawActivities = Array.isArray(p.activities) ? p.activities : [];
  const activities: ProgressEntry[] = rawActivities
    .map((item, index): ProgressEntry | null => {
      if (!item || typeof item !== "object") {
        return null;
      }
      const a = item as Record<string, unknown>;
      const title = typeof a.title === "string" ? a.title : "";
      if (!title) {
        return null;
      }
      const sourcesRaw = Array.isArray(a.sources_preview)
        ? a.sources_preview
        : Array.isArray(a.sourcesPreview)
          ? a.sourcesPreview
          : [];
      const entry: ProgressEntry = {
        id: typeof a.id === "string" ? a.id : `restored-${index}`,
        phase: typeof a.phase === "string" ? a.phase : "act",
        title,
        detail: typeof a.detail === "string" ? a.detail : a.detail == null ? null : String(a.detail),
        counts:
          a.counts && typeof a.counts === "object" && !Array.isArray(a.counts)
            ? (a.counts as Record<string, number>)
            : {},
        sourcesPreview: sourcesRaw
          .filter((s): s is Record<string, unknown> => Boolean(s) && typeof s === "object")
          .map((s) => ({
            id: String(s.id ?? ""),
            label: String(s.label ?? s.id ?? ""),
            href: typeof s.href === "string" ? s.href : undefined,
          })),
        timestamp: typeof a.timestamp === "string" ? a.timestamp : null,
      };
      return localizeProgressEntry(locale, entry);
    })
    .filter((entry): entry is ProgressEntry => entry != null);

  if (activities.length === 0) {
    return null;
  }

  return {
    mode,
    activities,
    startedAtMs: typeof p.startedAtMs === "number" ? p.startedAtMs : null,
    endedAtMs: typeof p.endedAtMs === "number" ? p.endedAtMs : Date.now(),
    collapsed: p.collapsed === true,
  };
}
