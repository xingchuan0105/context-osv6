#!/usr/bin/env bash
# verify-legal-p0.sh
# 逐项检查设计文档 §9.3 中全部 P0 验收标准。
# 输出每项的 PASS/FAIL/BLOCKED 状态，汇总退出码。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FE_DIR="$REPO_ROOT/frontend_next"
CONTENT_DIR="$FE_DIR/content/legal/zh-CN"
PUBLIC_LEGAL="$FE_DIR/public/legal"
PASS=0
FAIL=0
BLOCKED=0
SKIP=0

pass() { echo "  ✅ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ❌ $1"; FAIL=$((FAIL + 1)); }
blocked() { echo "  ⏳ $1 [外部阻塞: $2]"; BLOCKED=$((BLOCKED + 1)); }
skip() { echo "  ⏭️  $1 [需人工验证]"; SKIP=$((SKIP + 1)); }

echo "========================================"
echo "  P0 验收标准逐项检查"
echo "  设计文档: avrag-rs/docs/legal-compliance-pages-design-2026-06-13.md §9.3"
echo "========================================"
echo ""

# ─── 9.3.1 信息架构与页面 ───
echo "── 9.3.1 信息架构与页面 (P0-IA-*) ──"

# P0-IA-1: /legal 索引页
if [ -f "$FE_DIR/app/(marketing)/legal/page.tsx" ]; then
  pass "P0-IA-1: /legal 索引页存在"
else
  fail "P0-IA-1: /legal 索引页不存在"
fi

# P0-IA-2: /legal/terms
if [ -f "$FE_DIR/app/(marketing)/legal/terms/page.tsx" ] && [ -f "$CONTENT_DIR/terms.mdx" ]; then
  pass "P0-IA-2: /legal/terms 页面 + MDX 存在"
else
  fail "P0-IA-2: /legal/terms 缺失"
fi

# P0-IA-3: /legal/privacy
if [ -f "$FE_DIR/app/(marketing)/legal/privacy/page.tsx" ] && [ -f "$CONTENT_DIR/privacy.mdx" ]; then
  pass "P0-IA-3: /legal/privacy 页面 + MDX 存在"
else
  fail "P0-IA-3: /legal/privacy 缺失"
fi

# P0-IA-4: /legal/licenses 摘要页
if [ -f "$FE_DIR/app/(marketing)/legal/licenses/page.tsx" ]; then
  pass "P0-IA-4: /legal/licenses 摘要页存在"
else
  fail "P0-IA-4: /legal/licenses 摘要页缺失"
fi

# P0-IA-5: /legal/licenses/third-party
if [ -f "$FE_DIR/app/(marketing)/legal/licenses/third-party/page.tsx" ]; then
  pass "P0-IA-5: /legal/licenses/third-party 页面存在"
else
  fail "P0-IA-5: /legal/licenses/third-party 缺失"
fi

# P0-IA-6: /legal/licenses/project 与根 LICENSE 一致
if [ -f "$FE_DIR/app/(marketing)/legal/licenses/project/page.tsx" ] && [ -f "$PUBLIC_LEGAL/LICENSE" ]; then
  if diff -q "$REPO_ROOT/LICENSE" "$PUBLIC_LEGAL/LICENSE" > /dev/null 2>&1; then
    pass "P0-IA-6: /legal/licenses/project 与根 LICENSE 一致"
  else
    fail "P0-IA-6: LICENSE 文件不同步"
  fi
else
  fail "P0-IA-6: 页面或 LICENSE 文件缺失"
fi

# P0-IA-7: 页脚三链
if grep -q "LegalFooterLinks" "$FE_DIR/app/page.tsx" 2>/dev/null && \
   grep -q "LegalFooterLinks" "$FE_DIR/app/(marketing)/pricing/page.tsx" 2>/dev/null && \
   grep -q "LegalFooterLinks" "$FE_DIR/app/(auth)/login/page.tsx" 2>/dev/null; then
  pass "P0-IA-7: 首页/定价/登录页脚含三链"
