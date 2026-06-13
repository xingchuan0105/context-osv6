#!/usr/bin/env bash
# check-legal-publish-gate.sh
# 法律文档发布门控：
# 1. 检查 ToS/Privacy frontmatter status 是否为 published（P0-CNT-2）
# 2. 跟踪 legal_review 字段，提醒法务审阅
# 设计文档 §9.3.2 P0-CNT-2 + §9.11 签 off 清单
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONTENT_DIR="$REPO_ROOT/frontend_next/content/legal/zh-CN"
EXIT_CODE=0

check_doc() {
  local file="$1"
  local label="$2"
  if [ ! -f "$file" ]; then
    echo "❌ $label: 文件不存在 ($file)"
    EXIT_CODE=1
    return
  fi
  local status
  status=$(grep -m1 '^status:' "$file" | sed 's/status:\s*//' | tr -d ' "')
  local legal_review
  legal_review=$(grep -m1 '^legal_review:' "$file" | sed 's/legal_review:\s*//' | tr -d ' "')
  if [ "$status" = "published" ]; then
    echo "✅ $label: status=$status"
    if [ "$legal_review" = "approved" ]; then
      echo "   ✅ 法务审阅: approved"
    else
      echo "   ⚠️  法务审阅: ${legal_review:-未填写}（§9.11 签 off 待法务完成）"
    fi
  else
    echo "❌ $label: status=$status (需要 published 才能上线)"
    EXIT_CODE=1
  fi
}

echo "=== 法律文档发布门控检查 ==="
echo "    设计文档: avrag-rs/docs/legal-compliance-pages-design-2026-06-13.md §9.3.2 P0-CNT-2"
echo ""
check_doc "$CONTENT_DIR/terms.mdx" "用户服务协议 (ToS)"
check_doc "$CONTENT_DIR/privacy.mdx" "隐私政策 (Privacy)"
echo ""

if [ $EXIT_CODE -ne 0 ]; then
  echo "⛔ 文档 status 未达 published。"
  echo "   设计文档: avrag-rs/docs/legal-compliance-pages-design-2026-06-13.md §9.3.2 P0-CNT-2"
fi

exit $EXIT_CODE
