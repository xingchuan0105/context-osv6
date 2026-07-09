#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CACHE_ROOT="${HOME}/.cache/context-osv6"
WORKTREES_ROOT="${REPO_ROOT}/.worktrees"
MIGRATE_MAIN_TARGETS=1
MIGRATE_WORKTREE_TARGETS=0

usage() {
  cat <<'EOF'
Usage:
  bash scripts/activate-rust-cache.sh [options]

Options:
  --repo-root PATH               Workspace root to configure
  --cache-root PATH              Shared cache root
  --worktrees-root PATH          Worktrees root
  --no-migrate-main-targets      Only write Cargo overrides; do not move main targets
  --migrate-worktree-targets     Also move existing worktree-local targets into shared cache
  -h, --help                     Show this help
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo-root)
      REPO_ROOT="$2"
      shift 2
      ;;
    --cache-root)
      CACHE_ROOT="$2"
      shift 2
      ;;
    --worktrees-root)
      WORKTREES_ROOT="$2"
      shift 2
      ;;
    --no-migrate-main-targets)
      MIGRATE_MAIN_TARGETS=0
      shift
      ;;
    --migrate-worktree-targets)
      MIGRATE_WORKTREE_TARGETS=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

mkdir -p "${CACHE_ROOT}/target/avrag-rs" \
         "${CACHE_ROOT}/sccache"

workspace_cache_target() {
  local workspace_root="$1"
  case "$(basename "${workspace_root}")" in
    avrag-rs) echo "${CACHE_ROOT}/target/avrag-rs" ;;
        *)
      die "unsupported workspace for cache activation: ${workspace_root}"
      ;;
  esac
}

write_local_machine() {
  local workspace_root="$1"
  local target_dir="$2"
  local local_machine="${workspace_root}/.cargo/local-machine.toml"

  mkdir -p "${workspace_root}/.cargo"
  {
    echo "[build]"
    printf 'target-dir = "%s"\n' "${target_dir}"
    if command -v sccache >/dev/null 2>&1; then
      echo 'rustc-wrapper = "sccache"'
    fi
    echo
    echo "[profile.dev]"
    echo "debug = 0"
    echo "incremental = true"
    echo
    echo "[profile.test]"
    echo "debug = 0"
    echo "incremental = true"
    if command -v sccache >/dev/null 2>&1; then
      echo
      echo "[env]"
      printf 'SCCACHE_DIR = { value = "%s", force = true }\n' "${CACHE_ROOT}/sccache"
    fi
  } > "${local_machine}"

  echo "wrote ${local_machine}"
}

migrate_workspace_target() {
  local workspace_root="$1"
  local target_dir="$2"
  local workspace_target="${workspace_root}/target"
  local target_parent
  local entries=()

  target_parent="$(dirname "${target_dir}")"
  mkdir -p "${target_parent}"

  if [[ -L "${workspace_target}" ]]; then
    echo "target already symlinked: ${workspace_target} -> $(readlink "${workspace_target}")"
    return
  fi

  if [[ -e "${target_dir}" && ! -d "${target_dir}" ]]; then
    die "cache target exists but is not a directory: ${target_dir}"
  fi

  if [[ -d "${target_dir}" ]]; then
    shopt -s dotglob nullglob
    entries=("${target_dir}"/*)
    shopt -u dotglob nullglob
    if [[ ${#entries[@]} -eq 0 ]]; then
      rmdir "${target_dir}"
    elif [[ ${#entries[@]} -eq 1 && "$(basename "${entries[0]}")" == ".rustc_info.json" ]]; then
      rm -f "${target_dir}/.rustc_info.json"
      rmdir "${target_dir}"
    else
      echo "cache target already populated, leaving ${workspace_target} untouched"
      return
    fi
  fi

  if [[ -d "${workspace_target}" ]]; then
    mv "${workspace_target}" "${target_dir}"
    ln -s "${target_dir}" "${workspace_target}"
    echo "migrated ${workspace_target} -> ${target_dir}"
    return
  fi

  mkdir -p "${target_dir}"
  ln -s "${target_dir}" "${workspace_target}"
  echo "created ${workspace_target} -> ${target_dir}"
}

configure_checkout() {
  local checkout_root="$1"
  local migrate_targets="$2"
  local workspace_root target_dir

  for workspace_root in "${checkout_root}/avrag-rs"; do
    [[ -d "${workspace_root}" ]] || continue
    target_dir="$(workspace_cache_target "${workspace_root}")"
    write_local_machine "${workspace_root}" "${target_dir}"
    if (( migrate_targets == 1 )); then
      migrate_workspace_target "${workspace_root}" "${target_dir}"
    fi
  done
}

configure_checkout "${REPO_ROOT}" "${MIGRATE_MAIN_TARGETS}"

if [[ -d "${WORKTREES_ROOT}" ]]; then
  while IFS= read -r worktree_checkout; do
    configure_checkout "${worktree_checkout}" "${MIGRATE_WORKTREE_TARGETS}"
  done < <(find "${WORKTREES_ROOT}" -mindepth 2 -maxdepth 2 -type d -name context-osv6 | sort)
fi
