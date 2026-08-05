"use client";

import { useEffect, useMemo, useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ProgressEntry } from "../../hooks/use-chat-session";
import styles from "./workspace-chat.module.css";

/**
 * Live process indicator (2026-07-23 v2) + completed collapse (2026-08-05):
 * - Live: rolling window of latest MAX_VISIBLE work facts (newest at bottom).
 * - Completed: auto-collapsed summary row with chevron; expand to review steps.
 */
type ProgressStatusLineProps = {
  activities: ProgressEntry[];
  locale: "zh-CN" | "en";
  mode: WorkspaceChatMode;
  startedAtMs: number | null;
  endedAtMs?: number | null;
  /** When true, step list is hidden (completed default). Live ignores and stays open. */
  collapsed?: boolean;
  onToggleCollapsed?: () => void;
};

const MAX_VISIBLE = 4;

function formatElapsed(totalSeconds: number): string {
  if (totalSeconds < 60) {
    return `${totalSeconds}s`;
  }
  const minutes = Math.floor(totalSeconds / 60);
  return `${minutes}m ${totalSeconds % 60}s`;
}

function completedTitle(locale: "zh-CN" | "en", mode: WorkspaceChatMode): string {
  if (mode === "rag" || mode === "rag+search") {
    return formatUiMessage(locale, "workspaceProgressCompletedRag");
  }
  if (mode === "search") {
    return formatUiMessage(locale, "workspaceProgressCompletedSearch");
  }
  return formatUiMessage(locale, "workspaceProgressCompletedThinking");
}

export function ProgressStatusLine({
  activities,
  locale,
  mode,
  startedAtMs,
  endedAtMs = null,
  collapsed = false,
  onToggleCollapsed,
}: ProgressStatusLineProps) {
  const completed = endedAtMs != null;
  const [nowMs, setNowMs] = useState(() => Date.now());
  // Local fallback when parent does not control collapsed (e.g. message snapshot).
  const [localCollapsed, setLocalCollapsed] = useState(true);
  const controlled = onToggleCollapsed != null;
  const isCollapsed = completed && (controlled ? collapsed : localCollapsed);

  useEffect(() => {
    if (startedAtMs == null || completed) {
      return;
    }
    const id = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [startedAtMs, completed]);

  // New turn / re-complete → start collapsed again.
  useEffect(() => {
    if (completed) {
      setLocalCollapsed(true);
    }
  }, [completed, startedAtMs, endedAtMs]);

  const elapsedSeconds = useMemo(() => {
    if (startedAtMs == null) {
      return 0;
    }
    const end = completed && endedAtMs != null ? endedAtMs : nowMs;
    return Math.max(0, Math.floor((end - startedAtMs) / 1000));
  }, [nowMs, startedAtMs, endedAtMs, completed]);

  // Dedupe consecutive identical title+detail (host may re-emit the same step).
  const deduped: ProgressEntry[] = [];
  for (const a of activities) {
    const prev = deduped[deduped.length - 1];
    if (prev && prev.title === a.title && (prev.detail ?? "") === (a.detail ?? "")) {
      continue;
    }
    deduped.push(a);
  }
  const windowed = deduped.slice(-MAX_VISIBLE);
  const latestIndex = windowed.length - 1;
  const fallbackTitle = locale === "zh-CN" ? "正在理解问题" : "Understanding the question";
  const rows: Array<{ id: string; title: string; detail: string | null; age: number }> =
    windowed.length > 0
      ? windowed.map((activity, index) => ({
          id: activity.id,
          title: activity.title,
          // Hide English monologue dumps in the collapsed summary path; still
          // show short Chinese details while live.
          detail:
            activity.phase === "reasoning" && completed
              ? null
              : activity.detail,
          age: latestIndex - index,
        }))
      : [{ id: "pending", title: fallbackTitle, detail: null, age: 0 }];

  const showSteps = !completed || !isCollapsed;
  const canToggle = completed && (activities.length > 0 || rows.length > 0);
  const summaryLabel = completedTitle(locale, mode);
  const chevron = isCollapsed ? "▸" : "▾";

  function handleToggle() {
    if (!canToggle) {
      return;
    }
    if (controlled) {
      onToggleCollapsed?.();
    } else {
      setLocalCollapsed((c) => !c);
    }
  }

  return (
    <section
      aria-live={completed ? "off" : "polite"}
      className={[
        styles.statusLine,
        completed ? styles.statusLineCompleted : styles.statusLineLive,
        isCollapsed ? styles.statusLineCollapsed : "",
      ]
        .filter(Boolean)
        .join(" ")}
      data-progress-state={completed ? "completed" : "live"}
      data-collapsed={isCollapsed ? "true" : "false"}
      data-testid="workspace-progress-status-line"
    >
      {startedAtMs != null ? (
        <span className={styles.statusLineElapsed} data-testid="workspace-progress-elapsed">
          {formatElapsed(elapsedSeconds)}
        </span>
      ) : null}
      <span
        aria-hidden="true"
        className={completed ? styles.statusLineDoneIcon : styles.statusLineSpinner}
      />
      <div className={styles.statusLineMain}>
        {completed ? (
          <button
            aria-expanded={!isCollapsed}
            aria-label={formatUiMessage(
              locale,
              isCollapsed ? "workspaceProgressToggleExpand" : "workspaceProgressToggleCollapse",
            )}
            className={styles.statusLineToggle}
            data-testid="workspace-progress-collapse-toggle"
            disabled={!canToggle}
            onClick={handleToggle}
            type="button"
          >
            <strong className={styles.statusLineSummaryTitle}>{summaryLabel}</strong>
            <span className={styles.statusLineChevron} aria-hidden="true">
              {chevron}
            </span>
          </button>
        ) : null}

        {showSteps ? (
          <div className={styles.statusLineSteps}>
            {rows.map((row) => (
              <div
                className={[
                  styles.statusLineStep,
                  row.age === 0 && !completed
                    ? styles.statusLineStepCurrent
                    : styles.statusLineStepAged,
                ]
                  .filter(Boolean)
                  .join(" ")}
                data-age={row.age}
                key={row.id}
              >
                <strong className={styles.statusLineTitle}>{row.title}</strong>
                {row.detail ? (
                  <span className={styles.statusLineDetail}>{row.detail}</span>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
      </div>
    </section>
  );
}
