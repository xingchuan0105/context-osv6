#!/usr/bin/env bash
set -euo pipefail

ENV_FILE="${1:-.env}"

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "[storage-parity] missing env file: ${ENV_FILE}" >&2
  exit 1
fi

trim() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf "%s" "${value}"
}

strip_quotes() {
  local value="$1"
  local first last
  if [[ ${#value} -ge 2 ]]; then
    first="${value:0:1}"
    last="${value: -1}"
    if [[ ("${first}" == '"' && "${last}" == '"') || ("${first}" == "'" && "${last}" == "'") ]]; then
      value="${value:1:${#value}-2}"
    fi
  fi
  printf "%s" "${value}"
}

declare -A ENV_MAP
while IFS= read -r line || [[ -n "${line}" ]]; do
  line="$(trim "${line}")"
  [[ -z "${line}" ]] && continue
  [[ "${line}" == \#* ]] && continue
  [[ "${line}" != *=* ]] && continue

  key="$(trim "${line%%=*}")"
  value="$(trim "${line#*=}")"
  value="$(strip_quotes "${value}")"
  ENV_MAP["${key}"]="${value}"
done < "${ENV_FILE}"

count_non_empty() {
  local -n keys_ref=$1
  local count=0
  local key value
  for key in "${keys_ref[@]}"; do
    value="$(trim "${ENV_MAP[${key}]:-}")"
    if [[ -n "${value}" ]]; then
      ((count += 1))
    fi
  done
  printf "%s" "${count}"
}

S3_KEYS=("S3_ENDPOINT" "S3_BUCKET" "S3_ACCESS_KEY" "S3_SECRET_KEY")

s3_set_count="$(count_non_empty S3_KEYS)"

if [[ "${s3_set_count}" != "0" && "${s3_set_count}" != "4" ]]; then
  echo "[storage-parity] mismatch: S3 config must be all set or all empty (current: ${s3_set_count}/4)." >&2
  exit 1
fi

backend="local"
if [[ "${s3_set_count}" == "4" ]]; then
  backend="s3"
fi

echo "[storage-parity] ok: API and worker resolve backend=${backend} from ${ENV_FILE}"
