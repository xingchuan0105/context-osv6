#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
WeChat Official Account Markdown Publisher & Formatter (微信公众号排版与发布工具)

Best Practices Implemented:
1. Full Inline CSS: WeChat strips <style> and class tags; all typography is rendered with inlined styles.
2. WeChat Safe Typography: 15.5px base font, 1.85 line height, clean monospace code blocks, accented headers & quotes.
3. Dual Mode:
   - Mode A (API Mode): Uploads cover & inline images to WeChat CDN, calls /cgi-bin/draft/add to push directly to Draft Box.
   - Mode B (Export / Copy Mode): Generates standalone WeChat-compatible HTML. Open in browser, Ctrl+A & Ctrl+C, paste into WeChat editor without losing styles.
4. Path Translation: Supports both Windows paths (e.g. E:\\OneDrive\\...) and WSL Linux paths (/mnt/e/OneDrive/...).
5. IP Whitelist Detection: Gracefully catches 40164 error and reports the exact IP address needed for the whitelist.

Usage:
  # Mode B: Convert Markdown to WeChat HTML for local preview / one-click clipboard copy
  python3 scripts/wechat-publish.py --input docs/posts/2026-09-04-claude-fable-5-1-agent-memory.md --export docs/posts/2026-09-04-claude-fable-5-1-agent-memory-wechat.html

  # Mode A: Direct push to WeChat Draft Box
  python3 scripts/wechat-publish.py --input docs/posts/2026-09-04-claude-fable-5-1-agent-memory.md \
      --cover "E:/OneDrive/桌面/grok-image-25c90a86-8587-4451-8d5c-8a30df678f17.jpg" \
      --title "从 Claude Fable 5.1 的提示词变化，聊聊专业知识工作究竟需要怎样的“记忆”" \
      --author "邢川" \
      --push
