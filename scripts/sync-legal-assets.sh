#!/usr/bin/env bash
# sync-legal-assets.sh
# 将根目录的 LICENSE 和 THIRD_PARTY_NOTICES.md 拷贝到 frontend_next/public/legal/
# 用于构建时同步，确保线上页面与仓库内容一致。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PUBLIC_LEGAL="$REPO_ROOT/frontend_next/public/legal"

mkdir -p "$PUBLIC_LEGAL"

# 拷贝 LICENSE
if [ -f "$REPO_ROOT/LICENSE" ]; then
  cp "$REPO_ROOT/LICENSE" "$PUBLIC_LEGAL/LICENSE"
  echo "✅ LICENSE → public/legal/LICENSE"
else
  echo "⚠️  根目录 LICENSE 不存在，跳过" >&2
fi

# 拷贝 THIRD_PARTY_NOTICES.md
if [ -f "$REPO_ROOT/THIRD_PARTY_NOTICES.md" ]; then
  cp "$REPO_ROOT/THIRD_PARTY_NOTICES.md" "$PUBLIC_LEGAL/third-party-notices.md"
  echo "✅ THIRD_PARTY_NOTICES.md → public/legal/third-party-notices.md"
else
  echo "⚠️  根目录 THIRD_PARTY_NOTICES.md 不存在，跳过" >&2
fi

echo "Legal assets synced to $PUBLIC_LEGAL"
