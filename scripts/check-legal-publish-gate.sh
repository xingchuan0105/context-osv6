#!/usr/bin/env bash
# check-legal-publish-gate.sh
# 法律文档发布门控：检查 ToS/Privacy 的 frontmatter status 是否为 published。
# 设计文档 P0-CNT-2 要求："ToS/Privacy status: published"。
# 在 CI 生产部署流程中调用此脚本，status 为 draft 时阻断部署。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONTENT_DIR="$REPO_ROOT/frontend_next/content/legal/zh-CN"
EXIT_CODE=0

check_status() {
  local file="$1"
  local label="$2"
  if [ ! -f "$file" ]; then
    echo "❌ $label: 文件不存在 ($file)"
    EXIT_CODE=1
    return
  fi
  local status
  status=$(grep -m1 '^status:' "$file" | sed 's/status:\s*//' | tr -d ' "')
  if [ "$status" = "published" ]; then
    echo "✅ $label: status=$status"
  else
    echo "❌ $label: status=$status (需要 published 才能上线)"
    EXIT_CODE=1
  fi
}

echo "=== 法律文档发布门控检查 ==="
echo ""
check_status "$CONTENT_DIR/terms.mdx" "用户服务协议 (ToS)"
check_status "$CONTENT_DIR/privacy.mdx" "隐私政策 (Privacy)"
echo ""

if [ $EXIT_CODE -ne 0 ]; then
  echo "⛔ 法律文档未达到发布状态。"
  echo "   请完成法务审阅后，将 frontmatter 中的 status 改为 published。"
  echo "   设计文档: docs/legal-compliance-pages-design-2026-06-13.md §9.3.2 P0-CNT-2"
fi

exit $EXIT_CODE
