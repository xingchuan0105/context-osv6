#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

COMMAND="${1:-help}"
if [[ $# -gt 0 ]]; then
  shift
fi

ROOT="${DEFAULT_ROOT}"
CACHE_ROOT="${HOME}/.cache/context-osv6"
TMP_ROOT="/tmp"
SINGLE_THRESHOLD_MB=8192
TOTAL_THRESHOLD_MB=20480
KEEP_GENERATIONS=2
DRY_RUN=0
AGGRESSIVE=0

usage() {
  cat <<'EOF'
Usage:
  bash scripts/rust-disk-hygiene.sh check [options]
  bash scripts/rust-disk-hygiene.sh prune [options]
  bash scripts/rust-disk-hygiene.sh clean [options]

Note: monorepo default is local <workspace>/target (see deactivate-rust-cache.sh).
Shared cache under ~/.cache/context-osv6/target is opt-in only.

Options:
  --root PATH                 Workspace root to scan
  --cache-root PATH           Shared cache root to scan
  --tmp-root PATH             Temp directory root to scan
  --single-threshold-mb N     Warn when one directory exceeds N MiB
  --total-threshold-mb N      Warn when combined incremental/tmp cache exceeds N MiB
  --keep-generations N        Keep the newest N incremental generations per crate
  --aggressive                clean: also remove workspace target/debug and legacy target dirs
  --dry-run                   Print cleanup targets without deleting them
  -h, --help                  Show this help
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

absolute_dir() {
  local path="$1"
  if [[ ! -d "${path}" ]]; then
    die "directory does not exist: ${path}"
  fi
  (
    cd "${path}"
    pwd
  )
}

format_bytes() {
  local bytes="$1"
  if command -v numfmt >/dev/null 2>&1; then
    numfmt --to=iec-i --suffix=B "${bytes}"
  else
    echo "${bytes}B"
  fi
}

size_bytes() {
  local path="$1"
  du -s -B1 -L "${path}" 2>/dev/null | awk '{print $1}'
}

sum_dir_sizes() {
  local total=0
  local dir
  for dir in "$@"; do
    if [[ -d "${dir}" || -L "${dir}" ]]; then
      total=$((total + $(size_bytes "${dir}")))
    fi
  done
  echo "${total}"
}

workspace_target_paths() {
  printf '%s\n' \
    "${ROOT}/avrag-rs/target" \
    "${ROOT}/frontend_rust/target" \
    "${ROOT}/contracts/target"

  if [[ -d "${ROOT}/.worktrees" ]]; then
    find "${ROOT}/.worktrees" \
      \( -path '*/context-osv6/avrag-rs/target' -o -path '*/context-osv6/frontend_rust/target' \) \
      -print 2>/dev/null
  fi
}

find_incremental_dirs() {
  {
    if [[ -d "${ROOT}" ]]; then
      find -L "${ROOT}" -type d -path '*/target/debug/incremental' -print 2>/dev/null
    fi
    if [[ -d "${CACHE_ROOT}/target" ]]; then
      find "${CACHE_ROOT}/target" -type d -path '*/debug/incremental' -print 2>/dev/null
    fi
  } | while IFS= read -r path; do
    realpath -e "${path}"
  done | sort -u
}

find_debug_dirs() {
  {
    if [[ -d "${ROOT}" ]]; then
      find -L "${ROOT}" -type d -path '*/target/debug' -print 2>/dev/null
    fi
    if [[ -d "${CACHE_ROOT}/target" ]]; then
      find "${CACHE_ROOT}/target" -type d -path '*/debug' -print 2>/dev/null
    fi
  } | while IFS= read -r path; do
    realpath -e "${path}"
  done | sort -u
}

find_legacy_target_dirs() {
  local path
  while IFS= read -r path; do
    [[ -n "${path}" ]] || continue
    if [[ -d "${path}" && ! -L "${path}" ]]; then
      realpath -e "${path}"
    fi
  done < <(workspace_target_paths) | sort -u
}

find_target_links() {
  local path
  while IFS= read -r path; do
    [[ -n "${path}" ]] || continue
    if [[ -L "${path}" ]]; then
      printf '%s -> %s\n' "${path}" "$(readlink -f "${path}")"
    fi
  done < <(workspace_target_paths)
}

find_tmp_dirs() {
  find "${TMP_ROOT}" -maxdepth 1 -mindepth 1 -type d \
    \( -name 'codex-*' -o -name 'transport-http-*' \) \
    -print 2>/dev/null | while IFS= read -r path; do
      realpath -e "${path}"
    done | sort -u
}

print_section() {
  local title="$1"
  shift
  local dirs=("$@")
  local dir size

  echo "${title}"
  if [[ ${#dirs[@]} -eq 0 ]]; then
    echo "  (none)"
    echo
    return
  fi

  for dir in "${dirs[@]}"; do
    size="$(size_bytes "${dir}")"
    printf "  %8s  %s\n" "$(format_bytes "${size}")" "${dir}"
  done
  printf "  %8s  total\n" "$(format_bytes "$(sum_dir_sizes "${dirs[@]}")")"
  echo
}

threshold_messages() {
  local single_threshold_bytes=$((SINGLE_THRESHOLD_MB * 1024 * 1024))
  local total_threshold_bytes=$((TOTAL_THRESHOLD_MB * 1024 * 1024))
  local dir size
  local flagged=()
  local combined_total=0

  local incremental_dirs=()
  local tmp_dirs=()
  local debug_dirs=()
  mapfile -t incremental_dirs < <(find_incremental_dirs)
  mapfile -t tmp_dirs < <(find_tmp_dirs)
  mapfile -t debug_dirs < <(find_debug_dirs)

  for dir in "${incremental_dirs[@]}" "${tmp_dirs[@]}"; do
    [[ -d "${dir}" ]] || continue
    size="$(size_bytes "${dir}")"
    combined_total=$((combined_total + size))
    if (( size >= single_threshold_bytes )); then
      flagged+=("$(format_bytes "${size}") ${dir}")
    fi
  done

  for dir in "${debug_dirs[@]}"; do
    [[ -d "${dir}" ]] || continue
    size="$(size_bytes "${dir}")"
    if (( size >= single_threshold_bytes )); then
      flagged+=("$(format_bytes "${size}") ${dir}")
    fi
  done

  if (( ${#flagged[@]} == 0 && combined_total < total_threshold_bytes )); then
    echo "Threshold status: within configured limits."
    return
  fi

  echo "Threshold exceeded:"
  if (( combined_total >= total_threshold_bytes )); then
    echo "  Combined incremental/tmp total is $(format_bytes "${combined_total}")"
    echo "  Limit is $(format_bytes "${total_threshold_bytes}")"
  fi
  if (( ${#flagged[@]} > 0 )); then
    echo "  Large directories:"
    local item
    for item in "${flagged[@]}"; do
      echo "    ${item}"
    done
  fi
  echo "  Next:"
  echo "    Conservative prune: bash scripts/rust-disk-hygiene.sh prune"
  echo "    Conservative clean: bash scripts/rust-disk-hygiene.sh clean"
  echo "    Aggressive clean:   bash scripts/rust-disk-hygiene.sh clean --aggressive"
}

is_allowed_cleanup_target() {
  local path="$1"
  case "${path}" in
    "${ROOT}"/*/target/debug/incremental/*) return 0 ;;
    "${ROOT}"/*/target/debug/incremental) return 0 ;;
    "${CACHE_ROOT}"/target/*/debug/incremental/*) return 0 ;;
    "${CACHE_ROOT}"/target/*/debug/incremental) return 0 ;;
    "${CACHE_ROOT}"/target/*/debug)
      if (( AGGRESSIVE == 1 )); then
        return 0
      fi
      ;;
    "${ROOT}"/*/target/debug)
      if (( AGGRESSIVE == 1 )); then
        return 0
      fi
      ;;
    "${ROOT}"/*/target)
      if (( AGGRESSIVE == 1 )); then
        return 0
      fi
      ;;
    "${TMP_ROOT}"/codex-*|"${TMP_ROOT}"/transport-http-*) return 0 ;;
  esac
  return 1
}

clean_dir() {
  local path="$1"
  if ! is_allowed_cleanup_target "${path}"; then
    die "refusing to clean unexpected path: ${path}"
  fi
  if [[ ! -e "${path}" ]]; then
    return
  fi
  if (( DRY_RUN == 1 )); then
    echo "would remove ${path}"
    return
  fi
  rm -rf "${path}"
  echo "removed ${path}"
}

collect_prune_targets_for_parent() {
  local parent="$1"
  local keep="$2"
  local -A kept_counts=()
  local line name stem

  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    name="${line#* }"
    stem="${name%-*}"
    if [[ -z "${stem}" || "${stem}" == "${name}" ]]; then
      stem="${name}"
    fi
    kept_counts["${stem}"]=$(( ${kept_counts["${stem}"]:-0} + 1 ))
    if (( kept_counts["${stem}"] > keep )); then
      printf '%s/%s\n' "${parent}" "${name}"
    fi
  done < <(find "${parent}" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %P\n' 2>/dev/null | sort -nr)
}

run_check() {
  local incremental_dirs=()
  local debug_dirs=()
  local legacy_target_dirs=()
  local tmp_dirs=()
  local target_links=()

  mapfile -t incremental_dirs < <(find_incremental_dirs)
  mapfile -t debug_dirs < <(find_debug_dirs)
  mapfile -t legacy_target_dirs < <(find_legacy_target_dirs)
  mapfile -t tmp_dirs < <(find_tmp_dirs)
  mapfile -t target_links < <(find_target_links)

  echo "Workspace root: ${ROOT}"
  echo "Cache root:     ${CACHE_ROOT}"
  echo "Temp root:      ${TMP_ROOT}"
  echo "Single limit:   $(format_bytes "$((SINGLE_THRESHOLD_MB * 1024 * 1024))")"
  echo "Total limit:    $(format_bytes "$((TOTAL_THRESHOLD_MB * 1024 * 1024))")"
  echo "Keep gens:      ${KEEP_GENERATIONS}"
  echo

  echo "Workspace target links"
  if [[ ${#target_links[@]} -eq 0 ]]; then
    echo "  (none)"
  else
    printf '  %s\n' "${target_links[@]}"
  fi
  echo

  print_section "Legacy workspace-local target directories" "${legacy_target_dirs[@]}"
  print_section "Incremental caches" "${incremental_dirs[@]}"
  print_section "target/debug directories" "${debug_dirs[@]}"
  print_section "Temporary Cargo directories" "${tmp_dirs[@]}"
  threshold_messages
}

run_prune() {
  local incremental_dirs=()
  local prune_targets=()
  local seen=()
  local dir target total_bytes=0

  mapfile -t incremental_dirs < <(find_incremental_dirs)
  if [[ ${#incremental_dirs[@]} -eq 0 ]]; then
    echo "No incremental directories found."
    return
  fi

  for dir in "${incremental_dirs[@]}"; do
    while IFS= read -r target; do
      [[ -n "${target}" ]] || continue
      if [[ " ${seen[*]} " == *" ${target} "* ]]; then
        continue
      fi
      seen+=("${target}")
      prune_targets+=("${target}")
      total_bytes=$((total_bytes + $(size_bytes "${target}")))
    done < <(collect_prune_targets_for_parent "${dir}" "${KEEP_GENERATIONS}")
  done

  if [[ ${#prune_targets[@]} -eq 0 ]]; then
    echo "No stale incremental generations found."
    return
  fi

  echo "Pruning ${#prune_targets[@]} stale incremental directories."
  echo "Potential reclaim: $(format_bytes "${total_bytes}")"
  printf '  %s\n' "${prune_targets[@]}"

  if (( DRY_RUN == 1 )); then
    return
  fi

  for target in "${prune_targets[@]}"; do
    clean_dir "${target}"
  done
}

run_clean() {
  local incremental_dirs=()
  local tmp_dirs=()
  local debug_dirs=()
  local legacy_target_dirs=()
  local targets=()
  local seen=()
  local dir

  mapfile -t incremental_dirs < <(find_incremental_dirs)
  mapfile -t tmp_dirs < <(find_tmp_dirs)
  mapfile -t debug_dirs < <(find_debug_dirs)
  mapfile -t legacy_target_dirs < <(find_legacy_target_dirs)

  targets+=("${incremental_dirs[@]}")
  targets+=("${tmp_dirs[@]}")
  if (( AGGRESSIVE == 1 )); then
    targets+=("${debug_dirs[@]}")
    targets+=("${legacy_target_dirs[@]}")
  fi

  if [[ ${#targets[@]} -eq 0 ]]; then
    echo "Nothing to clean."
    return
  fi

  for dir in "${targets[@]}"; do
    [[ -e "${dir}" ]] || continue
    if [[ " ${seen[*]} " == *" ${dir} "* ]]; then
      continue
    fi
    seen+=("${dir}")
    clean_dir "${dir}"
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      ROOT="$2"
      shift 2
      ;;
    --cache-root)
      CACHE_ROOT="$2"
      shift 2
      ;;
    --tmp-root)
      TMP_ROOT="$2"
      shift 2
      ;;
    --single-threshold-mb)
      SINGLE_THRESHOLD_MB="$2"
      shift 2
      ;;
    --total-threshold-mb)
      TOTAL_THRESHOLD_MB="$2"
      shift 2
      ;;
    --keep-generations)
      KEEP_GENERATIONS="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --aggressive)
      AGGRESSIVE=1
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

ROOT="$(absolute_dir "${ROOT}")"
if [[ -d "${CACHE_ROOT}" ]]; then
  CACHE_ROOT="$(absolute_dir "${CACHE_ROOT}")"
fi
TMP_ROOT="$(absolute_dir "${TMP_ROOT}")"

case "${COMMAND}" in
  check)
    run_check
    ;;
  prune)
    run_prune
    ;;
  clean)
    run_clean
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    usage
    die "unknown command: ${COMMAND}"
    ;;
esac
