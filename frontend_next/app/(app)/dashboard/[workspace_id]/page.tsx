import { WorkspaceSurface } from "../../../../components/workspace/workspace-surface";

type WorkspacePageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export default async function WorkspacePage({ params }: WorkspacePageProps) {
  const { workspace_id } = await params;
  return <WorkspaceSurface workspaceId={workspace_id} />;
}
