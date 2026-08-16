#!/usr/bin/env bash
# Windows packaged-client desktop E2E harness (PR-1 L0 + PR-2 L1).
#
# Usage:
#   DESKTOP_E2E_YES=1 bash scripts/desktop-e2e/run.sh l0
#   DESKTOP_E2E_YES=1 DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' bash scripts/desktop-e2e/run.sh l1
#   DESKTOP_E2E_YES=1 DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' bash scripts/desktop-e2e/run.sh l2
#   DESKTOP_E2E_YES=1 DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' bash scripts/desktop-e2e/run.sh l3
#   DESKTOP_E2E_YES=1 DESKTOP_E2E_OLD_SETUP='C:\temp\old-setup.exe' bash scripts/desktop-e2e/run.sh u1
#   DESKTOP_E2E_YES=1 DESKTOP_E2E_INSTALL_DIR='C:\Context-OS' bash scripts/desktop-e2e/run.sh l0
#
# l2 = l1 suite with real LLM keys mapped from avrag-rs/.env (KD8) and the
# default grep narrowed to D-rag-full. l3 = cloud-login acceptance: real gate
# (no bypass), no BYOK seed, RAG via the official relay (cloud creds come from
# avrag-rs/.env DESKTOP_E2E_CLOUD_*). u1 = install-upgrade-install data
# preservation run (see upgrade.ps1).
#
# Safety contract:
#   - Audits ports/data dirs before any shutdown.
#   - Shuts down only by closing the packaged Context-OS.exe main window.
#   - Never calls Stop-Process or redis-cli SHUTDOWN from this harness.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MODE="${1:-l0}"
RUN_ID="${DESKTOP_E2E_RUN_ID:-$(date +%Y%m%d-%H%M%S)-$$}"
POWERSHELL="${DESKTOP_E2E_POWERSHELL:-powershell.exe}"
YES="${DESKTOP_E2E_YES:-0}"
CDP_PORT="${DESKTOP_E2E_CDP_PORT:-19322}"
CDP_ATTACH_TIMEOUT_MS="${DESKTOP_E2E_CDP_ATTACH_TIMEOUT_MS:-120000}"

die() { echo "desktop-e2e: $*" >&2; exit 1; }
log() { echo "desktop-e2e: $*"; }

if [[ "$YES" != "1" ]]; then
  die "set DESKTOP_E2E_YES=1 to confirm this run is allowed to stop the local Context-OS client"
fi
if [[ "$MODE" != "l0" && "$MODE" != "audit" && "$MODE" != "l1" && "$MODE" != "l2" && "$MODE" != "l3" && "$MODE" != "u1" ]]; then
  die "unknown mode '$MODE' (use: l0 | audit | l1 | l2 | l3 | u1)"
fi
# l2 = l1 with real LLM keys (KD8 opt-in made explicit as a mode) and the
# default grep narrowed to the D-rag-full real-LLM journey.
if [[ "$MODE" == "l2" ]]; then
  DESKTOP_E2E_LLM=1
  DESKTOP_E2E_GREP="${DESKTOP_E2E_GREP:-D-rag-full}"
fi
# l3 = cloud-login acceptance (W3 gate 真机门): real login gate (no bypass env),
# no legacy BYOK seed — the RAG answer must come from the official relay.
if [[ "$MODE" == "l3" ]]; then
  DESKTOP_E2E_NO_BYPASS=1
  DESKTOP_E2E_GREP="${DESKTOP_E2E_GREP:-cloud-login}"
fi
if ! command -v "$POWERSHELL" >/dev/null 2>&1; then
  die "cannot find Windows PowerShell: $POWERSHELL"
fi

env_value() {
  grep -E "^$1=" "$ROOT/avrag-rs/.env" 2>/dev/null | head -n1 | cut -d= -f2- \
    | sed -e 's/^[[:space:]]*//' -e 's/^"//' -e 's/"$//' | tr -d '\r'
}

