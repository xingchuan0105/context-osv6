import { InviteSurface } from "../../../../components/share/invite-surface";

type InvitePageProps = {
  params: Promise<{
    workspace_id: string;
    member_id: string;
  }>;
};

export function generateStaticParams() {
  return [{ workspace_id: "_placeholder", member_id: "_placeholder" }];
}

export default async function InvitePage({ params }: InvitePageProps) {
  const { workspace_id, member_id } = await params;
  return <InviteSurface memberId={member_id} workspaceId={workspace_id} />;
}
