#!/usr/bin/env bash
# Deploy / Publish article to Ghost Blog on VPS (blog.contextlm.top)
# Usage:
#   bash scripts/deploy-blog-post.sh docs/articles/2026-09-13-后就业时代-publish.json
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
[[ -f "$ENV_FILE" ]] && { set -a; source "$ENV_FILE"; set +a; }

: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST in avrag-rs/.env}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

CONFIG="${1:?usage: bash scripts/deploy-blog-post.sh path/to-publish.json}"
if [[ "$CONFIG" != /* ]]; then
  CONFIG="$ROOT/$CONFIG"
fi
[[ -f "$CONFIG" ]] || { echo "publish json not found: $CONFIG" >&2; exit 1; }

eval "$(python3 - "$CONFIG" "$ROOT" << 'PY'
import json, os, sys, shlex
cfg_path, root = sys.argv[1], sys.argv[2]
with open(cfg_path, encoding="utf-8") as f:
    cfg = json.load(f)

def abspath(p):
    if not p:
        return ""
    if os.path.isabs(p):
        return p
    return os.path.normpath(os.path.join(root, p))

cover_name = f'{cfg["slug"]}-{os.path.basename(cfg["cover"])}'
assignments = {
    "SLUG": cfg["slug"],
    "TITLE": cfg["title"],
    "EXCERPT": cfg["excerpt"],
    "POST_MD": abspath(cfg["input"]),
    "COVER_SRC": abspath(cfg["cover"]),
    "COVER_DEST": cover_name,
    "FEATURE_IMAGE_URL": "https://blog.contextlm.top/content/images/2026/09/" + cover_name,
}
for key, value in assignments.items():
    print(f"{key}={shlex.quote(value)}")
PY
)"

CDN_DIR="/data/ghost/content/images/2026/09"
FEATURE_IMAGE_URL="https://blog.contextlm.top/content/images/2026/09/${COVER_DEST}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")
SCP=(sshpass -p "$VPS_MAIN_PASSWORD" scp -o StrictHostKeyChecking=no)

MANIFEST="/tmp/${SLUG}-image-manifest.tsv"
PAYLOAD="/tmp/${SLUG}-post-payload.json"

echo "=== 1. Preparing image manifest ==="
python3 "$ROOT/scripts/article_html.py" manifest \
  --input "$POST_MD" \
  --slug "$SLUG" \
  --output "$MANIFEST"

echo "=== 2. Uploading images to Ghost ==="
"${SSH[@]}" "mkdir -p ${CDN_DIR}"
if [[ -f "$COVER_SRC" ]]; then
  "${SCP[@]}" "$COVER_SRC" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:${CDN_DIR}/${COVER_DEST}"
  echo "Cover uploaded: ${COVER_DEST}"
else
  echo "Warning: cover not found at $COVER_SRC" >&2
fi

while IFS=$'\t' read -r local_path dest_name; do
  [[ -z "${local_path:-}" ]] && continue
  "${SCP[@]}" "$local_path" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:${CDN_DIR}/${dest_name}"
  echo "Uploaded ${dest_name}"
done < "$MANIFEST"

echo "=== 3. Converting Markdown to Ghost HTML ==="
python3 "$ROOT/scripts/article_html.py" ghost \
  --input "$POST_MD" \
  --slug "$SLUG" \
  --title "$TITLE" \
  --excerpt "$EXCERPT" \
  --feature-image "$FEATURE_IMAGE_URL" \
  --cdn-prefix "https://blog.contextlm.top/content/images/2026/09/" \
  --output "$PAYLOAD"

"${SCP[@]}" "$PAYLOAD" "${VPS_MAIN_USER}@${VPS_MAIN_HOST}:/tmp/post_payload.json"

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
        post_id = secrets.token_hex(12)
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

echo "=== 4. Verifying Live Post ==="
sleep 4
HTTP_CODE="$("${SSH[@]}" curl -sS -o /dev/null -w "%{http_code}" "https://blog.contextlm.top/${SLUG}/" || true)"
echo "Live URL test: https://blog.contextlm.top/${SLUG}/ (HTTP ${HTTP_CODE})"
echo "Publishing complete!"
