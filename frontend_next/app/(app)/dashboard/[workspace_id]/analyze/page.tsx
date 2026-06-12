import { redirect } from "next/navigation";

type WorkspaceAnalyzePageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export function generateStaticParams() {
  return [{ workspace_id: "_placeholder" }];
}

export default async function WorkspaceAnalyzePage({ params }: WorkspaceAnalyzePageProps) {
  const { workspace_id } = await params;
  redirect(`/dashboard/${workspace_id}/share#insights`);
}
