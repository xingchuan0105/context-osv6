"use client";

import { use } from "react";

import { WorkspaceAnalyzeSurface } from "@/components/share/workspace-analyze-surface";

type WorkspaceAnalyzePageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export default function WorkspaceAnalyzePage({ params }: WorkspaceAnalyzePageProps) {
  const { workspace_id } = use(params);
  return <WorkspaceAnalyzeSurface workspaceId={workspace_id} />;
}
