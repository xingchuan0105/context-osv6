"use client";

import { useCallback, useRef, useState } from "react";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { ChatEvent } from "../../lib/contracts";
import { formatUiMessage } from "../../lib/i18n/messages";
import { getInitialProgressEntry } from "./helpers";
import { localizeProgressActivity } from "./progress-i18n";
import type { ProgressEntry, UiProgressSnapshot } from "./types";

/** Product cap for 思考摘要 (mirrors backend REASONING_SUMMARY_MAX_CHARS). */
export const REASONING_SUMMARY_MAX_CHARS = 160;

function appendCappedReasoning(existing: string | null | undefined, chunk: string): string {
  const base = existing ?? "";
  if (base.endsWith("…") && base.length >= REASONING_SUMMARY_MAX_CHARS) {
    return base;
  }
  const merged = `${base}${chunk}`;
  const chars = Array.from(merged);
  if (chars.length <= REASONING_SUMMARY_MAX_CHARS) {
    return merged;
  }
  return `${chars.slice(0, REASONING_SUMMARY_MAX_CHARS).join("")}…`;
}

export function useProgressTracker(locale: "zh-CN" | "en") {
  const [mode, setMode] = useState<WorkspaceChatMode | null>(null);
  const [activities, setActivities] = useState<ProgressEntry[]>([]);
  const [collapsed, setCollapsed] = useState(false);
  const [startedAtMs, setStartedAtMs] = useState<number | null>(null);
  /** When set, stream is done: freeze elapsed and show completed summary (Grok end-state). */
  const [endedAtMs, setEndedAtMs] = useState<number | null>(null);
  const modeRef = useRef<WorkspaceChatMode | null>(null);
  const activitiesRef = useRef<ProgressEntry[]>([]);
  const startedAtMsRef = useRef<number | null>(null);
  const endedAtMsRef = useRef<number | null>(null);
  const collapsedRef = useRef(false);

  const show = useCallback(
    (m: WorkspaceChatMode) => {
      const now = Date.now();
      modeRef.current = m;
      const initial = getInitialProgressEntry(locale, m);
      const seed: ProgressEntry[] = [{ ...initial, startedAtMs: now }];
      activitiesRef.current = seed;
      startedAtMsRef.current = now;
      endedAtMsRef.current = null;
      collapsedRef.current = false;
      setMode(m);
      setStartedAtMs(now);
      setEndedAtMs(null);
      setActivities(seed);
      // Level-1 card: default expanded. Level-2 sections default collapsed in UI.
      setCollapsed(false);
    },
    [locale],
  );

  const hide = useCallback(() => {
    modeRef.current = null;
    activitiesRef.current = [];
    startedAtMsRef.current = null;
    endedAtMsRef.current = null;
    collapsedRef.current = false;
    setMode(null);
    setActivities([]);
    setCollapsed(false);
    setStartedAtMs(null);
    setEndedAtMs(null);
  }, []);

  /** Freeze timer; card stays expanded. Returns a snapshot for attachment to the assistant row. */
  const finalize = useCallback((): UiProgressSnapshot | null => {
    if (modeRef.current == null) {
      return null;
    }
    const ended = Date.now();
    endedAtMsRef.current = ended;
    collapsedRef.current = false;
    setEndedAtMs(ended);
    setCollapsed(false);
    return {
      mode: modeRef.current,
      activities: activitiesRef.current.map((entry) => ({ ...entry })),
      startedAtMs: startedAtMsRef.current,
      endedAtMs: ended,
      collapsed: false,
    };
  }, []);

  const snapshot = useCallback((): UiProgressSnapshot | null => {
    if (modeRef.current == null) {
      return null;
    }
    return {
      mode: modeRef.current,
      activities: activitiesRef.current.map((entry) => ({ ...entry })),
      startedAtMs: startedAtMsRef.current,
      endedAtMs: endedAtMsRef.current,
      collapsed: collapsedRef.current,
    };
  }, []);

  const addActivity = useCallback(
    (event: Extract<ChatEvent, { event: "activity" }>) => {
      const now = Date.now();
      const { title, detail } = localizeProgressActivity(locale, event);
      setActivities((current) => {
        const next = [
          ...current,
          {
            id: `${event.phase}-${current.length}-${event.timestamp ?? now}`,
            phase: event.phase,
            title,
            detail,
            counts: event.counts,
            sourcesPreview: event.sources_preview.map((source) => ({
              id: source.id,
              label: source.label,
              href: source.href ?? undefined,
            })),
            timestamp: event.timestamp ?? null,
            startedAtMs: now,
          },
        ];
        activitiesRef.current = next;
        return next;
      });
    },
    [locale],
  );

  const addReasoning = useCallback(
    (content: string) => {
      if (!content) {
        return;
      }
      const now = Date.now();
      setActivities((current) => {
        const last = current[current.length - 1];
        // Append into one reasoning step; cap length; do not force-expand (section stays collapsed).
        if (last?.phase === "reasoning") {
          const next = current.map((entry, index) =>
            index === current.length - 1
              ? { ...entry, detail: appendCappedReasoning(entry.detail, content) }
              : entry,
          );
          activitiesRef.current = next;
          return next;
        }
        const next = [
          ...current,
          {
            id: `reasoning-${current.length}-${now}`,
            phase: "reasoning",
            title: formatUiMessage(locale, "progress.reasonPreview"),
            detail: appendCappedReasoning(null, content),
            counts: {},
            sourcesPreview: [],
            timestamp: null,
            startedAtMs: now,
          },
        ];
        activitiesRef.current = next;
        return next;
      });
    },
    [locale],
  );

  const toggleCollapsed = useCallback(() => {
    setCollapsed((c) => {
      const next = !c;
      collapsedRef.current = next;
      return next;
    });
  }, []);

  return {
    progress: { mode, activities, collapsed, startedAtMs, endedAtMs },
    show,
    hide,
    finalize,
    snapshot,
    addActivity,
    addReasoning,
    toggleCollapsed,
    modeRef,
  };
}

export type ProgressTracker = ReturnType<typeof useProgressTracker>;
