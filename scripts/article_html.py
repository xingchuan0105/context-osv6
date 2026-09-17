#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Markdown → Ghost / WeChat HTML. Shared by deploy-blog-post.sh and wechat-publish.py."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

WECHAT_CONTAINER = (
    "font-family: -apple-system-font, BlinkMacSystemFont, 'Helvetica Neue', 'PingFang SC', "
    "'Hiragino Sans GB', 'Microsoft YaHei UI', 'Microsoft YaHei', Arial, sans-serif; "
    "font-size: 15.5px; line-height: 1.85; color: #2c3e50; letter-spacing: 0.5px; "
    "padding: 10px 8px; word-break: break-word;"
)
WECHAT_H2 = (
    "font-size: 18.5px; font-weight: bold; color: #111827; margin: 28px 0 14px; "
    "border-left: 4px solid #00c9a7; padding-left: 10px; line-height: 1.4;"
)
WECHAT_H3 = "font-size: 16.5px; font-weight: bold; color: #1f2937; margin: 20px 0 10px;"
WECHAT_P = "margin: 14px 0; text-align: justify;"
WECHAT_QUOTE = (
    "background: #f8fafc; border-left: 4px solid #00c9a7; padding: 12px 16px; "
    "color: #475569; font-size: 14.5px; line-height: 1.75; margin: 18px 0; "
    "border-radius: 0 4px 4px 0;"
)
WECHAT_QUOTE_P = "margin: 0 0 8px;"
WECHAT_QUOTE_P_LAST = "margin: 0;"
WECHAT_CODE = (
    "background: #1e1e1e; color: #d4d4d4; padding: 14px 16px; border-radius: 6px; "
    "font-family: Menlo, Monaco, Consolas, 'Courier New', monospace; font-size: 13px; "
    "line-height: 1.6; overflow-x: auto; margin: 18px 0; -webkit-overflow-scrolling: touch;"
)
WECHAT_INLINE_CODE = (
    "background: #f1f5f9; color: #0f766e; padding: 2px 6px; border-radius: 4px; "
    "font-family: Menlo, Monaco, Consolas, monospace; font-size: 13.5px; margin: 0 2px;"
)
WECHAT_HR = "border: none; border-top: 1px solid #e2e8f0; margin: 30px 0;"
WECHAT_UL = "margin: 14px 0; padding-left: 20px;"
WECHAT_LI = "margin: 6px 0;"
WECHAT_IMG = "max-width: 100%; height: auto; display: block; margin: 20px auto 6px; border-radius: 4px;"
WECHAT_CAPTION = "font-size: 12.5px; color: #64748b; text-align: center; margin: 0 0 22px; line-height: 1.6;"
WECHAT_TABLE = "border-collapse: collapse; width: 100%; margin: 18px 0; font-size: 13.5px; line-height: 1.6;"
WECHAT_TH = (
    "background: #f1f5f9; border: 1px solid #cbd5e1; padding: 8px 10px; "
    "font-weight: bold; color: #1e293b; text-align: left;"
)
WECHAT_TD = "border: 1px solid #cbd5e1; padding: 8px 10px; color: #334155;"
WECHAT_LINK_NOTE = "font-size:12px;color:#64748b;word-break:break-all;"
GHOST_A = "color:#0f766e;text-decoration:underline;"

IMG_RE = re.compile(r"!\[(.*?)\]\((.*?)\)")
LINKED_IMG_RE = re.compile(r"^\[!\[(.*?)\]\((.*?)\)\]\((.*?)\)$")
LINK_RE = re.compile(r"\[(.*?)\]\((.*?)\)")
BARE_URL_RE = re.compile(r"^https?://\S+$")


def resolve_path(p: str) -> str:
    if not p:
        return ""
    if os.path.exists(p):
        return p
    win_match = re.match(r"^([a-zA-Z]):[\\/](.*)", p)
    if win_match:
        drive = win_match.group(1).lower()
        rest = win_match.group(2).replace("\\", "/")
        wsl_path = f"/mnt/{drive}/{rest}"
        if os.path.exists(wsl_path):
            return wsl_path
    wsl_match = re.match(r"^/mnt/([a-zA-Z])/(.*)", p)
    if wsl_match:
        drive = wsl_match.group(1).upper()
        rest = wsl_match.group(2).replace("/", "\\")
        win_path = f"{drive}:\\{rest}"
        if os.path.exists(win_path):
            return win_path
    return p


def resolve_image_path(src: str, base_dir: str) -> str:
    src = src.strip().strip('"').strip("'")
    if os.path.isabs(src) or re.match(r"^[a-zA-Z]:[\\/]", src):
        return resolve_path(src)
    joined = os.path.normpath(os.path.join(base_dir, src))
    return resolve_path(joined)


