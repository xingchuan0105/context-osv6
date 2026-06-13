"use client";

import { useCallback, useEffect, useRef, useState } from "react";

const TRANSIENT_SOURCE_PROCESSING_MS = 1800;

export function useWorkspaceTransientSourceFilter() {
  const [transientProcessingSourceIds, setTransientProcessingSourceIds] = useState<string[]>([]);
  const transientProcessingTimersRef = useRef<number[]>([]);

  const markSourcesTransientProcessing = useCallback((sourceIds: string[]) => {
    if (sourceIds.length === 0) {
      return;
    }

    setTransientProcessingSourceIds((current) => Array.from(new Set([...current, ...sourceIds])));

    const timer = window.setTimeout(() => {
      setTransientProcessingSourceIds((current) => current.filter((id) => !sourceIds.includes(id)));
      transientProcessingTimersRef.current = transientProcessingTimersRef.current.filter((item) => item !== timer);
    }, TRANSIENT_SOURCE_PROCESSING_MS);

    transientProcessingTimersRef.current.push(timer);
  }, []);

  useEffect(() => {
    return () => {
      transientProcessingTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      transientProcessingTimersRef.current = [];
    };
  }, []);

  const resetTransientFilter = useCallback(() => {
    setTransientProcessingSourceIds([]);
  }, []);

  return {
    markSourcesTransientProcessing,
    resetTransientFilter,
    transientProcessingSourceIds,
  };
}
