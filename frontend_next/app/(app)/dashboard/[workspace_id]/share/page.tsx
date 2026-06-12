import { WorkspaceShareCenterSurface } from "../../../../../components/share/workspace-share-surface";

type SharePageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export function generateStaticParams() {
  return [{ workspace_id: "_placeholder" }];
}

export default async function SharePage({ params }: SharePageProps) {
  const { workspace_id } = await params;
  return <WorkspaceShareCenterSurface workspaceId={workspace_id} />;
}