# L2 / D-rag-full real keys (KD8): opt-in via DESKTOP_E2E_LLM=1. Map avrag-rs/.env
# platform keys to DESKTOP_E2E_* (respecting pre-set vars). Values never echo.
if [[ "${DESKTOP_E2E_LLM:-0}" == "1" ]]; then
  DESKTOP_E2E_LLM_API_KEY="${DESKTOP_E2E_LLM_API_KEY:-$(env_value AGENT_LLM_API_KEY)}"
  DESKTOP_E2E_LLM_BASE_URL="${DESKTOP_E2E_LLM_BASE_URL:-$(env_value AGENT_LLM_BASE_URL)}"
  DESKTOP_E2E_LLM_MODEL="${DESKTOP_E2E_LLM_MODEL:-$(env_value AGENT_LLM_MODEL)}"
  DESKTOP_E2E_EMBED_API_KEY="${DESKTOP_E2E_EMBED_API_KEY:-$(env_value EMBEDDING_API_KEY)}"
  DESKTOP_E2E_EMBED_BASE_URL="${DESKTOP_E2E_EMBED_BASE_URL:-$(env_value EMBEDDING_BASE_URL)}"
  DESKTOP_E2E_EMBED_MODEL="${DESKTOP_E2E_EMBED_MODEL:-$(env_value EMBEDDING_MODEL)}"
fi
# L3 cloud account (mapped from avrag-rs/.env like the LLM keys; never echoed).
if [[ "$MODE" == "l3" ]]; then
  DESKTOP_E2E_CLOUD_EMAIL="${DESKTOP_E2E_CLOUD_EMAIL:-$(env_value DESKTOP_E2E_CLOUD_EMAIL)}"
  DESKTOP_E2E_CLOUD_PASSWORD="${DESKTOP_E2E_CLOUD_PASSWORD:-$(env_value DESKTOP_E2E_CLOUD_PASSWORD)}"
fi

if [[ "${DESKTOP_E2E_HOTSWAP:-0}" == "1" ]]; then
  HOTSWAP_MODE="${DESKTOP_E2E_HOTSWAP_MODE:-hotswap}"
  log "hotswap mode=$HOTSWAP_MODE"
  LAUNCH=0 \
    SKIP_FRONTEND="${DESKTOP_E2E_HOTSWAP_SKIP_FRONTEND:-0}" \
    SKIP_SIDECARS="${DESKTOP_E2E_HOTSWAP_SKIP_SIDECARS:-0}" \
    bash "$ROOT/scripts/dev-windows-hotswap.sh" "$HOTSWAP_MODE"
fi

wsl_to_win() {
  local p="$1"
  if [[ "$p" =~ ^/mnt/([a-zA-Z])/(.*)$ ]]; then
    local drive rest
    drive="${BASH_REMATCH[1],,}"
    drive="${drive^^}"
    rest="${BASH_REMATCH[2]//\//\\}"
    printf '%s' "${drive}:\\${rest}"
    return
  fi
  printf '%s' "$p"
}

ps_quote() {
  local s="$1"
  s="${s//\'/\'\'}"
  printf "'%s'" "$s"
}

L0_SCRIPT="$ROOT/scripts/desktop-e2e/l0.ps1"

