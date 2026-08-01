#!/usr/bin/env bash
# SkillOpt 集成环境安装（落地期只装依赖，不触发评测/训练）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ ! -x .venv/bin/python ]; then
  python3 -m venv .venv
fi

.venv/bin/pip install --upgrade pip
.venv/bin/pip install -r requirements.txt

echo
echo "已就绪。静态自检（不跑评测）："
echo "  bash scripts/check.sh"
echo "训练（等开发全部落地后再执行）："
echo "  .venv/bin/python train_avrag149.py --config configs/avrag149/default.yaml"