def collect_images(md_text: str, base_dir: str) -> list[tuple[str, str, str]]:
    """Return (alt, original_src, resolved_path) for local images, in document order, unique by path."""
    seen: set[str] = set()
    out: list[tuple[str, str, str]] = []
    for alt, src in IMG_RE.findall(md_text):
        if src.startswith("http://") or src.startswith("https://"):
            continue
        resolved = resolve_image_path(src, base_dir)
        key = os.path.abspath(resolved)
        if key in seen:
            continue
        seen.add(key)
        out.append((alt, src, resolved))
    return out


def inline_format(text: str, wechat: bool) -> str:
    def code_sub(m: re.Match[str]) -> str:
        inner = m.group(1).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        if wechat:
            return f'<code style="{WECHAT_INLINE_CODE}">{inner}</code>'
        return f"<code>{inner}</code>"

    text = re.sub(r"`([^`]+)`", code_sub, text)
    text = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)", r"<em>\1</em>", text)

    def link_sub(m: re.Match[str]) -> str:
        label, href = m.group(1), m.group(2)
        if wechat:
            if href in label:
                return f'<a href="{href}">{label}</a>'
            return (
                f'<a href="{href}">{label}</a>'
                f'<span style="{WECHAT_LINK_NOTE}">（{href}）</span>'
            )
        return f'<a href="{href}" style="{GHOST_A}">{label}</a>'

    text = LINK_RE.sub(link_sub, text)
    return text


def parse_blocks(md_text: str) -> list[dict]:
    lines = md_text.split("\n")
    if lines and lines[0].strip() == "---":
        for j in range(1, len(lines)):
            if lines[j].strip() == "---":
                lines = lines[j + 1:]
                break
    if lines and lines[0].strip().startswith("# "):
        lines = lines[1:]

    blocks: list[dict] = []
    i = 0
    n = len(lines)

    def flush_list(kind: str, items: list[str]) -> None:
        if items:
            blocks.append({"type": kind, "items": items})

    while i < n:
        raw = lines[i]
        stripped = raw.strip()

        if stripped.startswith("```"):
            lang = stripped[3:].strip() or "text"
            code_lines: list[str] = []
            i += 1
            while i < n and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            blocks.append({"type": "code", "lang": lang, "text": "\n".join(code_lines)})
            i += 1
            continue

        if stripped.startswith("|") and stripped.endswith("|"):
            rows: list[list[str]] = []
            while i < n:
                row = lines[i].strip()
                if not (row.startswith("|") and row.endswith("|")):
                    break
                rows.append([c.strip() for c in row[1:-1].split("|")])
                i += 1
            if len(rows) >= 2:
                header = rows[0]
                body = rows[2:] if re.match(r"^[\s\-:]+$", "".join(rows[1])) else rows[1:]
                blocks.append({"type": "table", "header": header, "rows": body})
            continue

        if not stripped:
            i += 1
            continue

        if stripped.startswith(">"):
            quotes: list[str] = []
            while i < n and lines[i].strip().startswith(">"):
                q = lines[i].strip()
                q = q[1:]
                if q.startswith(" "):
                    q = q[1:]
                quotes.append(q)
                i += 1
            paras = [q for q in quotes if q]
            if paras:
                blocks.append({"type": "quote", "paras": paras})
            continue

        linked_img = LINKED_IMG_RE.fullmatch(stripped)
        if linked_img:
            blocks.append(
                {
                    "type": "image",
                    "alt": linked_img.group(1),
                    "src": linked_img.group(2),
                    "href": linked_img.group(3),
                }
            )
            i += 1
            continue

        img = IMG_RE.fullmatch(stripped)
        if img:
            blocks.append({"type": "image", "alt": img.group(1), "src": img.group(2)})
            i += 1
            continue

        if BARE_URL_RE.fullmatch(stripped):
            blocks.append({"type": "p", "text": f"[{stripped}]({stripped})"})
            i += 1
            continue

        if stripped.startswith("## "):
            blocks.append({"type": "h2", "text": stripped[3:]})
            i += 1
            continue
        if stripped.startswith("### "):
            blocks.append({"type": "h3", "text": stripped[4:]})
            i += 1
            continue
        if stripped == "---":
            blocks.append({"type": "hr"})
            i += 1
            continue

        if stripped.startswith("- "):
            items: list[str] = []
            while i < n and lines[i].strip().startswith("- "):
                items.append(lines[i].strip()[2:])
                i += 1
            flush_list("ul", items)
            continue

        ol_match = re.match(r"^\d+\.\s+", stripped)
        if ol_match:
            items = []
            while i < n and re.match(r"^\d+\.\s+", lines[i].strip()):
                items.append(re.sub(r"^\d+\.\s+", "", lines[i].strip()))
                i += 1
            flush_list("ol", items)
            continue

        blocks.append({"type": "p", "text": stripped})
        i += 1

    return blocks


