#!/usr/bin/env bash
# Update meta_description and custom_excerpt for Palantir article in Ghost on VPS
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
[[ -f "$ENV_FILE" ]] && { set -a; source "$ENV_FILE"; set +a; }
: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST in avrag-rs/.env}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")

"${SSH[@]}" 'python3 - <<PYEOF
import sqlite3, shutil, subprocess, time

DB = "/data/ghost/content/data/ghost.db"
DESC = "深度解析 Palantir 的商业与技术护城河：从 FDE 交付逻辑、Ontology 语义本体架构、决策回写闭环到地缘战略信任，剖析为何其哲学式垄断难以被传统通用 SaaS 复制。"
SLUG = "zhe-xue-shi-long-duan-wei-he-palantirnan-yi-fu-zhi"

subprocess.run(["docker", "stop", "ghost-blog"], check=True)
try:
    shutil.copy(DB, DB + ".bak-metafix-" + time.strftime("%Y%m%d-%H%M%S"))
    con = sqlite3.connect(DB)
    cur = con.cursor()
    cur.execute("SELECT id, custom_excerpt FROM posts WHERE slug=?", (SLUG,))
    row = cur.fetchone()
    print("before:", row)
    cur.execute("UPDATE posts SET custom_excerpt = ? WHERE slug = ?", (DESC, SLUG))
    con.commit()
    cur.execute("SELECT id, custom_excerpt FROM posts WHERE slug=?", (SLUG,))
    print("after:", cur.fetchone())
    con.close()
finally:
    subprocess.run(["docker", "start", "ghost-blog"], check=True)
PYEOF
sleep 3
curl -sS -m 8 "https://blog.contextlm.top/zhe-xue-shi-long-duan-wei-he-palantirnan-yi-fu-zhi/" | grep -o "<meta name=\"description\" content=\"[^\"]*\"" | head -1 || true
'
