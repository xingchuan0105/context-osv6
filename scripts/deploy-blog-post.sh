#!/usr/bin/env bash
# Deploy / Publish article to Ghost Blog on VPS (blog.contextlm.top)
# Usage:
#   bash scripts/deploy-blog-post.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
[[ -f "$ENV_FILE" ]] && { set -a; source "$ENV_FILE"; set +a; }

: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST in avrag-rs/.env}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

SLUG="cong-claude-fable-5-1-kan-agent-ji-yi"
TITLE="从 Claude Fable 5.1 的提示词变化，聊聊专业知识工作究竟需要怎样的“记忆”"
EXCERPT="这两天 Anthropic 最新版本 Claude Fable 5.1 内置提示词被逆向提取，其中新增篇幅最大的是持久化文件系统记忆。本文深入剖析：为什么专业知识工作的核心是“指代消解”？为什么全量记忆反而污染上下文？ContextOS 如何通过 Workspace-Session 架构与轻量自治记忆，在多轮消解评测中交出 100% 满分成绩单。"

POST_MD="$ROOT/docs/posts/2026-09-04-claude-fable-5-1-agent-memory.md"

IMAGE_SRC="/mnt/e/OneDrive/桌面/grok-image-25c90a86-8587-4451-8d5c-8a30df678f17.jpg"
if [[ ! -f "$IMAGE_SRC" ]]; then
  IMAGE_SRC="/mnt/c/Users/xingc/Desktop/grok-image-25c90a86-8587-4451-8d5c-8a30df678f17.jpg"
fi

IMAGE_DEST_NAME="grok-memory-cube-25c90a86.jpg"
FEATURE_IMAGE_URL="https://blog.contextlm.top/content/images/2026/09/${IMAGE_DEST_NAME}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")
SCP=(sshpass -p "$VPS_MAIN_PASSWORD" scp -o StrictHostKeyChecking=no)

echo "=== 1. Uploading theme image to VPS Ghost images directory ==="
"${SSH[@]}" "mkdir -p /data/ghost/content/images/2026/09"
if [[ -f "$IMAGE_SRC" ]]; then
  "${SCP[@]}" "$IMAGE_SRC" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/data/ghost/content/images/2026/09/${IMAGE_DEST_NAME}"
  echo "Image uploaded successfully to /data/ghost/content/images/2026/09/${IMAGE_DEST_NAME}"
else
  echo "Warning: Image source not found locally at $IMAGE_SRC, skipping image scp"
fi

echo "=== 2. Converting Markdown to HTML and Preparing JSON Payload ==="
python3 - << 'PYLOCAL'
import json, sys, re

def md_to_html(md_text):
    lines = md_text.split('\n')
    # strip leading # title if present because Ghost renders post title separately
    if lines and lines[0].startswith('# '):
        lines = lines[1:]
    
    html = []
    in_code = False
    code_lang = ""
    code_lines = []
    in_table = False
    table_rows = []
    in_list = False

    for line in lines:
        if line.strip().startswith('```'):
            if in_code:
                in_code = False
                escaped = "\n".join(code_lines).replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
                html.append(f'<pre><code class="language-{code_lang}">{escaped}</code></pre>')
                code_lines = []
            else:
                in_code = True
                code_lang = line.strip()[3:].strip() or "text"
            continue

        if in_code:
            code_lines.append(line)
            continue

        # Table detection
        if line.strip().startswith('|') and line.strip().endswith('|'):
            if not in_table:
                in_table = True
                table_rows = []
            table_rows.append([c.strip() for c in line.strip()[1:-1].split('|')])
            continue
        elif in_table:
            in_table = False
            # Render table
            if len(table_rows) >= 2:
                th_row = table_rows[0]
                tb_rows = table_rows[2:] if re.match(r'^[\s\-:]+$', "".join(table_rows[1])) else table_rows[1:]
                t_html = ['<figure class="kg-card kg-table-card"><table><thead><tr>']
                for th in th_row:
                    t_html.append(f'<th>{th}</th>')
                t_html.append('</tr></thead><tbody>')
                for tr in tb_rows:
                    t_html.append('<tr>')
                    for td in tr:
                        t_html.append(f'<td>{td}</td>')
                    t_html.append('</tr>')
                t_html.append('</tbody></table></figure>')
                html.append("".join(t_html))
            table_rows = []

        stripped = line.strip()
        if not stripped:
            if in_list:
                in_list = False
                html.append('</ul>')
            continue

        # Headings & Blocks
        if stripped.startswith('## '):
            if in_list: in_list = False; html.append('</ul>')
            html.append(f'<h2>{stripped[3:]}</h2>')
        elif stripped.startswith('### '):
            if in_list: in_list = False; html.append('</ul>')
            html.append(f'<h3>{stripped[4:]}</h3>')
        elif stripped.startswith('#### '):
            if in_list: in_list = False; html.append('</ul>')
            html.append(f'<h4>{stripped[5:]}</h4>')
        elif stripped == '---':
            if in_list: in_list = False; html.append('</ul>')
            html.append('<hr>')
        elif stripped.startswith('> '):
            if in_list: in_list = False; html.append('</ul>')
            quote = stripped[2:]
            quote = re.sub(r'\*\*(.*?)\*\*', r'<strong>\1</strong>', quote)
            quote = re.sub(r'`(.*?)`', r'<code>\1</code>', quote)
            html.append(f'<blockquote><p>{quote}</p></blockquote>')
        elif stripped.startswith('- '):
            if not in_list:
                in_list = True
                html.append('<ul>')
            li = stripped[2:]
            li = re.sub(r'\*\*(.*?)\*\*', r'<strong>\1</strong>', li)
            li = re.sub(r'`(.*?)`', r'<code>\1</code>', li)
            li = re.sub(r'\[(.*?)\]\((.*?)\)', r'<a href="\2">\1</a>', li)
            html.append(f'<li>{li}</li>')
        else:
            if in_list:
                in_list = False
                html.append('</ul>')
            p = stripped
            p = re.sub(r'\*\*(.*?)\*\*', r'<strong>\1</strong>', p)
            p = re.sub(r'`(.*?)`', r'<code>\1</code>', p)
            p = re.sub(r'\[(.*?)\]\((.*?)\)', r'<a href="\2">\1</a>', p)
            html.append(f'<p>{p}</p>')

    if in_list:
        html.append('</ul>')
    if in_table and len(table_rows) >= 2:
        th_row = table_rows[0]
        tb_rows = table_rows[2:] if re.match(r'^[\s\-:]+$', "".join(table_rows[1])) else table_rows[1:]
        t_html = ['<figure class="kg-card kg-table-card"><table><thead><tr>']
        for th in th_row:
            t_html.append(f'<th>{th}</th>')
        t_html.append('</tr></thead><tbody>')
        for tr in tb_rows:
            t_html.append('<tr>')
            for td in tr:
                t_html.append(f'<td>{td}</td>')
            t_html.append('</tr>')
        t_html.append('</tbody></table></figure>')
        html.append("".join(t_html))

    return "\n".join(html)

