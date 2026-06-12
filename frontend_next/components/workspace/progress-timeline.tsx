"use client";

import { useEffect, useRef } from "react";

import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ProgressEntry } from "../../hooks/use-chat-session";
import styles from "./workspace-chat.module.css";

function isResearchMode(mode: WorkspaceChatMode) {
  return mode === "rag" || mode === "search";
}

function getProgressHeading(locale: "zh-CN" | "en", mode: WorkspaceChatMode) {
  if (locale === "zh-CN") {
    return mode === "rag" ? "知识库检索中" : "网络搜索中";
  }
  return mode === "rag" ? "Knowledge Retrieval" : "Web Search";
}

function getProgressToggleLabel(locale: "zh-CN" | "en", collapsed: boolean) {
  if (locale === "zh-CN") {
    return collapsed ? "展开过程" : "收起过程";
  }
  return collapsed ? "Expand progress" : "Collapse progress";
}

function getCompactStatusTitle(locale: "zh-CN" | "en") {
  return locale === "zh-CN" ? "正在思考" : "Thinking";
}

function getProgressCountLabel(locale: "zh-CN" | "en", key: string) {
  if (locale === "zh-CN") {
    switch (key) {
      case "queries":
        return "查询";
      case "results":
        return "结果";
      case "sources":
        return "来源";
      case "chunks":
        return "片段";
      case "documents":
        return "文档";
      default:
        return key;
    }
  }
  switch (key) {
    case "queries":
      return "queries";
    case "results":
      return "results";
    case "sources":
      return "sources";
    case "chunks":
      return "chunks";
    case "documents":
      return "documents";
    default:
      return key;
  }
}

function getInitialProgressEntry(locale: "zh-CN" | "en", mode: WorkspaceChatMode): ProgressEntry {
  if (locale === "zh-CN") {
    if (mode === "rag") {
      return {
        id: "progress-initial",
        phase: "planning",
        title: "正在分析问题并准备检索知识库",
        detail: "系统正在规划检索范围与证据路径。",
        counts: {},
        sourcesPreview: [],
        timestamp: null,
      };
    }
    return {
      id: "progress-initial",
      phase: "planning",
      title: "正在生成网络搜索计划",
      detail: "系统正在拆解问题并准备搜索网页来源。",
      counts: {},
      sourcesPreview: [],
      timestamp: null,
    };
  }
  if (mode === "rag") {
    return {
      id: "progress-initial",
      phase: "planning",
      title: "Preparing knowledge retrieval",
      detail: "Building a retrieval plan and evidence path.",
      counts: {},
      sourcesPreview: [],
      timestamp: null,
    };
  }
  return {
    id: "progress-initial",
    phase: "planning",
    title: "Preparing a web research plan",
    detail: "Breaking down the request before searching the web.",
    counts: {},
    sourcesPreview: [],
    timestamp: null,
  };
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

