import { AdminAccountDetailSurface } from "../../../../components/admin/admin-core-surfaces";

export function generateStaticParams() {
  return [{ owner_user_id: "_placeholder" }];
}

export default function AccountDetailPage() {
  return <AdminAccountDetailSurface />;
}
