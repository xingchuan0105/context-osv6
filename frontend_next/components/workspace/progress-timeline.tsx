"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

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

function activityHasSectionBody(activity: ProgressEntry): boolean {
  return Boolean(
    activity.detail ||
      Object.keys(activity.counts).length > 0 ||
      activity.sourcesPreview.length > 0,
  );
}

type ProgressTimelineProps = {
  activities: ProgressEntry[];
  /** Level-1: entire card body (step list). Default expanded from tracker. */
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
  /** Level-2: which step sections are expanded (detail/meta). Default: none. */
  const [expandedSections, setExpandedSections] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    if (startedAtMs == null || completed) {
      return;
    }
    const id = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [startedAtMs, completed]);

  // New turn resets section expand state (including 思考摘要 — default collapsed).
  useEffect(() => {
    setExpandedSections(new Set());
  }, [startedAtMs]);

  const elapsedSeconds = useMemo(() => {
    if (startedAtMs == null) {
      return 0;
    }
    const end = completed && endedAtMs != null ? endedAtMs : nowMs;
    return Math.max(0, Math.floor((end - startedAtMs) / 1000));
  }, [nowMs, startedAtMs, endedAtMs, completed]);

  const headerTitle = getHeaderTitle(locale, mode, activities, completed);

  const cardChevron = collapsed ? "▸" : "▾";
  const canExpandCard = activities.length > 0;

  const toggleSection = useCallback((activityId: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev);
      if (next.has(activityId)) {
        next.delete(activityId);
      } else {
        next.add(activityId);
      }
      return next;
    });
  }, []);

  useEffect(() => {
    if (collapsed) {
      return;
    }
    const body = bodyRef.current;
    if (!body) {
      return;
    }
    body.scrollTop = body.scrollHeight;
  }, [activities, collapsed, expandedSections]);

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
      data-card-collapsed={collapsed ? "true" : "false"}
    >
      <div className={styles.progressHeader}>
        <span
          aria-hidden="true"
          className={[
            styles.progressIcon,
            styles.progressMatrix,
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
          {/* Grok-style 3×3 square matrix (cycles while live). */}
          {Array.from({ length: 9 }, (_, i) => (
            <span className={styles.progressMatrixCell} key={i} style={{ ["--cell-i" as string]: i }} />
          ))}
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
              data-testid="workspace-progress-card-toggle"
              disabled={!canExpandCard}
              onClick={onToggleCollapsed}
              type="button"
            >
              <span className={styles.progressDisclosureTitle}>{headerTitle}</span>
              {startedAtMs != null ? (
                <span className={styles.progressElapsed} data-testid="workspace-progress-elapsed">
                  · {formatElapsedSeconds(elapsedSeconds)}
                </span>
              ) : null}
              {canExpandCard ? (
                <span className={styles.progressDisclosureChevron} aria-hidden="true">
                  {cardChevron}
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
            const hasBody = activityHasSectionBody(activity);
            const sectionOpen = hasBody && expandedSections.has(activity.id);
            return (
              <div
                className={[styles.progressItem, isLatest ? styles.progressItemLatest : ""]
                  .filter(Boolean)
                  .join(" ")}
                data-section-expanded={sectionOpen ? "true" : "false"}
                data-testid="workspace-progress-step"
                key={activity.id}
              >
                <span aria-hidden="true" className={styles.progressItemDot} />
                <div className={styles.progressItemContent}>
                  {hasBody ? (
                    <button
                      aria-expanded={sectionOpen}
                      aria-label={formatUiMessage(
                        locale,
                        sectionOpen
                          ? "workspaceProgressStepCollapse"
                          : "workspaceProgressStepExpand",
                        { title: activity.title },
                      )}
                      className={styles.progressStepDisclosure}
                      data-testid="workspace-progress-step-toggle"
                      onClick={() => toggleSection(activity.id)}
                      type="button"
                    >
                      <span className={styles.progressItemHeader}>
                        <strong>{activity.title}</strong>
                        <span className={styles.progressStepChevron} aria-hidden="true">
                          {sectionOpen ? "▾" : "▸"}
                        </span>
                      </span>
                    </button>
                  ) : (
                    <div className={styles.progressItemHeader}>
                      <strong>{activity.title}</strong>
                    </div>
                  )}

                  {sectionOpen ? (
                    <div className={styles.progressStepBody} data-testid="workspace-progress-step-body">
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
                              <span
                                className={styles.progressSourcePill}
                                key={`${activity.id}-${source.id}`}
                              >
                                {source.label}
                              </span>
                            ),
                          )}
                        </div>
                      ) : null}
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
