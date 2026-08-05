# Bundled data-plane runtime (portable PG + Redis)

**Design**: `docs/desktop/2026-08-04-portable-runtime-design.md`  
**Not in git**: large binaries under `windows-x64/` / `linux-x64/` (see root `.gitignore`).

## Layout (after stage)

```text
desktop/runtime/bundled/
  README.md                 # this file
  pins.env                  # version pins + public URLs
  windows-x64/              # staged for NSIS / monorepo Windows
    runtime.version
    THIRD_PARTY.txt
    pgsql/                  # PG 16 tree (bin/pg_ctl.exe, lib, share/extension/vector*)
    redis/                  # redis-server.exe (+ optional redis-cli.exe)
  linux-x64/                # optional monorepo Linux portable (BR4)
```

**Install-time layout** (NSIS via `build-windows.sh`, BR2) embeds:

```text
$INSTDIR/runtime/pgsql/
$INSTDIR/runtime/redis/
$INSTDIR/runtime/migrations/   # avrag-rs migrations
$INSTDIR/runtime/runtime.version
```

State (data + `client.env`) lives in `%LOCALAPPDATA%\Context-OS Client\` — not under Program Files.

(`native_stack`: bins = install `runtime/`, monorepo `bundled/*`; state = AppData when packaged.)

Build flags: `SKIP_BUNDLED_RUNTIME=1` for slim ~37MB setup without PG/Redis.

## Commands

```bash
# What is staged / cached
bash scripts/stage-desktop-bundled-runtime.sh status

# Download published cos-runtime zip from VPS (sha256 verified) → windows-x64/
bash scripts/stage-desktop-bundled-runtime.sh fetch

# Pack local windows-x64/ → dist/desktop-runtime/ + .sha256 + manifest.json
bash scripts/stage-desktop-bundled-runtime.sh pack

# Upload dist/desktop-runtime/ to VPS /var/www/releases/desktop/runtime/
bash scripts/publish-desktop-bundled-runtime.sh

# Optional: assemble from component downloads (PG zip + Redis zip + optional vector)
# Large download — only when you intend to rebuild the zip.
bash scripts/stage-desktop-bundled-runtime.sh assemble
```

Local cache (offline reuse): `~/.cache/context-osv6/bundled-runtime/`.

## Path resolution (runtime)

1. `PG_BIN_DIR` / `REDIS_SERVER_BIN` env  
2. Unless `COS_USE_SYSTEM_PG=1`: install `runtime/pgsql|redis`, then monorepo `bundled/{host,windows-x64,linux-x64}`, then `native/`  
3. System packages / `PATH`

## Users vs builders

| Who | Artifact |
|-----|----------|
| End user | Full `setup.exe` (runtime already inside) — no second download |
| Builder / developer | `cos-runtime-*.zip` on VPS under `/releases/desktop/runtime/` |
