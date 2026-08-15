# Desktop Windows L0 E2E

PR-1 + PR-2 of `docs/plans/2026-08-13-windows-desktop-client-e2e-journey-design.md`.
PR-1 validates an installed or hot-swapped Windows client tree, local process
ownership, cold start, API health, and graceful shutdown. PR-2 attaches to the
packaged WebView2 via CDP and drives workspace creation plus IPC upload.
Neither layer requires an LLM key.

## Requirements

- Windows PowerShell available from WSL as `powershell.exe`.
- A packaged install tree or a hot-swapped tree containing:
  - `Context-OS.exe`
  - `avrag-api.exe`
  - `avrag-worker.exe`
  - `runtime\pgsql\bin\pg_ctl.exe`
  - `runtime\redis\redis-server.exe`
  - `runtime\pgsql\lib\vector.dll`
- `DESKTOP_E2E_YES=1` to confirm the run may close the local Context-OS client.
- For `l1`, a Windows `frontend_next` checkout with win32 `@playwright/test`.
  Set `DESKTOP_E2E_WIN_FRONTEND`; `DESKTOP_E2E_WIN_NPX` is resolved from
  `where.exe npx.cmd` when omitted.

## Usage

From the WSL repo root:

```bash
DESKTOP_E2E_YES=1 bash scripts/desktop-e2e/run.sh l0
```

Audit ports and install tree without launching or closing the app:

```bash
DESKTOP_E2E_YES=1 bash scripts/desktop-e2e/run.sh audit
```

Run the packaged WebView shell + ingest journey:

```bash
DESKTOP_E2E_YES=1 \
  DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' \
  bash scripts/desktop-e2e/run.sh l1
```

Real-LLM layer (D-rag-full grounded answer with citations). Keys are mapped
from `avrag-rs/.env` into `DESKTOP_E2E_*` env vars only (KD8) — never echoed,
never committed:

```bash
DESKTOP_E2E_YES=1 \
  DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' \
  bash scripts/desktop-e2e/run.sh l2
```

Install-upgrade-install data preservation (old setup → seed workspace marker →
install new setup on top → marker must survive; also asserts the bundled
parser tree). The new installer defaults to the fresh NSIS bundle; the old one
must be given explicitly. Paths accept WSL or Windows form:

```bash
DESKTOP_E2E_YES=1 \
  DESKTOP_E2E_OLD_SETUP='C:\temp\cos-old-setup.exe' \
  bash scripts/desktop-e2e/run.sh u1
```

The old baseline must be a build that passes `l0` on its own — u1 exercises
upgrade preservation, not cold-start repair. Note the published
`dist/desktop-release/v0.2.0` setup (2026-08-10) cannot serve as the old
baseline: its API never runs migrations on cold start (`relation "users" does
not exist`), fixed later in 559c042d. Until a newer release artifact is kept,
self-upgrade (OLD = NEW = current bundle) still covers installer idempotence,
state preservation, migration idempotence, and parser-tree arrival.

`l1` starts the app with a temp `STATE_HOME`, backs up AppData files, runs
`playwright.desktop-client.config.ts`, then gracefully closes the app and
restores AppData. It also stages `antifragile.txt` under the Windows temp
run directory and enables WebView2 CDP on port 19322 by default.

The Windows frontend tree (`DESKTOP_E2E_WIN_FRONTEND`) is a snapshot of this
repo, not a live mount: playwright there cannot see spec/fixture edits made
on the WSL side. `l1`/`l2` therefore mirror `frontend_next/e2e/` (robocopy
/MIR — stale files are deleted) and `playwright.desktop-client.config.ts`
into it before every run.

PR-3 config probes run in the same L1 suite. `seed-legacy-llm.ps1` writes a
dummy `llm-config.json` with `base_url=http://127.0.0.1:9`; the specs verify
unconfigured copy, dead-endpoint fast failure, drawer readback, and that a
stored provider secret does not reroute desktop chat. L1 also injects
test-only `E2E_ENABLED=true` (disables local HTTP rate limiting) and a
temporary `BYOK_MASTER_KEY` so the provider-secret route can encrypt a dummy row.

Document parsing is provisioned by the install tree itself: `runtime/parsers/`
ships stdlib-only `markitdown-lite` / `anydoc-lite` wrappers driven by the
bundled Python, plus `runtime/parsers/lit/` (cross-built `lit.exe` +
`pdfium.dll`) for the PDF route. The desktop shell writes `MARKITDOWN_BIN` /
`ANYDOC_BIN` / `LITEPARSE_BIN` into `client.env` when they are present. No
host-side parser install is needed for text/office/PDF ingest.

Optional timeout overrides:

- `DESKTOP_E2E_CDP_PORT`
- `DESKTOP_E2E_CDP_ATTACH_TIMEOUT_MS`
- `DESKTOP_E2E_SESSION_TIMEOUT`
- `DESKTOP_E2E_HOTSWAP=1` plus `DESKTOP_E2E_HOTSWAP_MODE` to run
  `scripts/dev-windows-hotswap.sh` before the L0/L1 gate.

Use a non-default install tree:

```bash
DESKTOP_E2E_YES=1 DESKTOP_E2E_INSTALL_DIR='C:\Context-OS' \
  bash scripts/desktop-e2e/run.sh l0
```

Use a temporary data tree:

```bash
DESKTOP_E2E_YES=1 DESKTOP_E2E_STATE_HOME='/mnt/c/Users/dev/AppData/Local/cos-e2e-state' \
  bash scripts/desktop-e2e/run.sh l0
```

The Windows path conversion accepts both `C:\...` and `/mnt/c/...` forms.

## Safety contract

- Ports 5433, 6380, and 18080 are audited before any shutdown action.
- If a port belongs to a foreign tree, the run fails with `S-desktop-port-owner`
  and does not touch that process.
- If ports belong to this tree, shutdown happens only by closing the
  `Context-OS.exe` main window, which lets Tauri run its exit lifecycle.
- The harness never calls `Stop-Process`, `redis-cli SHUTDOWN`, or a Rust
  teardown function directly.
- If ports belong to this tree but no closeable main window exists, the run
  fails with `S-desktop-no-app-window`; close the client manually and retry.

## Artifacts

The PowerShell script writes:

- `%TEMP%\cos-e2e-<run-id>\l0.json`
- `%TEMP%\cos-e2e-<run-id>\signals.txt`

The script prints the exact JSON path on success and failure.

## Signals

- `S-desktop-tree`: required install files are missing.
- `S-desktop-port`: a required port did not open.
- `S-desktop-port-owner`: a port is owned by a foreign tree or data dir.
- `S-desktop-no-app-window`: same-tree ports need teardown but no main window
  can be closed.
- `S-desktop-shutdown-timeout`: ports did not release after graceful close.
- `S-desktop-env`: `client.env` is missing after cold start.
- `S-desktop-cold`: expected window title/UI state did not appear.
- `S-desktop-session`: local session JWT is not valid against the local API.
- `S-desktop-cdp`: WebView2 CDP did not become ready.
- `S-desktop-cdp-gpo`: WebView2 GPO blocks the requested CDP port.
- `S-desktop-console`: visible console process detected (soft warning).
