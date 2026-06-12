"use client";

import { useEffect, useRef } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ProgressEntry } from "../../hooks/use-chat-session";
import styles from "./workspace-chat.module.css";

function isResearchMode(mode: WorkspaceChatMode) {
  return mode === "rag" || mode === "search";
}

function getProgressHeading(locale: "zh-CN" | "en", mode: WorkspaceChatMode) {
  if (mode === "rag") {
    return formatUiMessage(locale, "workspaceProgressHeadingRag");
  }
  return formatUiMessage(locale, "workspaceProgressHeadingSearch");
}

function getProgressToggleLabel(locale: "zh-CN" | "en", collapsed: boolean) {
  return formatUiMessage(
    locale,
    collapsed ? "workspaceProgressToggleExpand" : "workspaceProgressToggleCollapse",
  );
}

function getCompactStatusTitle(locale: "zh-CN" | "en") {
  return formatUiMessage(locale, "workspaceProgressThinking");
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

type ProgressTimelineProps = {
  activities: ProgressEntry[];
  collapsed: boolean;
  locale: "zh-CN" | "en";
  mode: WorkspaceChatMode;
  onToggleCollapsed: () => void;
};

export function ProgressTimeline({
  activities,
  collapsed,
  locale,
  mode,
  onToggleCollapsed,
}: ProgressTimelineProps) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const researchMode = isResearchMode(mode);
  const headerTitle = researchMode ? getProgressHeading(locale, mode) : getCompactStatusTitle(locale);
  const disclosureChevron = collapsed ? "▸" : "▾";

  useEffect(() => {
    if (!researchMode || collapsed) {
      return;
    }
    const body = bodyRef.current;
    if (!body) {
      return;
    }
    body.scrollTop = body.scrollHeight;
  }, [activities, collapsed, researchMode]);

  return (
    <section
      className={[
        styles.progressCard,
        researchMode ? styles.progressCardResearch : styles.progressCardCompact,
      ]
        .filter(Boolean)
        .join(" ")}
      data-testid={researchMode ? "workspace-progress-card" : "workspace-status-hint"}
    >
      <div className={styles.progressHeader}>
        <span
          aria-hidden="true"
          className={[
            styles.progressIcon,
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
              aria-label={getProgressToggleLabel(locale, collapsed)}
              className={styles.progressDisclosure}
              onClick={onToggleCollapsed}
              type="button"
            >
              <span>{headerTitle}</span>
              <span className={styles.progressDisclosureChevron} aria-hidden="true">
                {disclosureChevron}
              </span>
            </button>
          </div>
        </div>
      </div>

      {!collapsed && activities.length > 0 ? (
        <div className={styles.progressBody} ref={bodyRef}>
          {activities.map((activity) => (
            <div className={styles.progressItem} key={activity.id}>
              <span aria-hidden="true" className={styles.progressItemDot} />
              <div className={styles.progressItemContent}>
                <div className={styles.progressItemHeader}>
                  <strong>{activity.title}</strong>
                  {activity.timestamp ? <span>{activity.timestamp}</span> : null}
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
                    {activity.sourcesPreview.map((source) => (
                      <span className={styles.progressSourcePill} key={`${activity.id}-${source.id}`}>
                        {source.label}
                      </span>
                    ))}
                  </div>
                ) : null}
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}
