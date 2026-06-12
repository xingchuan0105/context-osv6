import { redirect } from "next/navigation";

type ShareAccessLogsPageProps = {
  params: Promise<{
    workspace_id: string;
  }>;
};

export function generateStaticParams() {
  return [{ workspace_id: "_placeholder" }];
}

export default async function ShareAccessLogsPage({ params }: ShareAccessLogsPageProps) {
  const { workspace_id } = await params;
  redirect(`/dashboard/${workspace_id}/share#activity`);
}
