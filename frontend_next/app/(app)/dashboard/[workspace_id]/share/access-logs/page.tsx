import { redirect } from "next/navigation";

type ShareAccessLogsPageProps = {
  params: {
    workspace_id: string;
  };
};

export default function ShareAccessLogsPage({ params }: ShareAccessLogsPageProps) {
  redirect(`/dashboard/${params.workspace_id}/share#activity`);
}
