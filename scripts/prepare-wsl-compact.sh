#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DISTRO_NAME="${WSL_DISTRO_NAME:-Ubuntu}"
AGGRESSIVE=0
SKIP_FSTRIM=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo-root)
      REPO_ROOT="$2"
      shift 2
      ;;
    --distro-name)
      DISTRO_NAME="$2"
      shift 2
      ;;
    --aggressive)
      AGGRESSIVE=1
      shift
      ;;
    --skip-fstrim)
      SKIP_FSTRIM=1
      shift
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

echo "Running: bash scripts/rust-disk-hygiene.sh clean"
bash "${REPO_ROOT}/scripts/rust-disk-hygiene.sh" clean

if (( AGGRESSIVE == 1 )); then
  echo "Running: bash scripts/rust-disk-hygiene.sh clean --aggressive"
  bash "${REPO_ROOT}/scripts/rust-disk-hygiene.sh" clean --aggressive
fi

if (( SKIP_FSTRIM == 1 )); then
  echo "Skipping fstrim"
else
  sudo fstrim -av
fi

WINDOWS_SCRIPT="$(wslpath -w "${REPO_ROOT}/scripts/wsl-vhd-compact.ps1")"
echo "Next run in elevated PowerShell:"
printf 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%s" -DistroName %s\n' "${WINDOWS_SCRIPT}" "${DISTRO_NAME}"