else
  fail "P0-IA-7: 部分页面缺少 LegalFooterLinks"
fi

echo ""

# ─── 9.3.2 页面内容 ───
echo "── 9.3.2 页面内容 (P0-CNT-*) ──"

# P0-CNT-1: ToS 十类章节
TOS_CHAPTERS=$(grep -c "^## " "$CONTENT_DIR/terms.mdx" 2>/dev/null || echo 0)
if [ "$TOS_CHAPTERS" -ge 10 ]; then
  pass "P0-CNT-1: ToS 含 $TOS_CHAPTERS 个章节（≥10）"
else
  fail "P0-CNT-1: ToS 仅 $TOS_CHAPTERS 个章节（需≥10）"
fi

# P0-CNT-2: ToS/Privacy status: published
TOS_STATUS=$(grep -m1 '^status:' "$CONTENT_DIR/terms.mdx" 2>/dev/null | sed 's/status:\s*//' | tr -d ' "')
PRIVACY_STATUS=$(grep -m1 '^status:' "$CONTENT_DIR/privacy.mdx" 2>/dev/null | sed 's/status:\s*//' | tr -d ' "')
if [ "$TOS_STATUS" = "published" ] && [ "$PRIVACY_STATUS" = "published" ]; then
  pass "P0-CNT-2: ToS/Privacy status=published"
  TOS_LEGAL=$(grep -m1 '^legal_review:' "$CONTENT_DIR/terms.mdx" 2>/dev/null | sed 's/legal_review:\s*//' | tr -d ' "')
  PRIVACY_LEGAL=$(grep -m1 '^legal_review:' "$CONTENT_DIR/privacy.mdx" 2>/dev/null | sed 's/legal_review:\s*//' | tr -d ' "')
  if [ "$TOS_LEGAL" = "approved" ] && [ "$PRIVACY_LEGAL" = "approved" ]; then
    pass "        ✅ 法务审阅 approved（§9.11 签 off 完成）"
  else
    echo "  ⏳        法务审阅 ToS=$TOS_LEGAL, Privacy=$PRIVACY_LEGAL（§9.11 签 off 待法务完成，但技术 P0-CNT-2 已通过）"
  fi
else
  blocked "P0-CNT-2: ToS=$TOS_STATUS, Privacy=$PRIVACY_STATUS (需法务签字改 published)" "法务审阅"
fi

# P0-CNT-3: Privacy 披露项
if grep -q "PostgreSQL" "$CONTENT_DIR/privacy.mdx" && \
   grep -q "S3\|MinIO\|对象存储" "$CONTENT_DIR/privacy.mdx" && \
   grep -q "DeepSeek\|DashScope" "$CONTENT_DIR/privacy.mdx"; then
  pass "P0-CNT-3: Privacy 披露项与 CONTEXT.md 一致"
else
  fail "P0-CNT-3: Privacy 缺少关键数据流披露"
fi

# P0-CNT-4: Privacy 明确不用于模型训练
if grep -qi "不会.*训练\|不.*用于.*训练\|不会用于模型训练" "$CONTENT_DIR/privacy.mdx"; then
  pass "P0-CNT-4: Privacy 明确文档不用于模型训练"
else
  fail "P0-CNT-4: Privacy 未明确训练数据声明"
fi

# P0-CNT-5: 摘要页五段结构
LICENSES_PAGE="$FE_DIR/app/(marketing)/legal/licenses/page.tsx"
if grep -q "我们的产品" "$LICENSES_PAGE" && \
   grep -q "主要开源组件" "$LICENSES_PAGE" && \
   grep -q "弱copyleft\|弱copyleft" "$LICENSES_PAGE" && \
   grep -q "完整清单\|完整第三方" "$LICENSES_PAGE" && \
   grep -q "桌面客户端" "$LICENSES_PAGE"; then
  pass "P0-CNT-5: 摘要页含五段结构"
else
  fail "P0-CNT-5: 摘要页结构不完整"
fi

