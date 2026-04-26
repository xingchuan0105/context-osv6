import { redirect } from "next/navigation";

type ShareAnalyticsPageProps = {
  params: {
    workspace_id: string;
  };
};

export default function ShareAnalyticsPage({ params }: ShareAnalyticsPageProps) {
  redirect(`/dashboard/${params.workspace_id}/share#insights`);
}
