import { WorkspaceApiAccessSurface } from "../../../../../components/api-access/workspace-api-access-surface";

type ApiAccessPageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export function generateStaticParams() {
  return [{ workspace_id: "_placeholder" }];
}

export default async function ApiAccessPage({ params }: ApiAccessPageProps) {
  const { workspace_id } = await params;
  return <WorkspaceApiAccessSurface workspaceId={workspace_id} />;
}
