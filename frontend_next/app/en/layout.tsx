import type { ReactNode } from "react";

import { OrganizationJsonLd } from "../../components/organization-jsonld";
import { SoftwareApplicationJsonLd } from "../../components/software-application-jsonld";

/**
 * /en/* 公开面统一挂 en 实体标注（Phase E E1.1）：
 * /en 路由不在 (marketing)/(open) 组内，此前完全没有实体 JSON-LD。
 * en 页自带 MarketingShell / 页面级 chrome，这里只补 JSON-LD，不包 chrome。
 */
export default function EnLayout({ children }: { children: ReactNode }) {
  return (
    <>
      <OrganizationJsonLd locale="en" />
      <SoftwareApplicationJsonLd locale="en" />
      {children}
    </>
  );
}
