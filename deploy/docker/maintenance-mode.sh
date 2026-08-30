#!/usr/bin/env bash
# Maintenance mode: flip nginx between full proxying and a static 503 page.
# Usage (on the VPS):
#   maintenance-mode.sh on     -> everything except /health returns 503 + Retry-After
#   maintenance-mode.sh off    -> remove the maintenance include (normal proxying)
# Keeps localhost health checks green (deploy scripts still verify health).
set -euo pipefail

die() { echo "maintenance-mode: $*" >&2; exit 1; }

STATE_ON='/etc/avrag-rs/maintenance.on'
SNIPPET='/etc/nginx/snippets/avrag-maintenance.conf'
VHOST=$(ls /etc/nginx/sites-available/app-contextlm.conf /etc/nginx/conf.d/app-contextlm.conf 2>/dev/null | head -1 || true)
[[ -n "$VHOST" ]] || die "app-contextlm.conf not found under sites-available or conf.d"

write_snippet() {
  cat > /var/www/maintenance/index.html <<'HTML'
<!doctype html>
<html lang="zh-CN"><meta charset="utf-8"><title>系统维护中</title>
<body style="font-family:system-ui;max-width:32rem;margin:12vh auto;text-align:center;color:#333">
<h1>系统维护中</h1>
<p>ContextLM 正在进行计划内维护，预计数分钟内恢复。请稍后刷新。</p>
</body></html>
HTML
  cat > "$SNIPPET" <<'NGINX'
# Maintenance gate — injected before proxy locations by maintenance-mode.sh on.
set $maintenance 0;
if (-f /etc/avrag-rs/maintenance.on) {
    set $maintenance 1;
}
if ($request_uri = /health) {
    set $maintenance 0;
}
if ($maintenance = 1) {
    return 503;
}
NGINX
}

case "${1:-}" in
  on)
    mkdir -p /var/www/maintenance
    write_snippet
    touch "$STATE_ON"
    echo "maintenance-mode: ON (503 for public traffic, /health exempt)"
    ;;
  off)
    rm -f "$STATE_ON"
    echo "maintenance-mode: OFF"
    ;;
  *)
    die "usage: maintenance-mode.sh {on|off}"
    ;;
esac

# Wire the snippet into the 443 server block once (idempotent marker).
if ! grep -q 'snippets/avrag-maintenance.conf' "$VHOST"; then
  # Insert right after the scanner-block include (evaluated first for all locations).
  sed -i '/include \/etc\/nginx\/snippets\/scanner-block.conf;/a\    include /etc/nginx/snippets\/avrag-maintenance.conf;' "$VHOST"
fi

nginx -t || die "nginx config test failed"
systemctl reload nginx

if [[ -f /etc/avrag-rs/maintenance.on ]]; then
  code=$(curl -s -o /dev/null -w '%{http_code}' -m 5 https://app.contextlm.top/health || true)
  [[ "$code" == "200" ]] || echo "maintenance-mode: WARN public /health returned ${code:-none}"
  pub=$(curl -s -o /dev/null -w '%{http_code}' -m 5 https://app.contextlm.top/ || true)
  echo "maintenance-mode: public / -> ${pub:-?} (expect 503)"
fi