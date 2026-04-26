import { redirect } from "next/navigation";

type WorkspaceAnalyzePageProps = {
  params: {
    workspace_id: string;
  };
};

export default function WorkspaceAnalyzePage({ params }: WorkspaceAnalyzePageProps) {
  redirect(`/dashboard/${params.workspace_id}/share#insights`);
}
