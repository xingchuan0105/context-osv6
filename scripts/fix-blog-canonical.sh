#!/usr/bin/env bash
# One-off GEO fix for blog.contextlm.top (Ghost on VPS):
#   1. Clear WeChat canonical_url on all posts (self-canonical恢复)
#   2. Demote imported leading <h1> to <h2> (theme already renders title H1)
#   3. Append a comparison table to the Palantir article (comparison-completeness)
# Backs up ghost.db first; stops/starts the ghost-blog container around the edit.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/avrag-rs/.env"
[[ -f "$ENV_FILE" ]] && { set -a; source "$ENV_FILE"; set +a; }
: "${VPS_MAIN_HOST:?set VPS_MAIN_HOST in avrag-rs/.env}"
: "${VPS_MAIN_USER:?set VPS_MAIN_USER}"
: "${VPS_MAIN_PASSWORD:?set VPS_MAIN_PASSWORD}"

SSH=(sshpass -p "$VPS_MAIN_PASSWORD" ssh -o StrictHostKeyChecking=no "${VPS_MAIN_USER}@${VPS_MAIN_HOST}")

"${SSH[@]}" 'python3 - <<PYEOF
import re, sqlite3, shutil, subprocess, time

DB = "/data/ghost/content/data/ghost.db"
subprocess.run(["docker", "stop", "ghost-blog"], check=True)
try:
    shutil.copy(DB, DB + ".bak-geofix-" + time.strftime("%Y%m%d-%H%M%S"))
    con = sqlite3.connect(DB)
    cur = con.cursor()

    # 1) WeChat canonical -> NULL (self-canonical)
    cur.execute("SELECT count(*) FROM posts WHERE type=\"post\" AND canonical_url LIKE \"%mp.weixin.qq.com%\"")
    n_canon = cur.fetchone()[0]
    cur.execute("UPDATE posts SET canonical_url = NULL WHERE canonical_url LIKE \"%mp.weixin.qq.com%\"")

    # 2) Leading <h1> in post html -> <h2> (avoid double H1 with theme title)
    cur.execute("SELECT id, html FROM posts WHERE type=\"post\" AND html LIKE \"<h1%\"")
    n_h1 = 0
    for pid, html in cur.fetchall():
        fixed = re.sub(r"^<h1([^>]*)>(.*?)</h1>", r"<h2\1>\2</h2>", html, count=1, flags=re.S)
        if fixed != html:
            cur.execute("UPDATE posts SET html = ? WHERE id = ?", (fixed, pid))
            n_h1 += 1

    # 3) Palantir article: append comparison table (derived from the article itself)
    TABLE = (
        "<figure class=\"kg-card kg-table-card\"><table>"
        "<thead><tr><th>Dimension</th><th>Mainstream SaaS / CN software</th><th>Palantir</th></tr></thead>"
        "<tbody>"
        "<tr><td>Target problem</td><td>Low-hanging fruit; consumer apps (ads, delivery, short video)</td><td>Hard problems tied to national/enterprise survival (counter-terror, war logistics, supply chain)</td></tr>"
        "<tr><td>Delivery logic</td><td>Throw software over the wall; implementation outsourced</td><td>FDE internalizes the pain; accountable for outcomes</td></tr>"
        "<tr><td>Data architecture</td><td>Data lakes; read-only dashboards</td><td>Ontology semantic layer + decision write-back (OODA loop)</td></tr>"
        "<tr><td>Stance</td><td>Technologically neutral, fence-sitting</td><td>Explicitly sided with the West and allies; trust moat</td></tr>"
        "<tr><td>Governance</td><td>Capital-market short-termism</td><td>Class F shares shield founder control; 20-year long-termism</td></tr>"
        "<tr><td>Financial result (2025 Q3)</td><td>High growth rarely meets high margin</td><td>Revenue +63% YoY, 40% net margin, Rule-of-40 &gt; 110</td></tr>"
        "</tbody></table>"
        "<figcaption>Appendix: Palantir vs mainstream software models (summary of this article)</figcaption></figure>"
    )
    cur.execute("SELECT html FROM posts WHERE slug=\"zhe-xue-shi-long-duan-wei-he-palantirnan-yi-fu-zhi\"")
    row = cur.fetchone()
    if row and "Palantir vs mainstream software models" not in row[0]:
        cur.execute(
            "UPDATE posts SET html = html || ? WHERE slug=\"zhe-xue-shi-long-duan-wei-he-palantirnan-yi-fu-zhi\"",
            (TABLE,),
        )
        print("palantir table: appended")
    else:
        print("palantir table: already present or post missing")

    con.commit()
    con.close()
    print(f"canonical cleared: {n_canon}")
    print(f"leading h1 demoted: {n_h1}")
finally:
    subprocess.run(["docker", "start", "ghost-blog"], check=True)
PYEOF
sleep 3
curl -sS -m 8 -o /dev/null -w "blog_home:%{http_code}\n" https://blog.contextlm.top/ || true
curl -sS -m 8 https://blog.contextlm.top/zhe-xue-shi-long-duan-wei-he-palantirnan-yi-fu-zhi/ | grep -o "canonical[^>]*" | head -1 || true
'