if [[ "$MODE" == "l1" || "$MODE" == "l2" || "$MODE" == "l3" ]]; then
  if [[ "${DESKTOP_E2E_AUDIT_ONLY:-0}" == "1" ]]; then
    die "DESKTOP_E2E_AUDIT_ONLY=1 is incompatible with mode l1"
  fi
  if [[ -z "${DESKTOP_E2E_WIN_FRONTEND:-}" ]]; then
    die "set DESKTOP_E2E_WIN_FRONTEND to the Windows frontend_next directory (e.g. C:\\dev\\context-osv6\\frontend_next)"
  fi
  if [[ -z "${DESKTOP_E2E_WIN_NPX:-}" ]]; then
    DESKTOP_E2E_WIN_NPX="$("$POWERSHELL" -NoProfile -Command "where.exe npx.cmd 2>\$null | Select-Object -First 1" | tr -d '\r' | tail -n 1 || true)"
    if [[ -z "$DESKTOP_E2E_WIN_NPX" ]]; then
      die "could not resolve Windows npx.cmd; set DESKTOP_E2E_WIN_NPX"
    fi
  fi
  # The Windows frontend tree (C:\dev\context-osv6) is a snapshot, not a live
  # mount of this repo: specs/fixtures/pom edited on the WSL side are invisible
  # to playwright there. Mirror e2e/ + the desktop-client config into it first.
  # $ROOT is a native WSL path, so translate via wslpath (\\wsl.localhost UNC);
  # robocopy exit codes 0-7 are success, >=8 means real failure.
  command -v wslpath >/dev/null 2>&1 || die "wslpath not available (this harness requires WSL)"
  WIN_FRONTEND_WIN="$(wsl_to_win "$DESKTOP_E2E_WIN_FRONTEND")"
  WIN_E2E_SRC="$(wslpath -w "$ROOT/frontend_next/e2e" | tr -d '\r')"
  WIN_DESKTOP_CFG_SRC="$(wslpath -w "$ROOT/frontend_next/playwright.desktop-client.config.ts" | tr -d '\r')"
  SYNC_LOG="$(mktemp)"
  "$POWERSHELL" -NoProfile -Command "
    robocopy $(ps_quote "$WIN_E2E_SRC") $(ps_quote "$WIN_FRONTEND_WIN\e2e") /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
    if (\$LASTEXITCODE -ge 8) { exit \$LASTEXITCODE }
    Copy-Item -Force -ErrorAction Stop $(ps_quote "$WIN_DESKTOP_CFG_SRC") $(ps_quote "$WIN_FRONTEND_WIN\\")
    exit 0
  " >"$SYNC_LOG" 2>&1 || { cat "$SYNC_LOG" >&2; rm -f "$SYNC_LOG"; die "e2e source sync into $WIN_FRONTEND_WIN failed"; }
  rm -f "$SYNC_LOG"
  log "synced e2e sources -> $WIN_FRONTEND_WIN"
  if [[ -z "${DESKTOP_E2E_STATE_HOME:-}" ]]; then
    DESKTOP_E2E_STATE_HOME="$("$POWERSHELL" -NoProfile -Command "Write-Output (Join-Path \$env:TEMP 'cos-e2e-$RUN_ID\state')" | tr -d '\r')"
  fi
  if [[ -z "${DESKTOP_E2E_APP_DATA_BACKUP:-}" ]]; then
    DESKTOP_E2E_APP_DATA_BACKUP="$("$POWERSHELL" -NoProfile -Command "Write-Output (Join-Path \$env:TEMP 'cos-e2e-$RUN_ID')" | tr -d '\r')"
  fi
  WIN_FIXTURE="${DESKTOP_E2E_APP_DATA_BACKUP}\antifragile.txt"
  FIXTURE_SOURCE_WIN="$(wslpath -w "$ROOT/frontend_next/e2e/fixtures/antifragile.txt")"
  "$POWERSHELL" -NoProfile -Command "New-Item -ItemType Directory -Force -Path $(ps_quote "$DESKTOP_E2E_APP_DATA_BACKUP") | Out-Null; Copy-Item -Force -LiteralPath $(ps_quote "$FIXTURE_SOURCE_WIN") -Destination $(ps_quote "$WIN_FIXTURE")" \
    || die "failed to stage Windows fixture: $WIN_FIXTURE"
fi

# u1: install old → seed data → install new on top → data must survive.
# Paths may be WSL (/mnt/c/...) or Windows form; both are converted below.
if [[ "$MODE" == "u1" ]]; then
  if [[ "${DESKTOP_E2E_AUDIT_ONLY:-0}" == "1" ]]; then
    die "DESKTOP_E2E_AUDIT_ONLY=1 is incompatible with mode u1"
  fi
  if [[ -z "${DESKTOP_E2E_OLD_SETUP:-}" ]]; then
    die "set DESKTOP_E2E_OLD_SETUP to the previous version's setup.exe"
  fi
  if [[ -z "${DESKTOP_E2E_NEW_SETUP:-}" ]]; then
    DESKTOP_E2E_NEW_SETUP="$ROOT/desktop/src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/Context-OS Client_0.2.0_x64-setup.exe"
  fi
  # Existence check: /mnt/... paths check locally, C:\... paths via PowerShell.
  u1_setup_exists() {
    local p="$1"
    if [[ "$p" =~ ^/ ]]; then
      [[ -f "$p" ]]
    else
      "$POWERSHELL" -NoProfile -Command "if (Test-Path $(ps_quote "$p")) { exit 0 } else { exit 1 }"
    fi
  }
  u1_setup_exists "$DESKTOP_E2E_OLD_SETUP" || die "old setup not found: $DESKTOP_E2E_OLD_SETUP"
  u1_setup_exists "$DESKTOP_E2E_NEW_SETUP" || die "new setup not found: $DESKTOP_E2E_NEW_SETUP"
