"use client";

import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { queryKeys } from "../../lib/query/keys";
import {
  fetchWorkspaceSourceRawContent,
  useWorkspaceSourceViewerQuery,
} from "../../lib/workspace/right-rail-queries";
import type { WorkspaceSource } from "../../lib/workspace/model";

export function useWorkspaceSourceViewerExpansion({
  token,
  workspaceId,
  locale,
  sources,
  sourcesPending,
}: {
  token: string | null;
  workspaceId: string;
  locale: "zh-CN" | "en";
  sources: WorkspaceSource[];
  sourcesPending: boolean;
}) {
  const queryClient = useQueryClient();

  const [viewerSourceId, setViewerSourceId] = useState<string | null>(null);
  const [viewerRawContent, setViewerRawContent] = useState("");
  const [viewerRawSummary, setViewerRawSummary] = useState("");
  const [viewerRawLoading, setViewerRawLoading] = useState(false);
  const [viewerError, setViewerError] = useState("");

  const sourceViewerQuery = useWorkspaceSourceViewerQuery(token, workspaceId, viewerSourceId, null);

  const viewerPreview = useMemo(
    () => sourceViewerQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [sourceViewerQuery.data],
  );
  const viewerSummary = useMemo(() => {
    const previewSummary = sourceViewerQuery.data?.pages.find((page) => page.summary)?.summary ?? "";

    return previewSummary || viewerRawSummary;
  }, [sourceViewerQuery.data, viewerRawSummary]);
  const viewerSource = viewerSourceId ? sources.find((source) => source.id === viewerSourceId) ?? null : null;
  const viewerLoading = Boolean(viewerSourceId) && (sourceViewerQuery.isPending || viewerRawLoading);
  const viewerLoadingMore = sourceViewerQuery.isFetchingNextPage;
  const viewerHasMore = Boolean(sourceViewerQuery.hasNextPage && !viewerRawContent);

  const resetViewerExpansion = useCallback(() => {
    setViewerSourceId(null);
    setViewerRawContent("");
    setViewerRawSummary("");
    setViewerRawLoading(false);
    setViewerError("");
  }, []);

  useEffect(() => {
    resetViewerExpansion();
  }, [resetViewerExpansion, workspaceId]);

  useEffect(() => {
    if (!viewerSourceId) {
      setViewerRawContent("");
      setViewerRawSummary("");
      setViewerRawLoading(false);
      setViewerError("");
      return;
    }

    setViewerRawContent("");
    setViewerRawSummary("");
    setViewerRawLoading(false);
    setViewerError("");
  }, [viewerSourceId]);

  useEffect(() => {
    if (!viewerSourceId || !token || !sourceViewerQuery.isError) {
      return;
    }

    if (viewerRawContent || viewerRawLoading || viewerError) {
      return;
    }

    let cancelled = false;
    setViewerRawLoading(true);

    void fetchWorkspaceSourceRawContent(queryClient, token, workspaceId, viewerSourceId)
      .then((response) => {
        if (cancelled) {
          return;
        }

        setViewerRawContent(response.content);
        setViewerRawSummary(response.summary ?? "");
        setViewerError("");
      })
      .catch(() => {
        if (!cancelled) {
          setViewerError(formatUiMessage(locale, "workspaceRightRail.viewerError"));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setViewerRawLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    locale,
    queryClient,
    sourceViewerQuery.isError,
    token,
    viewerError,
    viewerRawContent,
    viewerRawLoading,
    viewerSourceId,
    workspaceId,
  ]);

  useEffect(() => {
    if (viewerSourceId && !sourcesPending && !sources.some((source) => source.id === viewerSourceId)) {
      setViewerSourceId(null);
    }
  }, [sources, sourcesPending, viewerSourceId]);

  const handleOpenSource = useCallback((sourceId: string) => {
    setViewerSourceId((current) => (current === sourceId ? null : sourceId));
  }, []);

  return {
    handleOpenSource,
    resetViewerExpansion,
    setViewerSourceId,
    sourceViewerQuery,
    viewerError,
    viewerHasMore,
    viewerLoading,
    viewerLoadingMore,
    viewerPreview,
    viewerRawContent,
    viewerSource,
    viewerSourceId,
    viewerSummary,
  };
}
