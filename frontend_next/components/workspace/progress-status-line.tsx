"use client";

import { useEffect, useMemo, useState } from "react";

import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ProgressEntry } from "../../hooks/use-chat-session";
import styles from "./workspace-chat.module.css";

/**
 * Live process indicator (2026-07-23 v2):
 * - Elapsed timer pinned at a FIXED slot on the right (tabular-nums, never
 *   re-animates with step swaps).
 * - Rolling window of the latest MAX_VISIBLE work facts (newest at the
 *   bottom, dimmed by age); each new fact slides in, oldest drops out.
 * - No level-1 card, no placeholder step. All modes (chat / rag / search /
 *   rag+search), live and completed states share the component.
 */
type ProgressStatusLineProps = {
  activities: ProgressEntry[];
  locale: "zh-CN" | "en";
  mode: WorkspaceChatMode;
  startedAtMs: number | null;
  endedAtMs?: number | null;
};

const MAX_VISIBLE = 4;

function formatElapsed(totalSeconds: number): string {
  if (totalSeconds < 60) {
    return `${totalSeconds}s`;
  }
  const minutes = Math.floor(totalSeconds / 60);
  return `${minutes}m ${totalSeconds % 60}s`;
}

export function ProgressStatusLine({
  activities,
  locale,
  mode: _mode,
  startedAtMs,
  endedAtMs = null,
}: ProgressStatusLineProps) {
  const completed = endedAtMs != null;
  const [nowMs, setNowMs] = useState(() => Date.now());

  useEffect(() => {
    if (startedAtMs == null || completed) {
      return;
    }
    const id = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [startedAtMs, completed]);

  const elapsedSeconds = useMemo(() => {
    if (startedAtMs == null) {
      return 0;
    }
    const end = completed && endedAtMs != null ? endedAtMs : nowMs;
    return Math.max(0, Math.floor((end - startedAtMs) / 1000));
  }, [nowMs, startedAtMs, endedAtMs, completed]);

  const windowed = activities.slice(-MAX_VISIBLE);
  const latestIndex = windowed.length - 1;
  // Before the first fact lands (sub-second), keep one pending row so the
  // indicator is never empty.
  const fallbackTitle = locale === "zh-CN" ? "正在理解问题" : "Understanding the question";
  const rows: Array<{ id: string; title: string; detail: string | null; age: number }> =
    windowed.length > 0
      ? windowed.map((activity, index) => ({
          id: activity.id,
          title: activity.title,
          detail: activity.detail,
          age: latestIndex - index,
        }))
      : [{ id: "pending", title: fallbackTitle, detail: null, age: 0 }];

  return (
    <section
      aria-live="polite"
      className={[
        styles.statusLine,
        completed ? styles.statusLineCompleted : styles.statusLineLive,
      ]
        .filter(Boolean)
        .join(" ")}
      data-progress-state={completed ? "completed" : "live"}
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
      <div className={styles.statusLineSteps}>
        {rows.map((row) => (
          <div
            className={[
              styles.statusLineStep,
              row.age === 0 ? styles.statusLineStepCurrent : styles.statusLineStepAged,
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
    </section>
  );
}
