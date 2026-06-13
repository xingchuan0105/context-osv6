#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
PUBLIC_LEGAL="$PROJECT_ROOT/public/legal"

mkdir -p "$PUBLIC_LEGAL"

if [ -f "$PROJECT_ROOT/../THIRD_PARTY_NOTICES.md" ]; then
  cp "$PROJECT_ROOT/../THIRD_PARTY_NOTICES.md" "$PUBLIC_LEGAL/third-party-notices.md"
  echo "✅ 同步 THIRD_PARTY_NOTICES.md"
else
  echo "⚠️  THIRD_PARTY_NOTICES.md 不存在"
fi

if [ -f "$PROJECT_ROOT/../LICENSE" ]; then
  cp "$PROJECT_ROOT/../LICENSE" "$PUBLIC_LEGAL/LICENSE"
  echo "✅ 同步 LICENSE"
else
  echo "⚠️  LICENSE 不存在"
fi

echo "✅ 法律资产同步完成"
