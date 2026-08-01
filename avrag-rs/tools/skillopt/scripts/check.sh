#!/usr/bin/env bash
# SkillOpt 集成静态自检（不触发评测、不调 LLM）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ ! -x .venv/bin/python ]; then
  echo "缺少 .venv —— 先跑 bash scripts/setup.sh" >&2
  exit 1
fi

exec .venv/bin/python train_avrag149.py \
  --config configs/avrag149/default.yaml \
  --check
