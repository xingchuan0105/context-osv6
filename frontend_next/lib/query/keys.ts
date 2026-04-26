const authAll = ["auth"] as const;
const adminAll = ["admin"] as const;
const apiAccessAll = ["api-access"] as const;
const dashboardAll = ["dashboard"] as const;
const settingsAll = ["settings"] as const;
const shareAll = ["share"] as const;
const workspaceAll = ["workspace"] as const;

export const queryKeys = {
  auth: {
    all: authAll,
    me: () => [...authAll, "me"] as const,
    runtimeCapabilities: () => [...authAll, "runtime-capabilities"] as const,
  },
  admin: {
    all: adminAll,
    overview: () => [...adminAll, "overview"] as const,
  },
  apiAccess: {
    all: apiAccessAll,
    list: () => [...apiAccessAll, "list"] as const,
  },
  dashboard: {
    all: dashboardAll,
    workspaces: () => [...dashboardAll, "workspaces"] as const,
  },
  settings: {
    all: settingsAll,
    profile: () => [...settingsAll, "profile"] as const,
  },
  share: {
    all: shareAll,
    workspace: (workspaceId: string) => [...shareAll, "workspace", workspaceId] as const,
  },
  workspace: {
    all: workspaceAll,
    detail: (workspaceId: string) => [...workspaceAll, workspaceId] as const,
    sessions: (workspaceId: string) => [...workspaceAll, workspaceId, "sessions"] as const,
    session: (sessionId: string) => [...workspaceAll, "session", sessionId] as const,
    messages: (sessionId: string) => [...workspaceAll, "session", sessionId, "messages"] as const,
    sources: (workspaceId: string) => [...workspaceAll, workspaceId, "sources"] as const,
    sourceContent: (workspaceId: string, sourceId: string) =>
      [...workspaceAll, workspaceId, "sources", sourceId, "content"] as const,
    sourceRawContent: (workspaceId: string, sourceId: string) =>
      [...workspaceAll, workspaceId, "sources", sourceId, "raw-content"] as const,
    sourcePreview: (
      workspaceId: string,
      sourceId: string,
      sessionId: string | null | undefined,
      messageId: number | null | undefined,
      citationId: number | null | undefined,
    ) =>
      [
        ...workspaceAll,
        workspaceId,
        "sources",
        sourceId,
        "preview",
        sessionId ?? "none",
        messageId ?? "none",
        citationId ?? "none",
      ] as const,
    notes: (workspaceId: string) => [...workspaceAll, workspaceId, "notes"] as const,
    citation: (
      workspaceId: string,
      sessionId: string,
      messageId: number,
      citationId: number | null | undefined,
    ) => [...workspaceAll, workspaceId, "citation", sessionId, messageId, citationId ?? "unknown"] as const,
  },
} as const;
