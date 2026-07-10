"use client";

import { useCallback, useRef, useState } from "react";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ChatEvent } from "../../lib/contracts";
import { getInitialProgressEntry } from "./helpers";
import type { ProgressEntry } from "./types";

export function useProgressTracker(locale: "zh-CN" | "en") {
  const [mode, setMode] = useState<WorkspaceChatMode | null>(null);
  const [activities, setActivities] = useState<ProgressEntry[]>([]);
  const [collapsed, setCollapsed] = useState(true);
  const [startedAtMs, setStartedAtMs] = useState<number | null>(null);
  /** When set, stream is done: freeze elapsed and show completed summary (Grok end-state). */
  const [endedAtMs, setEndedAtMs] = useState<number | null>(null);
  const modeRef = useRef<WorkspaceChatMode | null>(null);

  const show = useCallback(
    (m: WorkspaceChatMode) => {
      const now = Date.now();
      modeRef.current = m;
      setMode(m);
      setStartedAtMs(now);
      setEndedAtMs(null);
      // Always seed a thinking step (Grok-style status line for every mode).
      const initial = getInitialProgressEntry(locale, m);
      setActivities([{ ...initial, startedAtMs: now }]);
      // Start collapsed: header shows mode/status · elapsed; expand for step list.
      setCollapsed(true);
    },
    [locale],
  );

  const hide = useCallback(() => {
    modeRef.current = null;
    setMode(null);
    setActivities([]);
    setCollapsed(true);
    setStartedAtMs(null);
    setEndedAtMs(null);
  }, []);

  /** Keep a collapsible process summary after the answer completes; freeze the timer. */
  const finalize = useCallback(() => {
    if (modeRef.current == null) {
      return;
    }
    setEndedAtMs(Date.now());
    setCollapsed(true);
  }, []);

  const addActivity = useCallback((event: Extract<ChatEvent, { event: "activity" }>) => {
    const now = Date.now();
    setActivities((current) => [
      ...current,
      {
        id: `${event.phase}-${current.length}-${event.timestamp ?? now}`,
        phase: event.phase,
        title: event.title,
        detail: event.detail ?? null,
        counts: event.counts,
        sourcesPreview: event.sources_preview.map((source) => ({
          id: source.id,
          label: source.label,
          href: source.href ?? undefined,
        })),
        timestamp: event.timestamp ?? null,
        startedAtMs: now,
      },
    ]);
    // Keep collapsed state: header updates via re-render; user expands for step list.
  }, []);

  const addReasoning = useCallback(
    (content: string) => {
      const now = Date.now();
      setActivities((current) => [
        ...current,
        {
          id: `reasoning-${current.length}-${now}`,
          phase: "reasoning",
          title: locale === "zh-CN" ? "正在整理思路" : "Reasoning",
          detail: content,
          counts: {},
          sourcesPreview: [],
          timestamp: null,
          startedAtMs: now,
        },
      ]);
    },
    [locale],
  );

  const toggleCollapsed = useCallback(() => {
    setCollapsed((c) => !c);
  }, []);

  return {
    progress: { mode, activities, collapsed, startedAtMs, endedAtMs },
    show,
    hide,
    finalize,
    addActivity,
    addReasoning,
    toggleCollapsed,
    modeRef,
  };
}

export type ProgressTracker = ReturnType<typeof useProgressTracker>;
