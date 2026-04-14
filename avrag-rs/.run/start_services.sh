#!/usr/bin/env bash
set -euo pipefail
cd /home/chuan/context-osv6/avrag-rs
mkdir -p .run
while IFS= read -r line; do
  line="${line%$'\r'}"
  [[ -z "$line" || "$line" =~ ^# ]] && continue
  [[ "$line" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]] || continue
  export "$line"
done < .env

if [[ -n "${FORCE_ANSWER_LLM_TIMEOUT_MS:-}" ]]; then
  export ANSWER_LLM_TIMEOUT_MS="$FORCE_ANSWER_LLM_TIMEOUT_MS"
fi

nohup cargo run -p avrag-api > .run/api.log 2>&1 &
echo $! > .run/api.pid
nohup cargo run -p avrag-worker > .run/worker.log 2>&1 &
echo $! > .run/worker.pid

echo "started api=$(cat .run/api.pid) worker=$(cat .run/worker.pid) answer_timeout=${ANSWER_LLM_TIMEOUT_MS:-unset}"