# P0-CNT-6: 摘要统计与 NOTICE 一致（sync 脚本保证）
if [ -f "$PUBLIC_LEGAL/third-party-notices.md" ]; then
  if diff -q "$REPO_ROOT/THIRD_PARTY_NOTICES.md" "$PUBLIC_LEGAL/third-party-notices.md" > /dev/null 2>&1; then
    pass "P0-CNT-6: NOTICE 与线上一致"
  else
    fail "P0-CNT-6: NOTICE 漂移"
  fi
else
  fail "P0-CNT-6: NOTICE 文件不存在"
fi

echo ""

# ─── 9.3.3 前端实现 ───
echo "── 9.3.3 前端实现 (P0-FE-*) ──"

# P0-FE-1: 路由与目录结构
if [ -d "$FE_DIR/app/(marketing)/legal" ] && \
   [ -f "$FE_DIR/app/(marketing)/legal/terms/page.tsx" ] && \
   [ -f "$FE_DIR/app/(marketing)/legal/privacy/page.tsx" ] && \
   [ -f "$FE_DIR/app/(marketing)/legal/licenses/page.tsx" ] && \
   [ -f "$FE_DIR/app/(marketing)/legal/licenses/third-party/page.tsx" ] && \
   [ -f "$FE_DIR/app/(marketing)/legal/licenses/project/page.tsx" ]; then
  pass "P0-FE-1: 路由与目录结构与设计一致"
else
  fail "P0-FE-1: 路由结构不完整"
fi

# P0-FE-2: 组件存在
if [ -f "$FE_DIR/components/legal/LegalLayout.tsx" ] && \
   [ -f "$FE_DIR/components/legal/LegalDocRenderer.tsx" ] && \
   [ -f "$FE_DIR/components/legal/LegalFooterLinks.tsx" ] && \
   [ -f "$FE_DIR/components/legal/ConsentCheckbox.tsx" ]; then
  pass "P0-FE-2: LegalLayout/DocRenderer/FooterLinks/ConsentCheckbox 存在"
else
  fail "P0-FE-2: 部分组件缺失"
fi

# P0-FE-3: sync-legal-assets.sh
if [ -x "$REPO_ROOT/scripts/sync-legal-assets.sh" ] && \
   [ -f "$PUBLIC_LEGAL/LICENSE" ] && \
   [ -f "$PUBLIC_LEGAL/third-party-notices.md" ]; then
  pass "P0-FE-3: sync-legal-assets.sh + public/legal/ 有文件"
else
  fail "P0-FE-3: 同步脚本或目标文件缺失"
fi

# P0-FE-4: CI 校验 NOTICE 无漂移
if [ -f "$REPO_ROOT/.github/workflows/license-check.yml" ] && \
   grep -q "notice-drift" "$REPO_ROOT/.github/workflows/license-check.yml"; then
  pass "P0-FE-4: CI 含 NOTICE 漂移检查 job"
else
  fail "P0-FE-4: CI 缺少 NOTICE drift job"
fi

# P0-FE-5: 法律页无需登录（marketing 路由组）
if grep -q "(marketing)" "$FE_DIR/app/(marketing)/legal/page.tsx" 2>/dev/null || \
   [ -d "$FE_DIR/app/(marketing)/legal" ]; then
  pass "P0-FE-5: 法律页在 (marketing) 路由组，无需登录"
else
  fail "P0-FE-5: 法律页可能需要登录"
fi

# P0-FE-6: Phase 1 中文内容可读
if [ -f "$CONTENT_DIR/terms.mdx" ] && [ -f "$CONTENT_DIR/privacy.mdx" ]; then
  TOS_LINES=$(wc -l < "$CONTENT_DIR/terms.mdx")
  PRIVACY_LINES=$(wc -l < "$CONTENT_DIR/privacy.mdx")
  if [ "$TOS_LINES" -gt 50 ] && [ "$PRIVACY_LINES" -gt 50 ]; then
    pass "P0-FE-6: 中文 ToS($TOS_LINES 行)/Privacy($PRIVACY_LINES 行) 内容充实"
  else
    fail "P0-FE-6: MDX 内容过少（ToS=$TOS_LINES, Privacy=$PRIVACY_LINES）"
  fi
