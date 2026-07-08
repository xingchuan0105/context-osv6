import DOMPurify, { type Config as DOMPurifyConfig } from "dompurify";

export const WORKSPACE_HTML_SANITIZE_CONFIG: DOMPurifyConfig = {
  ALLOWED_TAGS: [
    "a",
    "blockquote",
    "br",
    "button",
    "code",
    "em",
    "h1",
    "h2",
    "h3",
    "input",
    "li",
    "ol",
    "p",
    "pre",
    "span",
    "strong",
    "table",
    "tbody",
    "td",
    "th",
    "thead",
    "tr",
    "ul",
  ],
  ALLOWED_ATTR: [
    "aria-label",
    "checked",
    "class",
    "data-inline-citation-token-index",
    "data-testid",
    "disabled",
    "href",
    "rel",
    "target",
    "type",
  ],
  ALLOW_DATA_ATTR: false,
};

export function sanitizeWorkspaceHtml(html: string) {
  return DOMPurify.sanitize(html, WORKSPACE_HTML_SANITIZE_CONFIG);
}
