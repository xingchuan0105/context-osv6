import { InviteSurface } from "../../../../components/share/invite-surface";

type InvitePageProps = {
  params: {
    member_id: string;
    workspace_id: string;
  };
};

export default function InvitePage({ params }: InvitePageProps) {
  return <InviteSurface memberId={params.member_id} workspaceId={params.workspace_id} />;
}
