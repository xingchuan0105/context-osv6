"use client";

import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";

import { queryKeys } from "../query/keys";
import {
  addWorkspaceSourceUrl,
  createWorkspaceNote,
  deleteWorkspaceDocument,
  deleteWorkspaceNote,
  getWorkspaceSourceContent,
  getWorkspaceSourceParsedPreview,
  listWorkspaceNotes,
  listWorkspaceSources,
  lookupWorkspaceCitation,
  promoteWorkspaceNote,
  reindexWorkspaceDocument,
  updateWorkspaceNote,
  type CreateWorkspaceNoteRequest,
  type UpdateWorkspaceNoteRequest,
  type WorkspaceParsedPreviewItem,
  type WorkspaceSourceContentResponse,
} from "./client";
import {
  isWorkspaceSourceTerminal,
  sortWorkspaceNotes,
  sortWorkspaceSources,
  type WorkspaceCitationRequest,
  type WorkspaceNote,
  type WorkspaceSource,
} from "./model";

const QUERY_OPTIONS = {
  refetchOnWindowFocus: false,
  retry: false,
} as const;

const VIEWER_STALE_TIME = 5 * 60 * 1000;

export type WorkspaceViewerPreviewPage = {
  hasMore: boolean;
  items: WorkspaceParsedPreviewItem[];
  nextCursor: number;
  summary: string;
};

function citationMatchesPreviewItem(
  citation: WorkspaceCitationRequest["citation"],
  item: WorkspaceParsedPreviewItem,
) {
  if (citation.page !== null && citation.page !== undefined && item.page !== citation.page) {
    return false;
  }

  const preview = citation.preview?.trim();
  const content = citation.content?.trim();
  const previewMatches = preview ? item.text.includes(preview) || preview.includes(item.text) : false;
  const contentMatches = content ? item.text.includes(content) || content.includes(item.text) : false;

  return previewMatches || contentMatches || citation.page === item.page;
}

async function loadWorkspacePreviewPage(
  token: string,
  sourceId: string,
  cursor: number,
  citationRequest: WorkspaceCitationRequest | null,
) {
  let attempts = 0;
  const maxAttempts = cursor === 0 && citationRequest ? 6 : 1;
  let hasMore = false;
  let nextCursor = cursor;
  let summary = "";
  const items: WorkspaceParsedPreviewItem[] = [];

  while (attempts < maxAttempts) {
    const response = await getWorkspaceSourceParsedPreview(token, sourceId, nextCursor, 120);
    summary = summary || response.summary || "";
    items.push(...response.items);
    hasMore = response.has_more;
    nextCursor = response.next_cursor;
    attempts += 1;

    if (!citationRequest || cursor !== 0) {
      break;
    }

    if (items.some((item) => citationMatchesPreviewItem(citationRequest.citation, item)) || !hasMore) {
      break;
    }
  }

  return {
    hasMore,
    items,
    nextCursor,
    summary,
  } satisfies WorkspaceViewerPreviewPage;
}

export async function fetchWorkspaceSourceRawContent(
  queryClient: QueryClient,
  token: string,
  workspaceId: string,
  sourceId: string,
) {
  return queryClient.fetchQuery({
    ...QUERY_OPTIONS,
    staleTime: VIEWER_STALE_TIME,
    queryKey: queryKeys.workspace.sourceRawContent(workspaceId, sourceId),
    queryFn: () => getWorkspaceSourceContent(token, sourceId),
  });
}

export function useWorkspaceSourcesQuery(token: string | null | undefined, workspaceId: string) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: queryKeys.workspace.sources(workspaceId),
    queryFn: async () => {
      const response = await listWorkspaceSources(token as string, workspaceId);
      return response.sources;
    },
    enabled: Boolean(token && workspaceId),
    select: (sources: WorkspaceSource[]) => sortWorkspaceSources(sources),
    refetchInterval: (query) => {
      const data = query.state.data as WorkspaceSource[] | undefined;

      if (!data || data.length === 0) {
        return false;
      }

      return data.every((source) => isWorkspaceSourceTerminal(source.status)) ? false : 2000;
    },
  });
}

export function useWorkspaceNotesQuery(token: string | null | undefined, workspaceId: string) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: queryKeys.workspace.notes(workspaceId),
    queryFn: async () => {
      const response = await listWorkspaceNotes(token as string, workspaceId);
      return response.notes;
    },
    enabled: Boolean(token && workspaceId),
    select: (notes: WorkspaceNote[]) => sortWorkspaceNotes(notes),
  });
}

