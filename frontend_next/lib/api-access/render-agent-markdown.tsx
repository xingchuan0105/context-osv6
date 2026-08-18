import type { ReactNode } from "react";

/**
 * Minimal markdown → semantic HTML for the agent docs page.
 * Goal: real H1/H2/H3 for crawlers (GEO structure score), not a single <pre>.
 * Supports: ATX headings, fenced code, tables (pipe), lists, paragraphs, hr.
 */
export function renderAgentMarkdown(source: string): ReactNode[] {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const nodes: ReactNode[] = [];
  let i = 0;
  let key = 0;

  while (i < lines.length) {
    const line = lines[i] ?? "";

    // fenced code
    if (line.trimStart().startsWith("```")) {
      const fence = line.trimStart();
      const lang = fence.slice(3).trim();
      i += 1;
      const body: string[] = [];
      while (i < lines.length && !(lines[i] ?? "").trimStart().startsWith("```")) {
        body.push(lines[i] ?? "");
        i += 1;
      }
      if (i < lines.length) i += 1; // closing fence
      nodes.push(
        <pre
          key={key++}
          data-lang={lang || undefined}
          style={{
            margin: "0.75rem 0",
            padding: "0.85rem 1rem",
            overflow: "auto",
            borderRadius: "8px",
            border: "1px solid hsl(var(--border))",
            background: "hsl(var(--surface-muted))",
            fontSize: "0.85rem",
            lineHeight: 1.5,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          <code>{body.join("\n")}</code>
        </pre>,
      );
      continue;
    }

    // headings
    const heading = /^(#{1,3})\s+(.+)$/.exec(line);
    if (heading) {
      const level = heading[1]!.length;
      const text = heading[2]!.trim();
      if (level === 1) {
        nodes.push(
          <h1 key={key++} className="app-page-title" style={{ fontSize: "1.45rem", margin: "1.25rem 0 0.5rem" }}>
            {text}
          </h1>,
        );
      } else if (level === 2) {
        nodes.push(
          <h2 key={key++} style={{ fontSize: "1.2rem", margin: "1.15rem 0 0.45rem", fontWeight: 400 }}>
            {text}
          </h2>,
        );
      } else {
        nodes.push(
          <h3 key={key++} style={{ fontSize: "1.05rem", margin: "1rem 0 0.35rem", fontWeight: 400 }}>
            {text}
          </h3>,
        );
      }
      i += 1;
      continue;
    }

    // horizontal rule
    if (/^(-{3,}|\*{3,}|_{3,})\s*$/.test(line.trim())) {
      nodes.push(
        <hr key={key++} style={{ border: 0, borderTop: "1px solid hsl(var(--border))", margin: "1rem 0" }} />,
      );
      i += 1;
      continue;
    }

    // table block
    if (line.includes("|") && i + 1 < lines.length && /^\s*\|?[\s:-]+\|/.test(lines[i + 1] ?? "")) {
      const tableLines: string[] = [];
      while (i < lines.length && (lines[i] ?? "").includes("|")) {
        tableLines.push(lines[i] ?? "");
        i += 1;
      }
      const rows = tableLines
        .filter((row, idx) => idx !== 1) // skip separator
        .map((row) =>
          row
            .replace(/^\|/, "")
            .replace(/\|$/, "")
            .split("|")
            .map((cell) => cell.trim()),
        );
      if (rows.length > 0) {
        const [header, ...body] = rows;
        nodes.push(
          <div key={key++} style={{ overflowX: "auto", margin: "0.75rem 0" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.9rem" }}>
              <thead>
                <tr>
                  {header!.map((cell, ci) => (
                    <th
                      key={ci}
                      style={{
                        textAlign: "left",
                        borderBottom: "1px solid hsl(var(--border))",
                        padding: "0.4rem 0.55rem",
                      }}
                    >
                      {cell}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {body.map((row, ri) => (
                  <tr key={ri}>
                    {row.map((cell, ci) => (
                      <td
                        key={ci}
                        style={{
                          borderBottom: "1px solid hsl(var(--border))",
                          padding: "0.4rem 0.55rem",
                          verticalAlign: "top",
                        }}
                      >
                        {cell}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>,
        );
      }
      continue;
    }

    // unordered list
    if (/^\s*[-*+]\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*[-*+]\s+/.test(lines[i] ?? "")) {
        items.push((lines[i] ?? "").replace(/^\s*[-*+]\s+/, "").trim());
        i += 1;
      }
      nodes.push(
        <ul
          key={key++}
          style={{
            margin: "0.4rem 0 0.75rem",
            paddingLeft: "1.25rem",
            color: "hsl(var(--muted-foreground))",
            display: "grid",
            gap: "0.35rem",
          }}
        >
          {items.map((item, ii) => (
            <li key={ii}>{item}</li>
          ))}
        </ul>,
      );
      continue;
    }

    // ordered list
    if (/^\s*\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i] ?? "")) {
        items.push((lines[i] ?? "").replace(/^\s*\d+\.\s+/, "").trim());
        i += 1;
      }
      nodes.push(
        <ol
          key={key++}
          style={{
            margin: "0.4rem 0 0.75rem",
            paddingLeft: "1.25rem",
            color: "hsl(var(--muted-foreground))",
            display: "grid",
            gap: "0.35rem",
          }}
        >
          {items.map((item, ii) => (
            <li key={ii}>{item}</li>
          ))}
        </ol>,
      );
      continue;
    }

    // blank
    if (!line.trim()) {
      i += 1;
      continue;
    }

    // paragraph (consume consecutive non-empty non-special lines)
    const para: string[] = [line.trim()];
    i += 1;
    while (
      i < lines.length &&
      (lines[i] ?? "").trim() &&
      !/^(#{1,3})\s+/.test(lines[i] ?? "") &&
      !(lines[i] ?? "").trimStart().startsWith("```") &&
      !/^\s*[-*+]\s+/.test(lines[i] ?? "") &&
      !/^\s*\d+\.\s+/.test(lines[i] ?? "") &&
      !/^(-{3,}|\*{3,}|_{3,})\s*$/.test((lines[i] ?? "").trim())
    ) {
      // stop if next looks like table header pair
      if (
        (lines[i] ?? "").includes("|") &&
        i + 1 < lines.length &&
        /^\s*\|?[\s:-]+\|/.test(lines[i + 1] ?? "")
      ) {
        break;
      }
      para.push((lines[i] ?? "").trim());
      i += 1;
    }
    nodes.push(
      <p
        key={key++}
        style={{
          margin: "0.35rem 0 0.65rem",
          lineHeight: 1.6,
          color: "hsl(var(--foreground))",
        }}
      >
        {para.join(" ")}
      </p>,
    );
  }

  return nodes;
}
