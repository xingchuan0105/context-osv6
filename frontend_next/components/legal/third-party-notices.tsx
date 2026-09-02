import fs from "fs";
import path from "path";
import Link from "next/link";

import LegalLayout from "@/components/legal/LegalLayout";
import { renderLegalMarkdown } from "@/lib/legal/render-markdown";
import type { UiLocale } from "@/lib/i18n/config";

const copy: Record<UiLocale, { title: string; generated: string; total: string; download: string; back: string; loading: string }> = {
  "zh-CN": {
    title: "完整第三方组件声明",
    generated: "生成日期",
    total: "组件总数",
    download: "下载 .md",
    back: "返回开源摘要",
    loading: "第三方声明文件正在生成中，请运行 `pnpm sync:legal` 生成。",
  },
  en: {
    title: "Full third-party notices",
    generated: "Generated",
    total: "Total components",
    download: "Download .md",
    back: "Back to open-source summary",
    loading: "Third-party notices are being generated; run `pnpm sync:legal`.",
  },
};

export async function ThirdPartyNotices({ locale }: { locale: UiLocale }) {
  const t = copy[locale];
  const noticesPath = path.join(process.cwd(), "public/legal/third-party-notices.md");
  let noticesContent = "";
  let totalPackages = 0;
  let generationDate = "";

  try {
    noticesContent = fs.readFileSync(noticesPath, "utf8");
    // 统计 ### 级别标题作为组件条目数
    const componentEntries = noticesContent.match(/^### /gm);
    totalPackages = componentEntries?.length || 0;
    // 尝试从生成日期注释中提取
    const dateMatch = noticesContent.match(
      /[Gg]enerated:\s*(\d{4}-\d{2}-\d{2})/,
    );
    generationDate = dateMatch?.[1] || new Date().toISOString().split("T")[0];
  } catch {
    noticesContent = t.loading;
    generationDate = new Date().toISOString().split("T")[0];
  }

  const { html, toc } = await renderLegalMarkdown(noticesContent);

  return (
    <LegalLayout title={t.title} toc={toc}>
      <div className="third-party-notices">
        <div className="notices-header">
          <div className="notices-stats">
            <p>{t.generated}: {generationDate}</p>
            <p>{t.total}: {totalPackages}+</p>
          </div>
          <div className="notices-actions">
            <a
              href="/legal/third-party-notices.md"
              download
              className="app-button-secondary"
            >
              {t.download}
            </a>
          </div>
        </div>

        <div
          className="notices-content"
          dangerouslySetInnerHTML={{ __html: html }}
        />

        <div className="notices-footer">
          <Link href="/legal/licenses">{t.back}</Link>
        </div>
      </div>
    </LegalLayout>
  );
}