def _escape_code(text: str) -> str:
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def render_wechat(blocks: list[dict], url_map: dict[str, str] | None, base_dir: str) -> str:
    html = [f'<div style="{WECHAT_CONTAINER}">']
    for block in blocks:
        kind = block["type"]
        if kind == "h2":
            html.append(f'<h2 style="{WECHAT_H2}">{inline_format(block["text"], True)}</h2>')
        elif kind == "h3":
            html.append(f'<h3 style="{WECHAT_H3}">{inline_format(block["text"], True)}</h3>')
        elif kind == "hr":
            html.append(f'<hr style="{WECHAT_HR}">')
        elif kind == "quote":
            paras = block["paras"]
            inner = []
            for idx, para in enumerate(paras):
                style = WECHAT_QUOTE_P_LAST if idx == len(paras) - 1 else WECHAT_QUOTE_P
                inner.append(f'<p style="{style}">{inline_format(para, True)}</p>')
            html.append(f'<blockquote style="{WECHAT_QUOTE}">{"".join(inner)}</blockquote>')
        elif kind == "ul":
            items = "".join(
                f'<li style="{WECHAT_LI}">{inline_format(item, True)}</li>' for item in block["items"]
            )
            html.append(f'<ul style="{WECHAT_UL}">{items}</ul>')
        elif kind == "ol":
            items = "".join(
                f'<li style="{WECHAT_LI}">{inline_format(item, True)}</li>' for item in block["items"]
            )
            html.append(f'<ol style="{WECHAT_UL}">{items}</ol>')
        elif kind == "image":
            src = _image_src(block["src"], url_map, base_dir)
            alt = block["alt"]
            href = block.get("href")
            img_tag = f'<img src="{src}" alt="{alt}" style="{WECHAT_IMG}">'
            html.append(f'<a href="{href}">{img_tag}</a>' if href else img_tag)
            cap = alt
            if href:
                cap = f'{alt} · {href}' if alt else href
            if cap:
                html.append(f'<p style="{WECHAT_CAPTION}">{cap}</p>')
        elif kind == "code":
            html.append(
                f'<pre style="{WECHAT_CODE}"><code>{_escape_code(block["text"])}</code></pre>'
            )
        elif kind == "table":
            parts = [f'<table style="{WECHAT_TABLE}"><thead><tr>']
            for th in block["header"]:
                parts.append(f'<th style="{WECHAT_TH}">{th}</th>')
            parts.append("</tr></thead><tbody>")
            for row in block["rows"]:
                parts.append("<tr>")
                for td in row:
                    parts.append(f'<td style="{WECHAT_TD}">{inline_format(td, True)}</td>')
                parts.append("</tr>")
            parts.append("</tbody></table>")
            html.append("".join(parts))
        elif kind == "p":
            html.append(f'<p style="{WECHAT_P}">{inline_format(block["text"], True)}</p>')
    html.append("</div>")
    return "\n".join(html)


def render_ghost(blocks: list[dict], url_map: dict[str, str] | None, base_dir: str) -> str:
    html: list[str] = []
    for block in blocks:
        kind = block["type"]
        if kind == "h2":
            html.append(f"<h2>{inline_format(block['text'], False)}</h2>")
        elif kind == "h3":
            html.append(f"<h3>{inline_format(block['text'], False)}</h3>")
        elif kind == "hr":
            html.append("<hr>")
        elif kind == "quote":
            inner = "".join(f"<p>{inline_format(para, False)}</p>" for para in block["paras"])
            html.append(f"<blockquote>{inner}</blockquote>")
        elif kind == "ul":
            items = "".join(f"<li>{inline_format(item, False)}</li>" for item in block["items"])
            html.append(f"<ul>{items}</ul>")
        elif kind == "ol":
            items = "".join(f"<li>{inline_format(item, False)}</li>" for item in block["items"])
            html.append(f"<ol>{items}</ol>")
        elif kind == "image":
            src = _image_src(block["src"], url_map, base_dir)
            alt = block["alt"]
            href = block.get("href")
            img_tag = f'<img src="{src}" alt="{alt}" class="kg-image">'
            if href:
                img_tag = f'<a href="{href}" style="{GHOST_A}">{img_tag}</a>'
            cap = f"<figcaption>{alt}</figcaption>" if alt else ""
            cls = "kg-card kg-image-card kg-card-hascaption" if alt else "kg-card kg-image-card"
            html.append(f'<figure class="{cls}">{img_tag}{cap}</figure>')
        elif kind == "code":
            html.append(
                f'<pre><code class="language-{block["lang"]}">{_escape_code(block["text"])}</code></pre>'
            )
        elif kind == "table":
            parts = ['<figure class="kg-card kg-table-card"><table><thead><tr>']
            for th in block["header"]:
                parts.append(f"<th>{th}</th>")
            parts.append("</tr></thead><tbody>")
            for row in block["rows"]:
                parts.append("<tr>")
                for td in row:
                    parts.append(f"<td>{inline_format(td, False)}</td>")
                parts.append("</tr>")
            parts.append("</tbody></table></figure>")
            html.append("".join(parts))
        elif kind == "p":
            html.append(f"<p>{inline_format(block['text'], False)}</p>")
    return "\n".join(html)


