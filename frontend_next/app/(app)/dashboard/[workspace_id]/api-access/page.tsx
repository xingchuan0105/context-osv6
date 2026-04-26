import { WorkspaceApiAccessSurface } from "../../../../../components/api-access/workspace-api-access-surface";

type ApiAccessPageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export default async function ApiAccessPage({ params }: ApiAccessPageProps) {
  const resolvedParams = await params;
  const workspace_id = typeof resolvedParams?.workspace_id === "string" ? resolvedParams.workspace_id : "";
  return <WorkspaceApiAccessSurface workspaceId={workspace_id} />;
}
