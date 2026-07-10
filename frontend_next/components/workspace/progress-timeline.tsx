"use client";

import { useEffect, useMemo, useRef, useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ProgressEntry } from "../../hooks/use-chat-session";
import styles from "./workspace-chat.module.css";

function isResearchMode(mode: WorkspaceChatMode) {
  return mode === "rag" || mode === "search";
}

function getProgressCountLabel(locale: "zh-CN" | "en", key: string) {
  switch (key) {
    case "queries":
      return formatUiMessage(locale, "workspaceProgressCountQueries");
    case "results":
      return formatUiMessage(locale, "workspaceProgressCountResults");
    case "sources":
      return formatUiMessage(locale, "workspaceProgressCountSources");
    case "chunks":
      return formatUiMessage(locale, "workspaceProgressCountChunks");
    case "documents":
      return formatUiMessage(locale, "workspaceProgressCountDocuments");
    default:
      return key;
  }
}

function formatElapsedSeconds(totalSeconds: number): string {
  if (totalSeconds < 60) {
    return `${totalSeconds}s`;
  }
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}m ${seconds}s`;
}

function getHeaderTitle(
  locale: "zh-CN" | "en",
  mode: WorkspaceChatMode,
  activities: ProgressEntry[],
  completed: boolean,
) {
  const researchMode = isResearchMode(mode);
  if (completed) {
    if (mode === "rag") {
      return formatUiMessage(locale, "workspaceProgressCompletedRag");
    }
    if (mode === "search") {
      return formatUiMessage(locale, "workspaceProgressCompletedSearch");
    }
    return (
      activities[activities.length - 1]?.title ??
      formatUiMessage(locale, "workspaceProgressCompletedThinking")
    );
  }
  if (researchMode) {
    return mode === "rag"
      ? formatUiMessage(locale, "workspaceProgressHeadingRag")
      : formatUiMessage(locale, "workspaceProgressHeadingSearch");
  }
  return (
    activities[activities.length - 1]?.title ??
    formatUiMessage(locale, "workspaceProgressThinking")
  );
}

type ProgressTimelineProps = {
  activities: ProgressEntry[];
  collapsed: boolean;
  locale: "zh-CN" | "en";
  mode: WorkspaceChatMode;
  startedAtMs: number | null;
  /** When set, process is finalized: freeze elapsed, no pulse, completed copy. */
  endedAtMs?: number | null;
  onToggleCollapsed: () => void;
};

export function ProgressTimeline({
  activities,
  collapsed,
  locale,
  mode,
  startedAtMs,
  endedAtMs = null,
  onToggleCollapsed,
}: ProgressTimelineProps) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const researchMode = isResearchMode(mode);
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

  const headerTitle = getHeaderTitle(locale, mode, activities, completed);

  const disclosureChevron = collapsed ? "▸" : "▾";
  const canExpand = activities.length > 0;

  useEffect(() => {
    if (collapsed) {
      return;
    }
    const body = bodyRef.current;
    if (!body) {
      return;
    }
    body.scrollTop = body.scrollHeight;
  }, [activities, collapsed]);

  return (
    <section
      aria-live={completed ? "off" : "polite"}
      className={[
        styles.progressCard,
        researchMode ? styles.progressCardResearch : styles.progressCardCompact,
        completed ? styles.progressCardCompleted : styles.progressCardLive,
      ]
        .filter(Boolean)
        .join(" ")}
      data-testid={researchMode ? "workspace-progress-card" : "workspace-status-hint"}
      data-progress-state={completed ? "completed" : "live"}
    >
      <div className={styles.progressHeader}>
        <span
          aria-hidden="true"
          className={[
            styles.progressIcon,
            completed ? styles.progressIconDone : styles.progressIconPulse,
            mode === "rag"
              ? styles.progressIconRag
              : mode === "search"
                ? styles.progressIconSearch
                : styles.progressIconGeneral,
          ]
            .filter(Boolean)
            .join(" ")}
        >
          <span className={styles.progressIconCore} />
        </span>

        <div className={styles.progressHeaderMain}>
          <div className={styles.progressHeaderTitleRow}>
            <button
              aria-expanded={!collapsed}
              aria-label={formatUiMessage(
                locale,
                collapsed ? "workspaceProgressToggleExpand" : "workspaceProgressToggleCollapse",
              )}
              className={styles.progressDisclosure}
              disabled={!canExpand}
              onClick={onToggleCollapsed}
              type="button"
            >
              <span className={styles.progressDisclosureTitle}>{headerTitle}</span>
              {startedAtMs != null ? (
                <span className={styles.progressElapsed} data-testid="workspace-progress-elapsed">
                  · {formatElapsedSeconds(elapsedSeconds)}
                </span>
              ) : null}
              {canExpand ? (
                <span className={styles.progressDisclosureChevron} aria-hidden="true">
                  {disclosureChevron}
                </span>
              ) : null}
            </button>
          </div>
        </div>
      </div>

      {!collapsed && activities.length > 0 ? (
        <div className={styles.progressBody} ref={bodyRef}>
          {activities.map((activity, index) => {
            const isLatest = !completed && index === activities.length - 1;
            return (
              <div
                className={[styles.progressItem, isLatest ? styles.progressItemLatest : ""]
                  .filter(Boolean)
                  .join(" ")}
                key={activity.id}
              >
                <span aria-hidden="true" className={styles.progressItemDot} />
                <div className={styles.progressItemContent}>
                  <div className={styles.progressItemHeader}>
                    <strong>{activity.title}</strong>
                  </div>
                  {activity.detail ? (
                    <p className={styles.progressItemDetail}>{activity.detail}</p>
                  ) : null}
                  {Object.keys(activity.counts).length > 0 ? (
                    <div className={styles.progressMetaRow}>
                      {Object.entries(activity.counts).map(([key, value]) => (
                        <span className={styles.progressMetaPill} key={`${activity.id}-${key}`}>
                          {getProgressCountLabel(locale, key)} {value}
                        </span>
                      ))}
                    </div>
                  ) : null}
                  {activity.sourcesPreview.length > 0 ? (
                    <div className={styles.progressMetaRow}>
                      {activity.sourcesPreview.map((source) =>
                        source.href ? (
                          <a
                            className={styles.progressSourcePill}
                            href={source.href}
                            key={`${activity.id}-${source.id}`}
                            rel="noopener noreferrer"
                            target="_blank"
                          >
                            {source.label}
                          </a>
                        ) : (
                          <span className={styles.progressSourcePill} key={`${activity.id}-${source.id}`}>
                            {source.label}
                          </span>
                        ),
                      )}
                    </div>
                  ) : null}
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
    </section>
  );
}