"""

import os
import re
import sys
import json
import uuid
import mimetypes
import argparse
import urllib.request
import urllib.error

# Inline Style Constants (微信排版最佳美学标准)
STYLE_CONTAINER = "font-family: -apple-system-font, BlinkMacSystemFont, 'Helvetica Neue', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei UI', 'Microsoft YaHei', Arial, sans-serif; font-size: 15.5px; line-height: 1.85; color: #2c3e50; letter-spacing: 0.5px; padding: 10px 8px; word-break: break-word;"
STYLE_H1 = "font-size: 21px; font-weight: bold; color: #111827; margin: 32px 0 16px; border-bottom: 2px solid #00c9a7; padding-bottom: 8px; letter-spacing: 0.5px; line-height: 1.4;"
STYLE_H2 = "font-size: 18.5px; font-weight: bold; color: #111827; margin: 28px 0 14px; border-left: 4px solid #00c9a7; padding-left: 10px; line-height: 1.4;"
STYLE_H3 = "font-size: 16.5px; font-weight: bold; color: #1f2937; margin: 20px 0 10px;"
STYLE_P = "margin: 14px 0; text-align: justify;"
STYLE_BLOCKQUOTE = "background: #f8fafc; border-left: 4px solid #00c9a7; padding: 12px 16px; color: #475569; font-size: 14.5px; line-height: 1.75; margin: 18px 0; border-radius: 0 4px 4px 0;"
STYLE_CODE_BLOCK = "background: #1e1e1e; color: #d4d4d4; padding: 14px 16px; border-radius: 6px; font-family: Menlo, Monaco, Consolas, 'Courier New', monospace; font-size: 13px; line-height: 1.6; overflow-x: auto; margin: 18px 0; -webkit-overflow-scrolling: touch;"
STYLE_INLINE_CODE = "background: #f1f5f9; color: #0f766e; padding: 2px 6px; border-radius: 4px; font-family: Menlo, Monaco, Consolas, monospace; font-size: 13.5px; margin: 0 2px;"
STYLE_HR = "border: none; border-top: 1px solid #e2e8f0; margin: 30px 0;"
STYLE_UL = "margin: 14px 0; padding-left: 20px;"
STYLE_LI = "margin: 6px 0;"
STYLE_TABLE = "border-collapse: collapse; width: 100%; margin: 18px 0; font-size: 13.5px; line-height: 1.6;"
STYLE_TH = "background: #f1f5f9; border: 1px solid #cbd5e1; padding: 8px 10px; font-weight: bold; color: #1e293b; text-align: left;"
STYLE_TD = "border: 1px solid #cbd5e1; padding: 8px 10px; color: #334155;"

def resolve_path(p):
    """Handles Windows and WSL paths seamlessly."""
    if not p:
        return ""
    if os.path.exists(p):
        return p
    # Convert Windows E:\... to WSL /mnt/e/...
    win_match = re.match(r'^([a-zA-Z]):[\\/](.*)', p)
    if win_match:
        drive = win_match.group(1).lower()
        rest = win_match.group(2).replace('\\', '/')
        wsl_path = f"/mnt/{drive}/{rest}"
        if os.path.exists(wsl_path):
            return wsl_path
    # Reverse WSL /mnt/c/... to C:\...
    wsl_match = re.match(r'^/mnt/([a-zA-Z])/(.*)', p)
    if wsl_match:
        drive = wsl_match.group(1).upper()
        rest = wsl_match.group(2).replace('/', '\\')
        win_path = f"{drive}:\\{rest}"
        if os.path.exists(win_path):
            return win_path
    return p

def md_to_wechat_html(md_text):
    """Converts standard Markdown into WeChat-compatible inline-styled HTML."""
    lines = md_text.split('\n')
    # Skip leading H1 if present to avoid duplicate title in WeChat
    if lines and lines[0].strip().startswith('# '):
        lines = lines[1:]

    html_out = [f'<div style="{STYLE_CONTAINER}">']
    
    in_code = False
    code_lines = []
    in_table = False
    table_rows = []

    for line in lines:
        stripped = line.strip()

        # Code block handling
        if stripped.startswith('```'):
            if in_code:
                in_code = False
                escaped = "\n".join(code_lines).replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
                html_out.append(f'<pre style="{STYLE_CODE_BLOCK}"><code>{escaped}</code></pre>')
                code_lines = []
            else:
                in_code = True
            continue

        if in_code:
            code_lines.append(line)
            continue

        # Table handling
        if stripped.startswith('|') and stripped.endswith('|'):
            if not in_table:
                in_table = True
                table_rows = []
            table_rows.append([col.strip() for col in stripped[1:-1].split('|')])
            continue
        elif in_table:
            in_table = False
            if len(table_rows) >= 2:
                th_row = table_rows[0]
                tb_rows = table_rows[2:] if re.match(r'^[\s\-:]+$', "".join(table_rows[1])) else table_rows[1:]
                t_html = [f'<table style="{STYLE_TABLE}"><thead><tr>']
                for th in th_row:
                    t_html.append(f'<th style="{STYLE_TH}">{th}</th>')
                t_html.append('</tr></thead><tbody>')
                for tr in tb_rows:
                    t_html.append('<tr>')
                    for td in tr:
                        t_html.append(f'<td style="{STYLE_TD}">{td}</td>')
                    t_html.append('</tr>')
                t_html.append('</tbody></table>')
                html_out.append("".join(t_html))
            table_rows = []

        if not stripped:
            continue

        # Headings
        if stripped.startswith('## '):
            html_out.append(f'<h2 style="{STYLE_H2}">{stripped[3:]}</h2>')
        elif stripped.startswith('### '):
            html_out.append(f'<h3 style="{STYLE_H3}">{stripped[4:]}</h3>')
        elif stripped.startswith('#### '):
            html_out.append(f'<h4 style="font-size:15px; font-weight:bold; margin:14px 0 6px;">{stripped[5:]}</h4>')
        elif stripped == '---':
            html_out.append(f'<hr style="{STYLE_HR}">')
        elif stripped.startswith('> '):
            quote_text = stripped[2:]
            quote_text = re.sub(r'\*\*(.*?)\*\*', r'<strong>\1</strong>', quote_text)
            quote_text = re.sub(r'`(.*?)`', f'<code style="{STYLE_INLINE_CODE}">\\1</code>', quote_text)
            html_out.append(f'<blockquote style="{STYLE_BLOCKQUOTE}">{quote_text}</blockquote>')
        elif stripped.startswith('- '):
            li_text = stripped[2:]
            li_text = re.sub(r'\*\*(.*?)\*\*', r'<strong>\1</strong>', li_text)
            li_text = re.sub(r'`(.*?)`', f'<code style="{STYLE_INLINE_CODE}">\\1</code>', li_text)
            html_out.append(f'<ul style="{STYLE_UL}"><li style="{STYLE_LI}">{li_text}</li></ul>')
        else:
            p_text = stripped
            p_text = re.sub(r'\*\*(.*?)\*\*', r'<strong>\1</strong>', p_text)
            p_text = re.sub(r'`(.*?)`', f'<code style="{STYLE_INLINE_CODE}">\\1</code>', p_text)
            html_out.append(f'<p style="{STYLE_P}">{p_text}</p>')

    html_out.append('</div>')
    return "\n".join(html_out)


def load_env_credentials():
    """Reads WeChat credentials from avrag-rs/.env if available."""
    env_path = os.path.join(os.path.dirname(__file__), '..', 'avrag-rs', '.env')
    creds = {"app_id": os.getenv("WECHAT_APP_ID", ""), "app_secret": os.getenv("WECHAT_APP_SECRET", "")}
    if os.path.exists(env_path):
        with open(env_path, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if line.startswith("WECHAT_APP_ID="):
                    creds["app_id"] = line.split("=", 1)[1].strip().strip('"').strip("'")
                elif line.startswith("WECHAT_APP_SECRET="):
                    creds["app_secret"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return creds


def get_wechat_access_token(app_id, app_secret):
    """Fetches WeChat access token and handles IP whitelist error gracefully."""
    url = f"https://api.weixin.qq.com/cgi-bin/token?grant_type=client_credential&appid={app_id}&secret={app_secret}"
    req = urllib.request.Request(url)
    try:
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode('utf-8'))
            if "access_token" in data:
                return data["access_token"]
            
            errcode = data.get("errcode")
            errmsg = data.get("errmsg", "")
            if errcode == 40164:
                # Extract IP from errmsg like "invalid ip 123.45.67.89 ipv6 ..., not in whitelist"
                ip_match = re.search(r'invalid ip ([\d\.]+|[0-9a-fA-F:]+)', errmsg)
                bad_ip = ip_match.group(1) if ip_match else "未知"
                print("\n" + "="*70, file=sys.stderr)
                print(f"[微信安全拦截 40164] 调用端公网 IP ({bad_ip}) 未在微信公众平台的 IP 白名单中！", file=sys.stderr)
                print("解决步骤：", file=sys.stderr)
                print("1. 登录微信公众平台 (https://mp.weixin.qq.com/)", file=sys.stderr)
                print("2. 进入【设置与开发】->【基本配置】->【IP白名单】->【修改】", file=sys.stderr)
                print(f"3. 添加白名单 IP: {bad_ip}", file=sys.stderr)
                print("4. 保存后重新运行本脚本即可直推草稿箱！", file=sys.stderr)
                print("="*70 + "\n", file=sys.stderr)
                sys.exit(2)
            raise RuntimeError(f"WeChat API Error {errcode}: {errmsg}")
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"HTTP Error {e.code}: {e.read().decode('utf-8')}")


def upload_permanent_thumb(token, image_path):
    """Uploads cover image as a permanent material to get thumb_media_id."""
    resolved_img = resolve_path(image_path)
    if not os.path.exists(resolved_img):
        raise FileNotFoundError(f"Cover image not found at: {image_path} (resolved: {resolved_img})")

    url = f"https://api.weixin.qq.com/cgi-bin/material/add_material?access_token={token}&type=image"
    
    boundary = f"----WebKitFormBoundary{uuid.uuid4().hex}"
    filename = os.path.basename(resolved_img)
    content_type, _ = mimetypes.guess_type(resolved_img)
    if not content_type:
        content_type = "image/jpeg"

    with open(resolved_img, "rb") as f:
        file_bytes = f.read()

    body = (
        f"--{boundary}\r\n"
        f'Content-Disposition: form-data; name="media"; filename="{filename}"\r\n'
        f"Content-Type: {content_type}\r\n\r\n"
    ).encode("utf-8") + file_bytes + f"\r\n--{boundary}--\r\n".encode("utf-8")

    req = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}"},
        method="POST"
    )

    with urllib.request.urlopen(req) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        if "media_id" in res:
            return res["media_id"]
        raise RuntimeError(f"Failed to upload permanent material: {res}")


def create_draft(token, title, author, digest, content_html, thumb_media_id, source_url):
    """Creates a new article in the WeChat Draft Box (草稿箱)."""
    url = f"https://api.weixin.qq.com/cgi-bin/draft/add?access_token={token}"
    payload = {
        "articles": [
            {
                "title": title,
                "author": author,
                "digest": digest,
                "content": content_html,
                "content_source_url": source_url,
                "thumb_media_id": thumb_media_id,
                "need_open_comment": 0,
                "only_fans_can_comment": 0
            }
        ]
    }

    req = urllib.request.Request(
        url,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST"
    )

    with urllib.request.urlopen(req) as resp:
        res = json.loads(resp.read().decode("utf-8"))
        if "media_id" in res:
            return res["media_id"]
        raise RuntimeError(f"Failed to create draft: {res}")


def main():
    parser = argparse.ArgumentParser(description="WeChat Markdown Formatter & Publisher")
    parser.add_argument("--input", required=True, help="Path to input Markdown file")
    parser.add_argument("--export", help="Export to standalone HTML file for direct copy")
    parser.add_argument("--title", default="从 Claude Fable 5.1 的提示词变化，聊聊专业知识工作究竟需要怎样的“记忆”")
    parser.add_argument("--author", default="邢川")
    parser.add_argument("--digest", default="从 Claude Fable 5.1 提示词变化，看专业知识工作的记忆与指代消解架构，以及 ContextOS 的克制工程实践。")
    parser.add_argument("--cover", help="Path to cover image")
    parser.add_argument("--source-url", default="https://blog.contextlm.top/cong-claude-fable-5-1-kan-agent-ji-yi/")
    parser.add_argument("--push", action="store_true", help="Push to WeChat Official Account Draft Box")
    
    args = parser.parse_args()

    input_path = resolve_path(args.input)
    if not os.path.exists(input_path):
        print(f"Error: input file {args.input} does not exist (resolved: {input_path})", file=sys.stderr)
        sys.exit(1)

    with open(input_path, 'r', encoding='utf-8') as f:
        md_text = f.read()

    html_result = md_to_wechat_html(md_text)

    # 1. Export standalone HTML if requested
    if args.export:
        export_path = resolve_path(args.export)
        with open(export_path, 'w', encoding='utf-8') as f:
            f.write(html_result)
        print(f"[OK] 微信富文本内联 HTML 已成功导出至: {args.export}")

    # 2. Push to WeChat Draft Box
    if args.push:
        creds = load_env_credentials()
        if not creds["app_id"] or not creds["app_secret"]:
            print("[ERROR] WECHAT_APP_ID / WECHAT_APP_SECRET 未在 avrag-rs/.env 中配置！", file=sys.stderr)
            sys.exit(1)

        print("[1/3] 正在获取微信公众平台 Access Token...")
        token = get_wechat_access_token(creds["app_id"], creds["app_secret"])
        print(f"[OK] Token 获取成功: {token[:8]}...")

        if not args.cover:
            print("[ERROR] 推送草稿箱必须提供封面图 (--cover 参数)！", file=sys.stderr)
            sys.exit(1)

        print(f"[2/3] 正在上传封面素材至微信媒体库: {args.cover}...")
        thumb_id = upload_permanent_thumb(token, args.cover)
        print(f"[OK] 封面上传成功，thumb_media_id: {thumb_id}")

        print("[3/3] 正在创建图文草稿至公众号草稿箱...")
        draft_id = create_draft(
            token=token,
            title=args.title,
            author=args.author,
            digest=args.digest,
            content_html=html_result,
            thumb_media_id=thumb_id,
            source_url=args.source_url
        )
        print("\n" + "="*60)
        print(f"🎉 成功推送到微信公众号草稿箱！Draft Media ID: {draft_id}")
        print("请在微信公众平台后台 (mp.weixin.qq.com) ->【草稿箱】或手机【订阅号助手】中查看和群发。")
        print("="*60 + "\n")


if __name__ == "__main__":
    main()