fi

ARGS=()

ARGS=()
ARGS+=("-NoProfile" "-ExecutionPolicy" "Bypass" "-File" "$L0_SCRIPT")
ARGS+=("-RunId" "$RUN_ID")
if [[ "$MODE" == "l1" || "$MODE" == "l2" || "$MODE" == "l3" ]]; then
  ARGS+=("-CdpPort" "$CDP_PORT")
  ARGS+=("-CdpTimeoutSeconds" "$((CDP_ATTACH_TIMEOUT_MS / 1000))")
fi

if [[ -n "${DESKTOP_E2E_INSTALL_DIR:-}" ]]; then
  ARGS+=("-InstallDir" "$(wsl_to_win "$DESKTOP_E2E_INSTALL_DIR")")
fi
if [[ -n "${DESKTOP_E2E_STATE_HOME:-}" ]]; then
  ARGS+=("-StateHome" "$(wsl_to_win "$DESKTOP_E2E_STATE_HOME")")
fi
if [[ -n "${DESKTOP_E2E_APP_DATA_BACKUP:-}" ]]; then
  ARGS+=("-AppDataBackupPath" "$(wsl_to_win "$DESKTOP_E2E_APP_DATA_BACKUP")")
fi
if [[ "${DESKTOP_E2E_USE_DEFAULT_TREE:-0}" == "1" ]]; then
  ARGS+=("-UseDefaultTree")
fi
if [[ "${DESKTOP_E2E_NO_BYPASS:-0}" == "1" ]]; then
  # l3: keep the W3 login gate real and skip the legacy BYOK seed — the answer
  # must come from the official relay, not a seeded provider secret.
  ARGS+=("-NoCloudGateBypass" "-NoLegacyLlmSeed")
fi
if [[ "$MODE" == "audit" || "${DESKTOP_E2E_AUDIT_ONLY:-0}" == "1" ]]; then
  ARGS+=("-AuditOnly")
fi
if [[ -n "${DESKTOP_E2E_COLD_START_TIMEOUT:-}" ]]; then
  ARGS+=("-ColdStartTimeoutSeconds" "$DESKTOP_E2E_COLD_START_TIMEOUT")
fi
if [[ -n "${DESKTOP_E2E_SHUTDOWN_TIMEOUT:-}" ]]; then
  ARGS+=("-ShutdownTimeoutSeconds" "$DESKTOP_E2E_SHUTDOWN_TIMEOUT")
fi
if [[ -n "${DESKTOP_E2E_HEALTH_TIMEOUT:-}" ]]; then
  ARGS+=("-HealthTimeoutSeconds" "$DESKTOP_E2E_HEALTH_TIMEOUT")
fi
if [[ -n "${DESKTOP_E2E_SESSION_TIMEOUT:-}" ]]; then
  ARGS+=("-SessionTimeoutSeconds" "$DESKTOP_E2E_SESSION_TIMEOUT")
fi

log "mode=$MODE run_id=$RUN_ID install_dir=${DESKTOP_E2E_INSTALL_DIR:-<default>}"

