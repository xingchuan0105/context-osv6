function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function escapeMarkdownText(value: string) {
  return value
    .replace(/\\/g, "\\\\")
    .replace(/\[/g, "\\[")
    .replace(/\]/g, "\\]")
    .replace(/\*/g, "\\*")
    .replace(/_/g, "\\_")
    .replace(/`/g, "\\`");
}

function renderInlineMarkdown(value: string) {
  let html = escapeHtml(value);
  const codeTokens: string[] = [];
  html = html.replace(/`([^`]+)`/g, (_match, code) => {
    const token = `__CODE_TOKEN_${codeTokens.length}__`;
    codeTokens.push(`<code>${code}</code>`);
    return token;
  });
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  html = html.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  html = html.replace(/__([^_]+)__/g, "<strong>$1</strong>");
  html = html.replace(/\*([^*]+)\*/g, "<em>$1</em>");
  html = html.replace(/_([^_]+)_/g, "<em>$1</em>");
  html = codeTokens.reduce(
    (current, codeHtml, index) => current.replace(`__CODE_TOKEN_${index}__`, codeHtml),
    html,
  );
  return html;
}

function renderParagraph(lines: string[]) {
  return `<p>${lines.map((line) => renderInlineMarkdown(line)).join("<br>")}</p>`;
}

export function markdownToInlineHtml(markdown: string | null | undefined) {
  const normalized = (markdown ?? "").replace(/\r\n?/g, "\n").trim();

  if (!normalized) {
    return "";
  }

  return normalized
    .split("\n")
    .map((line) => renderInlineMarkdown(line))
    .join("<br>");
}

function parseTableRow(line: string) {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

function isTableSeparator(line: string) {
  const cells = parseTableRow(line);
  return cells.length > 0 && cells.every((cell) => /^:?-{3,}:?$/.test(cell));
}

function isTaskListLine(line: string) {
  return /^[-*+]\s+\[(?: |x|X)\]\s+/.test(line);
}

export function markdownToRichTextHtml(markdown: string | null | undefined) {
  const normalized = (markdown ?? "").replace(/\r\n?/g, "\n").trim();

  if (!normalized) {
    return "<p></p>";
  }

  const lines = normalized.split("\n");
  const blocks: string[] = [];

  for (let index = 0; index < lines.length; ) {
    const rawLine = lines[index] ?? "";
    const line = rawLine.trimEnd();

    if (!line.trim()) {
      index += 1;
      continue;
    }

    if (/^```/.test(line)) {
      const codeLines: string[] = [];
      index += 1;
      while (index < lines.length && !/^```/.test(lines[index] ?? "")) {
        codeLines.push(lines[index] ?? "");
        index += 1;
      }
      if (index < lines.length && /^```/.test(lines[index] ?? "")) {
        index += 1;
      }
      blocks.push(`<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`);
      continue;
    }

    const headingMatch = line.match(/^(#{1,3})\s+(.*)$/);
    if (headingMatch) {
      const tag = `h${headingMatch[1].length}`;
      blocks.push(`<${tag}>${renderInlineMarkdown(headingMatch[2])}</${tag}>`);
      index += 1;
      continue;
    }

    if (/^>\s?/.test(line)) {
      const quoteLines: string[] = [];
      while (index < lines.length && /^>\s?/.test(lines[index] ?? "")) {
        quoteLines.push((lines[index] ?? "").replace(/^>\s?/, ""));
        index += 1;
      }
      blocks.push(`<blockquote>${quoteLines.map((item) => `<p>${renderInlineMarkdown(item)}</p>`).join("")}</blockquote>`);
      continue;
    }

    if (line.includes("|") && isTableSeparator(lines[index + 1] ?? "")) {
      const headerCells = parseTableRow(line);
      const columnCount = headerCells.length;
      const rows: string[][] = [];
      index += 2;
      while (index < lines.length) {
        const rowLine = lines[index] ?? "";
        if (!rowLine.trim() || !rowLine.includes("|")) {
          break;
        }
        rows.push(parseTableRow(rowLine));
        index += 1;
      }
      blocks.push(
        `<table><thead><tr>${headerCells.map((cell) => `<th>${renderInlineMarkdown(cell)}</th>`).join("")}</tr></thead>${
          rows.length > 0
            ? `<tbody>${rows
                .map(
                  (row) =>
                    `<tr>${Array.from({ length: columnCount }, (_value, cellIndex) => `<td>${renderInlineMarkdown(row[cellIndex] ?? "")}</td>`).join("")}</tr>`,
                )
                .join("")}</tbody>`
            : ""
        }</table>`,
      );
      continue;
    }

    if (/^\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^\d+\.\s+/.test(lines[index] ?? "")) {
        items.push((lines[index] ?? "").replace(/^\d+\.\s+/, ""));
        index += 1;
      }
      blocks.push(`<ol>${items.map((item) => `<li>${renderInlineMarkdown(item)}</li>`).join("")}</ol>`);
      continue;
    }

    if (isTaskListLine(line)) {
      const items: Array<{ checked: boolean; text: string }> = [];
      while (index < lines.length && isTaskListLine(lines[index] ?? "")) {
        const currentLine = lines[index] ?? "";
        const checked = /^[-*+]\s+\[(?:x|X)\]\s+/.test(currentLine);
        items.push({
          checked,
          text: currentLine.replace(/^[-*+]\s+\[(?: |x|X)\]\s+/, ""),
        });
        index += 1;
      }
      blocks.push(
        `<ul class="taskList">${items
          .map(
            (item) =>
              `<li class="taskListItem"><input type="checkbox" disabled${item.checked ? " checked" : ""} /><span>${renderInlineMarkdown(item.text)}</span></li>`,
          )
          .join("")}</ul>`,
      );
      continue;
    }

    if (/^[-*+]\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^[-*+]\s+/.test(lines[index] ?? "")) {
        items.push((lines[index] ?? "").replace(/^[-*+]\s+/, ""));
        index += 1;
      }
      blocks.push(`<ul>${items.map((item) => `<li>${renderInlineMarkdown(item)}</li>`).join("")}</ul>`);
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length) {
      const candidate = lines[index] ?? "";
      if (!candidate.trim()) {
        break;
      }
      if (
        /^```/.test(candidate) ||
        /^>\s?/.test(candidate) ||
        (candidate.includes("|") && isTableSeparator(lines[index + 1] ?? "")) ||
        /^(#{1,3})\s+/.test(candidate) ||
        /^\d+\.\s+/.test(candidate) ||
        isTaskListLine(candidate) ||
        /^[-*+]\s+/.test(candidate)
      ) {
        break;
      }
      paragraphLines.push(candidate);
      index += 1;
    }
    blocks.push(renderParagraph(paragraphLines));
  }

  return blocks.join("");
}

function serializeInlineNode(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    return escapeMarkdownText(node.textContent ?? "");
  }

  if (!(node instanceof HTMLElement)) {
    return "";
  }

  const tag = node.tagName.toLowerCase();

  if (tag === "br") {
    return "\n";
  }

  const children = Array.from(node.childNodes).map(serializeInlineNode).join("");

  if (!children) {
    return "";
  }

  if (tag === "strong" || tag === "b") {
    return `**${children}**`;
  }

  if (tag === "em" || tag === "i") {
    return `*${children}*`;
  }

  if (tag === "a") {
    const href = node.getAttribute("href")?.trim();
    return href ? `[${children}](${href})` : children;
  }

  return children;
}

function serializeBlockNode(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    return escapeMarkdownText(node.textContent ?? "").trim();
  }

  if (!(node instanceof HTMLElement)) {
    return "";
  }

  const tag = node.tagName.toLowerCase();

  if (tag === "ul" || tag === "ol") {
    const items = Array.from(node.children)
      .filter((child) => child.tagName.toLowerCase() === "li")
      .map((child, index) => {
        const content = Array.from(child.childNodes).map(serializeInlineNode).join("").trim();
        if (!content) {
          return "";
        }
        return tag === "ol" ? `${index + 1}. ${content}` : `- ${content}`;
      })
      .filter(Boolean);

    return items.join("\n");
  }

  const content = Array.from(node.childNodes).map(serializeInlineNode).join("").trim();

  if (!content) {
    return "";
  }

  if (tag === "h1") {
    return `# ${content}`;
  }

  if (tag === "h2") {
    return `## ${content}`;
  }

  return content;
}

export function richTextEditorToMarkdown(root: HTMLElement | null) {
  if (!root) {
    return "";
  }

  const blocks = Array.from(root.childNodes)
    .map(serializeBlockNode)
    .filter((block) => block.trim().length > 0);

  if (blocks.length === 0) {
    const fallback = escapeMarkdownText(root.textContent ?? "").trim();
    return fallback;
  }

  return blocks.join("\n\n").replace(/\n{3,}/g, "\n\n").trim();
}

export function markdownToPlainText(markdown: string | null | undefined) {
  return (markdown ?? "")
    .replace(/\r\n?/g, "\n")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/__([^_]+)__/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/_([^_]+)_/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}
