#!/usr/bin/env bash
set -euo pipefail
cd /home/chuan/context-osv6
export GIT_EDITOR=true

git commit -F .git/COMMIT_MSG_PR4.txt
echo "PR-4: $(git log -1 --oneline)"

git add \
  avrag-rs/crates/app/src/agents/capability/schemas.rs \
  avrag-rs/crates/app/src/agents/capability/mod.rs \
  avrag-rs/crates/app/src/agents/capability/registry.rs \
  avrag-rs/crates/app/src/agents/strategy/chat.rs \
  avrag-rs/crates/app/src/agents/strategy/rag.rs \
  avrag-rs/crates/app/src/agents/strategy/search.rs \
  avrag-rs/crates/app/tests/strategy_chat.rs
git commit -F .git/COMMIT_MSG_PR5.txt
echo "PR-5: $(git log -1 --oneline)"

git add \
  avrag-rs/crates/billing/src/quota_service.rs \
  avrag-rs/crates/billing/src/lib.rs \
  avrag-rs/crates/app/src/lib_impl/chat_private.rs \
  frontend_next/components/settings/settings-surface.tsx \
  frontend_next/e2e/specs/billing/usage-meter.spec.ts \
  frontend_next/e2e/specs/billing/usage-settings.spec.ts \
  frontend_next/e2e/pom/settings-page.ts \
  CONTEXT.md
git commit -F .git/COMMIT_MSG_PR6.txt
echo "PR-6: $(git log -1 --oneline)"

git status --short
git log -3 --oneline
