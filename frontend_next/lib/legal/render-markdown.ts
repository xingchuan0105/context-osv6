import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypeSlug from "rehype-slug";
import rehypeStringify from "rehype-stringify";
import remarkGfm from "remark-gfm";
import remarkParse from "remark-parse";
import remarkRehype from "remark-rehype";
import { unified } from "unified";

export interface TocEntry {
  id: string;
  text: string;
  depth: number;
}

/**
 * 将 Markdown 转换为 HTML，同时提取目录（TOC）。
 * 使用 remark-gfm 支持表格、任务列表等。
 * 使用 rehype-slug 为标题生成 id，rehype-autolink-headings 添加锚点链接。
 */
export async function renderLegalMarkdown(
  markdown: string,
): Promise<{ html: string; toc: TocEntry[] }> {
  const toc: TocEntry[] = [];

  const processor = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(remarkRehype, { allowDangerousHtml: true })
    .use(rehypeSlug)
    .use(rehypeAutolinkHeadings, {
      behavior: "wrap",
      properties: {
        className: ["legal-heading-anchor"],
      },
    })
    .use(rehypeStringify, { allowDangerousHtml: true });

  const file = await processor.process(markdown);
  const html = String(file);

  // 从生成的 HTML 中提取标题构建 TOC
  const headingRegex = /<h([2-3])\s+id="([^"]*)"[^>]*>(.*?)<\/h[2-3]>/g;
  let match;
  while ((match = headingRegex.exec(html)) !== null) {
    const depth = parseInt(match[1], 10);
    const id = match[2];
    // 去除锚点链接标签，只保留文本
    const text = match[3].replace(/<[^>]+>/g, "").trim();
    toc.push({ id, text, depth });
  }

  return { html, toc };
}
