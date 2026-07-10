#!/usr/bin/env bash
# Default Rust layout: per-workspace local target/ (Cargo default).
# Removes shared-target symlinks and target-dir overrides written by activate-rust-cache.sh.
# Does NOT delete ~/.cache/context-osv6/target (opt-in prune; see rust-disk-hygiene.sh).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CACHE_ROOT="${HOME}/.cache/context-osv6"
WORKTREES_ROOT="${REPO_ROOT}/.worktrees"
APPLY_WORKTREES=0
DRY_RUN=0

usage() {
  cat <<'EOF'
Usage:
  bash scripts/deactivate-rust-cache.sh [options]

Restore Cargo default: compile into <workspace>/target (not shared cache).

Options:
  --repo-root PATH       Workspace root (default: repo root of this script)
  --cache-root PATH      Shared cache root (for messages only)
  --worktrees-root PATH  Also process checkouts under this root
  --apply-worktrees      Walk --worktrees-root for context-osv6 checkouts
  --dry-run              Print actions only
  -h, --help             Show help
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo-root) REPO_ROOT="$2"; shift 2 ;;
    --cache-root) CACHE_ROOT="$2"; shift 2 ;;
    --worktrees-root) WORKTREES_ROOT="$2"; shift 2 ;;
    --apply-worktrees) APPLY_WORKTREES=1; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

run() {
  if (( DRY_RUN == 1 )); then
    echo "dry-run: $*"
  else
    eval "$@"
  fi
}

# Strip target-dir from local-machine.toml; keep other keys (SWAGGER_UI, sccache, profiles).
strip_target_dir_from_local_machine() {
  local local_machine="$1"
  [[ -f "${local_machine}" ]] || return 0

  if ! grep -qE '^\s*target-dir\s*=' "${local_machine}"; then
    echo "no target-dir in ${local_machine}"
    return 0
  fi

  if (( DRY_RUN == 1 )); then
    echo "dry-run: remove target-dir from ${local_machine}"
    return 0
  fi

  local tmp
  tmp="$(mktemp)"
  # Drop target-dir lines; leave rest intact.
  grep -vE '^\s*target-dir\s*=' "${local_machine}" > "${tmp}" || true
  mv "${tmp}" "${local_machine}"
  echo "removed target-dir from ${local_machine}"
}

unlink_shared_target() {
  local workspace_root="$1"
  local workspace_target="${workspace_root}/target"
  local cache_prefix="${CACHE_ROOT}/target/"

  if [[ ! -e "${workspace_target}" && ! -L "${workspace_target}" ]]; then
    echo "no target at ${workspace_target}"
    return 0
  fi

  if [[ -L "${workspace_target}" ]]; then
    local dest
    dest="$(readlink "${workspace_target}")"
    if [[ "${dest}" == "${cache_prefix}"* ]] || [[ "${dest}" == "${CACHE_ROOT}/target/"* ]]; then
      if (( DRY_RUN == 1 )); then
        echo "dry-run: rm symlink ${workspace_target} -> ${dest}"
      else
        rm -f "${workspace_target}"
        echo "removed shared target symlink: ${workspace_target} (was -> ${dest})"
      fi
      return 0
    fi
    echo "leave non-cache symlink: ${workspace_target} -> ${dest}"
    return 0
  fi

  if [[ -d "${workspace_target}" ]]; then
    echo "local directory target already present: ${workspace_target}"
  fi
}

deactivate_checkout() {
  local checkout_root="$1"
  local workspace_root

  for workspace_root in "${checkout_root}/avrag-rs" "${checkout_root}/frontend_rust"; do
    [[ -d "${workspace_root}" ]] || continue
    echo "==> ${workspace_root}"
    strip_target_dir_from_local_machine "${workspace_root}/.cargo/local-machine.toml"
    unlink_shared_target "${workspace_root}"
  done
}

echo "Default policy: local target/ per workspace (no shared CARGO target-dir)."
echo "Shared cache (if any) left at ${CACHE_ROOT}/target — prune with scripts/rust-disk-hygiene.sh"
echo

deactivate_checkout "${REPO_ROOT}"

if (( APPLY_WORKTREES == 1 )) && [[ -d "${WORKTREES_ROOT}" ]]; then
  while IFS= read -r worktree_checkout; do
    deactivate_checkout "${worktree_checkout}"
  done < <(find "${WORKTREES_ROOT}" -mindepth 2 -maxdepth 2 -type d -name context-osv6 2>/dev/null | sort)
fi

echo
echo "Done. Next cargo build will use <workspace>/target."
echo "Optional free disk: bash scripts/rust-disk-hygiene.sh check"
echo "  then carefully prune ${CACHE_ROOT}/target if no longer needed."
