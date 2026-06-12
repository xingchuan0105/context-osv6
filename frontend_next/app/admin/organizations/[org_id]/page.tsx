import { AdminOrganizationDetailSurface } from "../../../../components/admin/admin-core-surfaces";

export function generateStaticParams() {
  return [{ org_id: "_placeholder" }];
}

export default function OrganizationDetailPage() {
  return <AdminOrganizationDetailSurface />;
}
