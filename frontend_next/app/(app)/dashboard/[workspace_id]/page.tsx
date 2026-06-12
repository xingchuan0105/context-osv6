import { WorkspaceSurface } from "../../../../components/workspace/workspace-surface";

type WorkspacePageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export function generateStaticParams() {
  return [{ workspace_id: "_placeholder" }];
}

export default async function WorkspacePage({ params }: WorkspacePageProps) {
  const { workspace_id } = await params;
  return <WorkspaceSurface workspaceId={workspace_id} />;
}
