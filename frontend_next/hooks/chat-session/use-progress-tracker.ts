"use client";

import { useCallback, useRef, useState } from "react";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { WorkspaceChatStreamEvent } from "../../lib/workspace/stream";
import { getInitialProgressEntry, isResearchMode } from "./helpers";
import type { ProgressEntry } from "./types";

export function useProgressTracker(locale: "zh-CN" | "en") {
  const [mode, setMode] = useState<WorkspaceChatMode | null>(null);
  const [activities, setActivities] = useState<ProgressEntry[]>([]);
  const [collapsed, setCollapsed] = useState(true);
  const modeRef = useRef<WorkspaceChatMode | null>(null);

  const show = useCallback(
    (m: WorkspaceChatMode) => {
      modeRef.current = m;
      setMode(m);
      setActivities(isResearchMode(m) ? [getInitialProgressEntry(locale, m)] : []);
      setCollapsed(true);
    },
    [locale],
  );

  const hide = useCallback(() => {
    modeRef.current = null;
    setMode(null);
    setActivities([]);
    setCollapsed(true);
  }, []);

  const addActivity = useCallback(
    (event: Extract<WorkspaceChatStreamEvent, { kind: "activity" }>) => {
      setActivities((current) => [
        ...current,
        {
          id: `${event.phase}-${current.length}-${event.timestamp ?? Date.now()}`,
          phase: event.phase,
          title: event.title,
          detail: event.detail ?? null,
          counts: event.counts,
          sourcesPreview: event.sources_preview,
          timestamp: event.timestamp ?? null,
        },
      ]);
    },
    [],
  );

  const addReasoning = useCallback(
    (content: string) => {
      setActivities((current) => [
        ...current,
        {
          id: `reasoning-${current.length}-${Date.now()}`,
          phase: "reasoning",
          title: locale === "zh-CN" ? "正在整理思路" : "Reasoning summary",
          detail: content,
          counts: {},
          sourcesPreview: [],
          timestamp: null,
        },
      ]);
    },
    [locale],
  );

  const toggleCollapsed = useCallback(() => {
    setCollapsed((c) => !c);
  }, []);

  return {
    progress: { mode, activities, collapsed },
    show,
    hide,
    addActivity,
    addReasoning,
    toggleCollapsed,
    modeRef,
  };
}

export type ProgressTracker = ReturnType<typeof useProgressTracker>;
