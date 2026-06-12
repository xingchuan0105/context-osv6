import { SharedWorkspaceSurface } from "../../../../components/share/shared-workspace-surface";

type SharedWorkspacePageProps = {
  params: Promise<{
    token: string;
  }>;
};

export function generateStaticParams() {
  return [{ token: "_placeholder" }];
}

export default async function SharedWorkspacePage({ params }: SharedWorkspacePageProps) {
  const { token } = await params;
  return <SharedWorkspaceSurface shareToken={token} />;
}
