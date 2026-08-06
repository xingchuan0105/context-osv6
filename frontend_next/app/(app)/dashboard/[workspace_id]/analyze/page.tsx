import { redirect } from "next/navigation";

/**
 * Legacy URL. Object-level share traffic lives on Share center (PRODUCT_IA).
 * @see docs/design/PRODUCT_IA.md §4
 */
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
  redirect(`/dashboard/${workspace_id}/share`);
}