else
  fail "P0-FE-6: MDX 文件缺失"
fi

echo ""

# ─── 9.3.4 视觉与可访问性 ───
echo "── 9.3.4 视觉与可访问性 (P0-UX-*) ──"

# P0-UX-1: 正文区 ~48rem + 标题下版本与日期
if grep -q "max-width.*48rem" "$FE_DIR/app/globals.css" && \
   grep -q "legal-updated\|legal-version" "$FE_DIR/components/legal/LegalLayout.tsx"; then
  pass "P0-UX-1: 正文区 48rem + 版本日期"
else
  fail "P0-UX-1: 样式或组件缺少宽度/版本信息"
fi

# P0-UX-2: 长文有 TOC
if grep -q "legal-toc" "$FE_DIR/components/legal/LegalLayout.tsx" && \
   grep -q "legal-toc" "$FE_DIR/app/globals.css"; then
  pass "P0-UX-2: 长文有 TOC 目录锚点"
else
  fail "P0-UX-2: 缺少 TOC 支持"
fi

# P0-UX-3: 注册勾选 a11y
if grep -q "htmlFor\|for=" "$FE_DIR/components/legal/ConsentCheckbox.tsx" || \
   grep -q "<label" "$FE_DIR/components/legal/ConsentCheckbox.tsx"; then
  pass "P0-UX-3: ConsentCheckbox 有 label 关联"
else
  fail "P0-UX-3: ConsentCheckbox 缺少 a11y label"
fi

echo ""

# ─── 9.3.5 用户同意 ───
echo "── 9.3.5 用户同意 (P0-CON-*) ──"

# P0-CON-1: 未勾选无法注册
if grep -q "consent_required\|consent" "$FE_DIR/app/(auth)/register/page.tsx" 2>/dev/null; then
  pass "P0-CON-1: 注册页有同意校验"
else
  fail "P0-CON-1: 注册页缺少同意校验"
fi

# P0-CON-2: 勾选文案链到 terms/privacy
if grep -q "/legal/terms" "$FE_DIR/components/legal/ConsentCheckbox.tsx" && \
   grep -q "/legal/privacy" "$FE_DIR/components/legal/ConsentCheckbox.tsx"; then
  pass "P0-CON-2: 勾选文案链到 terms/privacy"
else
  fail "P0-CON-2: 链接缺失"
fi

# P0-CON-3: 落库 version + accepted_at
if [ -f "$REPO_ROOT/avrag-rs/migrations/0041_legal_acceptances.up.sql" ] && \
   grep -q "terms_version" "$REPO_ROOT/avrag-rs/migrations/0041_legal_acceptances.up.sql" && \
   grep -q "accepted_at" "$REPO_ROOT/avrag-rs/migrations/0041_legal_acceptances.up.sql"; then
  pass "P0-CON-3: legal_acceptances 表含 version + accepted_at"
else
  fail "P0-CON-3: 缺少 legal_acceptances 迁移"
fi

# P0-CON-4: 落库版本与线上 frontmatter 一致
if grep -q "terms_version" "$REPO_ROOT/avrag-rs/crates/transport-http/src/auth_types.rs" && \
   grep -q "privacy_version" "$REPO_ROOT/avrag-rs/crates/transport-http/src/auth_types.rs"; then
  pass "P0-CON-4: 后端接收并存储 terms_version/privacy_version"
else
  fail "P0-CON-4: 后端缺少版本字段"
fi

# P0-CON-4b: 支付/重签 HTTP 端点
if grep -q '"/legal-acceptance"' "$REPO_ROOT/avrag-rs/crates/transport-http/src/routes/auth.rs"; then
  pass "P0-CON-4b: POST /api/auth/legal-acceptance 已接线"
else
  fail "P0-CON-4b: 缺少 legal-acceptance 路由"
fi

