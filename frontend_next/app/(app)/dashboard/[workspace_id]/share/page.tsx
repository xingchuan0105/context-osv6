import { WorkspaceShareCenterSurface } from "../../../../../components/share/workspace-share-surface";

type SharePageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export default async function SharePage({ params }: SharePageProps) {
  const { workspace_id } = await params;
  return <WorkspaceShareCenterSurface workspaceId={workspace_id} />;
}
