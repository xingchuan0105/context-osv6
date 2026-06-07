# Gotchas

## JavaScript-rendered content is not supported

`web_fetch` retrieves static HTML only. Pages that require JavaScript to load their main content (SPAs, React/Vue/Angular apps without SSR) may return empty or minimal text.

**What to return when extraction yields nothing:**

- If the page body is empty after stripping boilerplate, return a successful response with `content: ""` and `length: 0`. Do not fabricate text.
- In the agent response, inform the user: "This page appears to rely on JavaScript for its content and could not be loaded as static HTML."

## Private addresses are blocked

URLs pointing to `localhost`, `127.0.0.1`, `::1`, or RFC1918 private ranges (`10.x.x.x`, `192.168.x.x`, `172.16-31.x.x`) are rejected with an error. This prevents SSRF attacks.

## Non-HTTP schemes are rejected

Only `http://` and `https://` are allowed. `ftp://`, `file://`, `data://`, etc. return an error.

## Content extraction is heuristic

The tool removes common boilerplate tags (`<script>`, `<style>`, `<nav>`, `<header>`, `<footer>`, `<aside>`, `<noscript>`, `<svg>`, `<canvas>`) and strips remaining HTML. On pages with unusual markup, some noise may remain or some content may be lost.

**Cleanup scope**: Navigation, ads, scripts, styles, headers, footers, sidebars, SVGs, and canvases are removed. Main article body, paragraphs, lists, tables, and code blocks are preserved.

## Large pages are truncated

By default only the first 8,000 characters of cleaned text are returned. If the user asks for "the full article", consider calling with a higher `max_length` (up to ~32,000) or ask the user to confirm.

## Do not chain fetch inside loops without reason

Fetching multiple URLs in rapid succession can trigger rate limits or anti-bot measures. If the user provides several URLs, fetch only the most relevant ones and explain the limitation.