with open('docs/posts/2026-09-04-claude-fable-5-1-agent-memory.md', 'r', encoding='utf-8') as f:
    raw_md = f.read()

rendered_html = md_to_html(raw_md)

payload = {
    "slug": "cong-claude-fable-5-1-kan-agent-ji-yi",
    "title": "从 Claude Fable 5.1 的提示词变化，聊聊专业知识工作究竟需要怎样的“记忆”",
    "excerpt": "这两天 Anthropic 最新版本 Claude Fable 5.1 内置提示词被逆向提取，其中新增篇幅最大的是持久化文件系统记忆。本文深入剖析：为什么专业知识工作的核心是“指代消解”？为什么全量记忆反而污染上下文？ContextOS 如何通过 Workspace-Session 架构与轻量自治记忆，在多轮消解评测中交出 100% 满分成绩单。",
    "feature_image": "https://blog.contextlm.top/content/images/2026/09/grok-memory-cube-25c90a86.jpg",
    "html": rendered_html
}

with open('/tmp/post_payload.json', 'w', encoding='utf-8') as f:
    json.dump(payload, f, ensure_ascii=False, indent=2)

print("Payload prepared at /tmp/post_payload.json")
PYLOCAL

"${SCP[@]}" /tmp/post_payload.json "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/post_payload.json"

"${SSH[@]}" python3 - << 'PYREMOTE'
import json, sqlite3, shutil, subprocess, time, uuid, secrets

with open('/tmp/post_payload.json', 'r', encoding='utf-8') as f:
    data = json.load(f)

slug = data["slug"]
title = data["title"]
excerpt = data["excerpt"]
feature_image = data["feature_image"]
html_content = data["html"]

DB = "/data/ghost/content/data/ghost.db"
print("Stopping ghost-blog container...")
subprocess.run(["docker", "stop", "ghost-blog"], check=True)

try:
    backup_db = DB + ".bak-publish-" + time.strftime("%Y%m%d-%H%M%S")
    shutil.copy(DB, backup_db)
    print(f"Backed up ghost.db to {backup_db}")

    con = sqlite3.connect(DB)
    cur = con.cursor()

    # Find author
    cur.execute("SELECT id FROM users LIMIT 1")
    author_row = cur.fetchone()
    author_id = author_row[0] if author_row else "1"

    now_iso = time.strftime("%Y-%m-%d %H:%M:%S")

    cur.execute("SELECT id FROM posts WHERE slug = ?", (slug,))
    existing = cur.fetchone()

    if existing:
        post_id = existing[0]
        print(f"Updating existing post {post_id} (slug={slug})...")
        cur.execute("""
            UPDATE posts 
            SET title = ?, html = ?, custom_excerpt = ?, feature_image = ?, updated_at = ?, status = 'published'
            WHERE id = ?
        """, (title, html_content, excerpt, feature_image, now_iso, post_id))
    else:
        post_id = secrets.token_hex(12)  # 24-character hex cuid-like id
        post_uuid = str(uuid.uuid4())
        print(f"Inserting new post {post_id} (slug={slug})...")
        cur.execute("""
            INSERT INTO posts (
                id, uuid, title, slug, html, comment_id, feature_image, featured,
                type, status, visibility, email_recipient_filter,
                created_at, created_by, updated_at, updated_by, published_at, published_by,
                custom_excerpt
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, 0,
                'post', 'published', 'public', 'none',
                ?, ?, ?, ?, ?, ?,
                ?
            )
        """, (
            post_id, post_uuid, title, slug, html_content, post_id, feature_image,
            now_iso, author_id, now_iso, author_id, now_iso, author_id,
            excerpt
        ))

        cur.execute("INSERT OR REPLACE INTO posts_authors (id, post_id, author_id, sort_order) VALUES (?, ?, ?, 0)",
                    (secrets.token_hex(12), post_id, author_id))

    con.commit()
    con.close()
    print("Database update committed successfully.")
finally:
    print("Starting ghost-blog container...")
    subprocess.run(["docker", "start", "ghost-blog"], check=True)

PYREMOTE

echo "=== 3. Verifying Live Post ==="
sleep 4
HTTP_CODE="$("${SSH[@]}" curl -sS -o /dev/null -w "%{http_code}" "https://blog.contextlm.top/${SLUG}/" || true)"
echo "Live URL test: https://blog.contextlm.top/${SLUG}/ (HTTP ${HTTP_CODE})"
echo "Publishing complete!"
