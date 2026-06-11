#!/bin/bash
set -euo pipefail
cd /home/chuan/context-osv6
cat > /tmp/task23-msg.txt << 'EOF'
feat(billing): add rollout flag for pricing revamp

- Hash-bucket rollout by user_id (no DB dependency)
- /api/v1/billing/usage/* endpoints gated
- Frontend fallback to old UI when disabled
- Rollout SOP doc: 10% → 50% → 100% with monitoring gates
EOF
export GIT_SEQUENCE_EDITOR="sed -i 's/^pick 8c51b01/reword 8c51b01/'"
export GIT_EDITOR='sh -c "cp /tmp/task23-msg.txt \"`$1\"" sh'
git rebase -i fcbb363