def _image_src(src: str, url_map: dict[str, str] | None, base_dir: str) -> str:
    if src.startswith("http://") or src.startswith("https://"):
        return src
    resolved = resolve_image_path(src, base_dir)
    abs_path = os.path.abspath(resolved)
    if url_map:
        if abs_path in url_map:
            return url_map[abs_path]
        if src in url_map:
            return url_map[src]
        if resolved in url_map:
            return url_map[resolved]
    return src


def md_to_wechat_html(md_text: str, base_dir: str = "", url_map: dict[str, str] | None = None) -> str:
    return render_wechat(parse_blocks(md_text), url_map, base_dir)


def md_to_ghost_html(md_text: str, base_dir: str = "", url_map: dict[str, str] | None = None) -> str:
    return render_ghost(parse_blocks(md_text), url_map, base_dir)


def ghost_dest_name(slug: str, local_path: str) -> str:
    return f"{slug}-{os.path.basename(local_path)}"


def build_ghost_url_map(images: list[tuple[str, str, str]], slug: str, cdn_prefix: str) -> dict[str, str]:
    mapping: dict[str, str] = {}
    prefix = cdn_prefix.rstrip("/") + "/"
    for _alt, src, resolved in images:
        dest = ghost_dest_name(slug, resolved)
        url = prefix + dest
        mapping[os.path.abspath(resolved)] = url
        mapping[src] = url
        mapping[resolved] = url
    return mapping


def write_image_manifest(images: list[tuple[str, str, str]], slug: str, path: str) -> None:
    with open(path, "w", encoding="utf-8") as f:
        for _alt, _src, resolved in images:
            if not os.path.exists(resolved):
                raise FileNotFoundError(f"image not found: {resolved}")
            f.write(f"{resolved}\t{ghost_dest_name(slug, resolved)}\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Convert article markdown to Ghost/WeChat HTML")
    parser.add_argument("target", choices=["ghost", "wechat", "manifest"])
    parser.add_argument("--input", required=True)
    parser.add_argument("--slug", default="")
    parser.add_argument("--title", default="")
    parser.add_argument("--excerpt", default="")
    parser.add_argument("--feature-image", default="")
    parser.add_argument("--cdn-prefix", default="https://blog.contextlm.top/content/images/2026/09/")
    parser.add_argument("--output", required=True)
    parser.add_argument("--manifest", default="")
    args = parser.parse_args()

    input_path = resolve_path(args.input)
    with open(input_path, "r", encoding="utf-8") as f:
        md_text = f.read()
    base_dir = os.path.dirname(os.path.abspath(input_path))
    images = collect_images(md_text, base_dir)

    if args.target == "manifest":
        write_image_manifest(images, args.slug, args.output)
        print(f"Wrote {len(images)} images to {args.output}")
        return

    url_map = build_ghost_url_map(images, args.slug, args.cdn_prefix) if args.slug else None
    if args.target == "ghost":
        html = md_to_ghost_html(md_text, base_dir, url_map)
        payload = {
            "slug": args.slug,
            "title": args.title,
            "excerpt": args.excerpt,
            "feature_image": args.feature_image,
            "html": html,
        }
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(payload, f, ensure_ascii=False, indent=2)
        print(f"Ghost payload written to {args.output}")
        return

    html = md_to_wechat_html(md_text, base_dir, url_map)
    with open(args.output, "w", encoding="utf-8") as f:
        f.write(html)
    print(f"WeChat HTML written to {args.output}")


if __name__ == "__main__":
    main()
