import type { Metadata } from "next";

import { ProjectLicense } from "@/components/legal/project-license";

export const metadata: Metadata = {
  title: "MIT License",
  description: "Full text of the MIT license used by the Context-OS project.",
  alternates: {
    canonical: "/en/legal/licenses/project",
    languages: {
      "zh-CN": "/legal/licenses/project",
      en: "/en/legal/licenses/project",
      "x-default": "/legal/licenses/project",
    },
  },
};

export default function EnProjectLicensePage() {
  return <ProjectLicense locale="en" />;
}
