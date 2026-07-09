import type { WorkspaceChatMessage } from "./client";
import type { WorkspaceSession } from "./model";
import type { AnswerBlock } from "./stream";

export type SessionSearchDocument = {
  text: string;
  updatedAt: string;
};

export type SessionSearchResult = {
  id: string;
  title: string;
  description: string;
  updatedAtLabel: string;
};

export function normalizeSearchText(value: string | null | undefined) {
  return value?.replace(/\s+/g, " ").trim().toLowerCase() ?? "";
}

export function collapseWhitespace(value: string | null | undefined) {
  return value?.replace(/\s+/g, " ").trim() ?? "";
}

export function extractMessageSearchText(message: WorkspaceChatMessage) {
  const answerText = (message.answer_blocks ?? [])
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join(" ");

  return [message.content, answerText].map(collapseWhitespace).filter(Boolean).join(" ");
}

export function stripSessionTitleMarkdownPrefix(value: string) {
  return value
    .replace(/^(?:(?:#{1,6}|>|[-*+])\s+|\d+[.)]\s+|\[[ xX]\]\s+|`{1,3}(?:[\w-]+)?\s*)+/u, "")
    .replace(/^\[(.+?)\]\((.+?)\)$/u, "$1")
    .replace(/^`([^`]+)`$/u, "$1")
    .replace(/^\*\*([^*]+)\*\*$/u, "$1")
    .replace(/^__([^_]+)__$/u, "$1")
    .replace(/^\*([^*]+)\*$/u, "$1")
    .replace(/^_([^_]+)_$/u, "$1")
    .replace(/^["'""'']+|["'""'']+$/gu, "")
    .trim();
}

export function extractLeadingSentence(value: string) {
  const matched = value.match(/^(.+?(?:[。！？!?]|(?:\.(?=\s|$))))/u);
  return matched?.[1] ?? value;
}

export function trimSessionTitleSuffix(value: string) {
  return value.replace(/[。！？!?.,，、:：;；\-–—\s]+$/u, "").trim();
}

export function extractSessionTitleText(value: string | null | undefined) {
  const collapsed = collapseWhitespace(value);

  if (!collapsed) {
    return "";
  }

  const firstLine = collapsed.split(/[\r\n]+/u)[0] ?? collapsed;
  const withoutPrefix = stripSessionTitleMarkdownPrefix(firstLine);
  const firstSentence = extractLeadingSentence(withoutPrefix);
  const normalized = trimSessionTitleSuffix(firstSentence);

  if (!normalized) {
    return "";
  }

  const maxLength = 48;
  if (normalized.length <= maxLength) {
    return normalized;
  }

  return `${trimSessionTitleSuffix(normalized.slice(0, maxLength).trim())}…`;
}

export function extractSessionTitleFromMessages(messages: WorkspaceChatMessage[]) {
  for (const message of messages) {
    if (message.role !== "user") {
      continue;
    }

    const title = extractSessionTitleText(message.content);

    if (title) {
      return title;
    }
  }

  return "";
}

export function buildSearchSnippet(session: WorkspaceSession, query: string, documentText: string) {
  const normalizedQuery = normalizeSearchText(query);
  const candidates = [documentText, session.title ?? ""]
    .map(collapseWhitespace)
    .filter(Boolean);

  if (candidates.length === 0) {
    return "";
  }

  const matchedCandidate =
    candidates.find((candidate) => candidate.toLowerCase().includes(normalizedQuery)) ?? candidates[0];

  if (!normalizedQuery) {
    return matchedCandidate;
  }

  const lowerCandidate = matchedCandidate.toLowerCase();
  const matchIndex = lowerCandidate.indexOf(normalizedQuery);

  if (matchIndex < 0) {
    return matchedCandidate;
  }

  const start = Math.max(0, matchIndex - 48);
  const end = Math.min(matchedCandidate.length, matchIndex + normalizedQuery.length + 72);
  const prefix = start > 0 ? "..." : "";
  const suffix = end < matchedCandidate.length ? "..." : "";

  return `${prefix}${matchedCandidate.slice(start, end).trim()}${suffix}`;
}

export function formatSessionUpdatedAt(locale: string, updatedAt: string) {
  const parsed = new Date(updatedAt);

  if (Number.isNaN(parsed.valueOf())) {
    return updatedAt;
  }

  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed);
}