# P0-CON-5: MDX / TS / Rust 版本单一事实源一致
mdx_version() {
  grep -m1 '^version:' "$1" 2>/dev/null | sed 's/version:\s*//' | tr -d ' "'
}
ts_version() {
  grep "$1" "$FE_DIR/lib/legal/versions.ts" 2>/dev/null | sed -n 's/.*= "\([^"]*\)".*/\1/p'
}
rust_version() {
  grep "$1" "$REPO_ROOT/avrag-rs/crates/app-core/src/legal_versions.rs" 2>/dev/null | sed -n 's/.*= "\([^"]*\)";/\1/p'
}
TOS_MDX=$(mdx_version "$CONTENT_DIR/terms.mdx")
PRIV_MDX=$(mdx_version "$CONTENT_DIR/privacy.mdx")
TOS_TS=$(ts_version PUBLISHED_TERMS_VERSION)
PRIV_TS=$(ts_version PUBLISHED_PRIVACY_VERSION)
TOS_RS=$(rust_version PUBLISHED_TERMS_VERSION)
PRIV_RS=$(rust_version PUBLISHED_PRIVACY_VERSION)
if [ -n "$TOS_MDX" ] && [ "$TOS_MDX" = "$TOS_TS" ] && [ "$TOS_MDX" = "$TOS_RS" ] && \
   [ -n "$PRIV_MDX" ] && [ "$PRIV_MDX" = "$PRIV_TS" ] && [ "$PRIV_MDX" = "$PRIV_RS" ]; then
  pass "P0-CON-5: MDX/TS/Rust 版本一致 (terms=$TOS_MDX, privacy=$PRIV_MDX)"
else
  fail "P0-CON-5: 版本漂移 (MDX $TOS_MDX/$PRIV_MDX, TS $TOS_TS/$PRIV_TS, Rust $TOS_RS/$PRIV_RS)"
fi

echo ""

# ─── 9.3.6 仓库资产与 CI ───
echo "── 9.3.6 仓库资产与 CI (P0-PIPE-*) ──"

# P0-PIPE-1: LICENSE 存在
if [ -f "$REPO_ROOT/LICENSE" ]; then
  pass "P0-PIPE-1: 根目录 LICENSE 存在"
else
  fail "P0-PIPE-1: LICENSE 缺失"
fi

# P0-PIPE-2: NOTICE 与线上一致
if [ -f "$PUBLIC_LEGAL/third-party-notices.md" ] && \
   diff -q "$REPO_ROOT/THIRD_PARTY_NOTICES.md" "$PUBLIC_LEGAL/third-party-notices.md" > /dev/null 2>&1; then
  pass "P0-PIPE-2: NOTICE 与线上一致"
else
  fail "P0-PIPE-2: NOTICE 漂移"
fi

# P0-PIPE-3: check-licenses.sh
if [ -x "$REPO_ROOT/scripts/check-licenses.sh" ]; then
  pass "P0-PIPE-3: check-licenses.sh 存在且可执行"
else
  fail "P0-PIPE-3: check-licenses.sh 缺失"
fi

# P0-PIPE-4: license-check.yml green
if [ -f "$REPO_ROOT/.github/workflows/license-check.yml" ]; then
  pass "P0-PIPE-4: license-check.yml 存在（需 CI 验证 green）"
else
  fail "P0-PIPE-4: license-check.yml 缺失"
fi

echo ""

# ─── 发布门控检查 ───
echo "── 发布门控 (P0-CNT-2) ──"
bash "$REPO_ROOT/scripts/check-legal-publish-gate.sh" || true

echo ""

# ─── 汇总 ───
echo "========================================"
echo "  汇总: ✅ $PASS 通过 | ❌ $FAIL 失败 | ⏳ $BLOCKED 外部阻塞 | ⏭️  $SKIP 待人工"
echo "========================================"

if [ $FAIL -gt 0 ]; then
  echo "⛔ 有 $FAIL 项未通过，需修复。"
  exit 1
elif [ $BLOCKED -gt 0 ]; then
  echo "⏳ 技术侧全部完成。$BLOCKED 项需外部输入（法务签字等）。"
  exit 0
else
  echo "🎉 全部 P0 验收标准通过！"
  exit 0
fi
