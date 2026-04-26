import { SharedWorkspaceSurface } from "../../../../components/share/shared-workspace-surface";

type SharedWorkspacePageProps = {
  params: {
    token: string;
  };
};

export default function SharedWorkspacePage({ params }: SharedWorkspacePageProps) {
  return <SharedWorkspaceSurface shareToken={params.token} />;
}