if [[ "$MODE" == "l1" || "$MODE" == "l2" || "$MODE" == "l3" ]]; then
  KEEP_ARGS=("${ARGS[@]}" "-KeepRunning")
  TEARDOWN_ARGS=("${ARGS[@]}" "-TeardownOnly")
  L1_CLEANUP_DONE=0
  cleanup_l1() {
    local code=$?
    if [[ "$L1_CLEANUP_DONE" == "1" ]]; then
      exit "$code"
    fi
    L1_CLEANUP_DONE=1
    "$POWERSHELL" "${TEARDOWN_ARGS[@]}" >/dev/null 2>&1 || true
    exit "$code"
  }
  trap cleanup_l1 EXIT INT TERM

  PS_CMD="\$env:DESKTOP_E2E_CDP_PORT='${CDP_PORT}';"
  PS_CMD+=" \$env:DESKTOP_E2E_CDP_URL='http://127.0.0.1:${CDP_PORT}';"
  PS_CMD+=" \$env:DESKTOP_E2E_CDP_ATTACH_TIMEOUT_MS='${CDP_ATTACH_TIMEOUT_MS}';"
  PS_CMD+=" \$env:DESKTOP_E2E_FIXTURE=$(ps_quote "$WIN_FIXTURE");"
  [[ -n "${DESKTOP_E2E_LLM_API_KEY:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_LLM_API_KEY=$(ps_quote "$DESKTOP_E2E_LLM_API_KEY");"
  [[ -n "${DESKTOP_E2E_LLM_BASE_URL:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_LLM_BASE_URL=$(ps_quote "$DESKTOP_E2E_LLM_BASE_URL");"
  [[ -n "${DESKTOP_E2E_LLM_MODEL:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_LLM_MODEL=$(ps_quote "$DESKTOP_E2E_LLM_MODEL");"
  [[ -n "${DESKTOP_E2E_EMBED_API_KEY:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_EMBED_API_KEY=$(ps_quote "$DESKTOP_E2E_EMBED_API_KEY");"
  [[ -n "${DESKTOP_E2E_EMBED_BASE_URL:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_EMBED_BASE_URL=$(ps_quote "$DESKTOP_E2E_EMBED_BASE_URL");"
  [[ -n "${DESKTOP_E2E_EMBED_MODEL:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_EMBED_MODEL=$(ps_quote "$DESKTOP_E2E_EMBED_MODEL");"
  [[ -n "${DESKTOP_E2E_CLOUD_EMAIL:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_CLOUD_EMAIL=$(ps_quote "$DESKTOP_E2E_CLOUD_EMAIL");"
  [[ -n "${DESKTOP_E2E_CLOUD_PASSWORD:-}" ]] && PS_CMD+=" \$env:DESKTOP_E2E_CLOUD_PASSWORD=$(ps_quote "$DESKTOP_E2E_CLOUD_PASSWORD");"
  PS_CMD+=" Set-Location $(ps_quote "$DESKTOP_E2E_WIN_FRONTEND");"
  PS_CMD+=" & $(ps_quote "$DESKTOP_E2E_WIN_NPX") playwright test --config=playwright.desktop-client.config.ts"
  [[ -n "${DESKTOP_E2E_GREP:-}" ]] && PS_CMD+=" --grep $(ps_quote "$DESKTOP_E2E_GREP")"

  set +e
  "$POWERSHELL" "${KEEP_ARGS[@]}"
  KEEP_STATUS=$?
  if [[ "$KEEP_STATUS" -eq 0 ]]; then
    "$POWERSHELL" -NoProfile -ExecutionPolicy Bypass -Command "$PS_CMD"
    TEST_STATUS=$?
  else
    TEST_STATUS=1
  fi
  "$POWERSHELL" "${TEARDOWN_ARGS[@]}"
  TEARDOWN_STATUS=$?
  L1_CLEANUP_DONE=1
  set -e

  if [[ "$KEEP_STATUS" -ne 0 || "$TEST_STATUS" -ne 0 || "$TEARDOWN_STATUS" -ne 0 ]]; then
    log "l1 failed keep=$KEEP_STATUS tests=$TEST_STATUS teardown=$TEARDOWN_STATUS"
    exit 1
  fi
  log "done: $MODE"
  exit 0
fi

if [[ "$MODE" == "u1" ]]; then
  U1_ARGS=(
    "-NoProfile" "-ExecutionPolicy" "Bypass"
    "-File" "$ROOT/scripts/desktop-e2e/upgrade.ps1"
    "-RunId" "$RUN_ID"
    "-OldSetup" "$(wsl_to_win "$DESKTOP_E2E_OLD_SETUP")"
    "-NewSetup" "$(wsl_to_win "$DESKTOP_E2E_NEW_SETUP")"
  )
  if [[ -n "${DESKTOP_E2E_STATE_HOME:-}" ]]; then
    U1_ARGS+=("-StateHome" "$(wsl_to_win "$DESKTOP_E2E_STATE_HOME")")
  fi
  "$POWERSHELL" "${U1_ARGS[@]}"
  log "done: $MODE"
  exit 0
fi

"$POWERSHELL" "${ARGS[@]}"
log "done: $MODE"