export function useWorkspaceCitationDetailQuery(
  token: string | null | undefined,
  workspaceId: string,
  citationRequest: WorkspaceCitationRequest | null,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    staleTime: VIEWER_STALE_TIME,
    queryKey: citationRequest
      ? queryKeys.workspace.citation(
          workspaceId,
          citationRequest.session_id,
          citationRequest.message_id,
          citationRequest.citation.citation_id,
        )
      : [...queryKeys.workspace.all, workspaceId, "citation", "idle"],
    queryFn: () =>
      lookupWorkspaceCitation(token as string, {
        session_id: citationRequest!.session_id,
        message_id: citationRequest!.message_id,
        citation_id: citationRequest!.citation.citation_id,
      }),
    enabled: Boolean(token && workspaceId && citationRequest),
  });
}

export function useWorkspaceSourceViewerQuery(
  token: string | null | undefined,
  workspaceId: string,
  sourceId: string | null,
  citationRequest: WorkspaceCitationRequest | null,
) {
  return useInfiniteQuery({
    ...QUERY_OPTIONS,
    initialPageParam: 0,
    staleTime: VIEWER_STALE_TIME,
    queryKey: sourceId
      ? queryKeys.workspace.sourcePreview(
          workspaceId,
          sourceId,
          citationRequest?.session_id,
          citationRequest?.message_id,
          citationRequest?.citation.citation_id,
        )
      : [...queryKeys.workspace.all, workspaceId, "sources", "preview", "idle"],
    queryFn: ({ pageParam }) =>
      loadWorkspacePreviewPage(token as string, sourceId as string, pageParam, citationRequest),
    getNextPageParam: (lastPage) => (lastPage.hasMore ? lastPage.nextCursor : undefined),
    enabled: Boolean(token && workspaceId && sourceId),
  });
}

export function useAddWorkspaceSourceUrlMutation(
  token: string | null | undefined,
  workspaceId: string,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (url: string) => addWorkspaceSourceUrl(token as string, workspaceId, url),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.workspace.sources(workspaceId) });
    },
  });
}

export function useDeleteWorkspaceSourceMutation(
  token: string | null | undefined,
  workspaceId: string,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (sourceId: string) => deleteWorkspaceDocument(token as string, sourceId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.workspace.sources(workspaceId) });
    },
  });
}

export function useReindexWorkspaceSourceMutation(
  token: string | null | undefined,
  workspaceId: string,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (sourceId: string) => reindexWorkspaceDocument(token as string, sourceId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.workspace.sources(workspaceId) });
    },
  });
}

export function useCreateWorkspaceNoteMutation(
  token: string | null | undefined,
  workspaceId: string,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (requestBody: CreateWorkspaceNoteRequest) =>
      createWorkspaceNote(token as string, workspaceId, requestBody),
    onSuccess: async (response) => {
      queryClient.setQueryData(queryKeys.workspace.notes(workspaceId), (current: WorkspaceNote[] | undefined) =>
        sortWorkspaceNotes([...(current ?? []), response.note]),
      );
      await queryClient.invalidateQueries({ queryKey: queryKeys.workspace.notes(workspaceId) });
    },
  });
}

export function useUpdateWorkspaceNoteMutation(
  token: string | null | undefined,
  workspaceId: string,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ noteId, requestBody }: { noteId: string; requestBody: UpdateWorkspaceNoteRequest }) =>
      updateWorkspaceNote(token as string, workspaceId, noteId, requestBody),
    onSuccess: (response) => {
      queryClient.setQueryData(queryKeys.workspace.notes(workspaceId), (current: WorkspaceNote[] | undefined) =>
        sortWorkspaceNotes(
          [...(current ?? []).filter((note) => note.id !== response.note.id), response.note],
        ),
      );
    },
  });
}

export function useDeleteWorkspaceNoteMutation(
  token: string | null | undefined,
  workspaceId: string,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (noteId: string) => deleteWorkspaceNote(token as string, workspaceId, noteId),
    onSuccess: async (_response, noteId) => {
      queryClient.setQueryData(queryKeys.workspace.notes(workspaceId), (current: WorkspaceNote[] | undefined) =>
        (current ?? []).filter((note) => note.id !== noteId),
      );
      await queryClient.invalidateQueries({ queryKey: queryKeys.workspace.notes(workspaceId) });
    },
  });
}

export function usePromoteWorkspaceNoteMutation(
  token: string | null | undefined,
  workspaceId: string,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (noteId: string) => promoteWorkspaceNote(token as string, workspaceId, noteId),
    onSuccess: async (response) => {
      queryClient.setQueryData(queryKeys.workspace.notes(workspaceId), (current: WorkspaceNote[] | undefined) =>
        sortWorkspaceNotes(
          [...(current ?? []).filter((note) => note.id !== response.note.id), response.note],
        ),
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.workspace.notes(workspaceId) }),
        queryClient.invalidateQueries({ queryKey: queryKeys.workspace.sources(workspaceId) }),
      ]);
    },
  });
}
