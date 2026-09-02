import fs from "fs";
import path from "path";

import LegalLayout from "@/components/legal/LegalLayout";
import type { UiLocale } from "@/lib/i18n/config";

const title: Record<UiLocale, string> = {
  "zh-CN": "MIT许可证",
  en: "MIT License",
};

export function ProjectLicense({ locale }: { locale: UiLocale }) {
  const licensePath = path.join(process.cwd(), "public/legal/LICENSE");
  let licenseContent = "";

  try {
    licenseContent = fs.readFileSync(licensePath, "utf8");
  } catch {
    // 回退到根目录 LICENSE
    try {
      const rootLicense = path.join(process.cwd(), "../../LICENSE");
      licenseContent = fs.readFileSync(rootLicense, "utf8");
    } catch {
      licenseContent = locale === "en" ? "MIT license file is loading…" : "MIT许可证文件正在加载中...";
    }
  }

  return (
    <LegalLayout title={title[locale]}>
      <div className="project-license">
        <div className="license-content">
          <pre className="license-text">{licenseContent}</pre>
        </div>
      </div>
    </LegalLayout>
  );
}
