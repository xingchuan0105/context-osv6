#!/usr/bin/env bash
# License compliance gate: fail on copyleft/unknown third-party Rust deps.
# Regenerate notices: ./scripts/generate-third-party-notices.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_JSON="$(mktemp)"
FE_JSON="$(mktemp)"
trap 'rm -f "${RUST_JSON}" "${FE_JSON}"' EXIT

if ! command -v cargo-license >/dev/null 2>&1; then
  echo "Installing cargo-license..."
  cargo install cargo-license --quiet
fi

echo "==> Rust dependency license scan (avrag-rs)"
cd "${ROOT}/avrag-rs"
cargo license --json > "${RUST_JSON}"
python3 - "${RUST_JSON}" <<'PY'
import json, sys
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text())
blocked = []
review = []

workspace_prefixes = ("avrag-", "app", "analytics", "ingestion", "storage-", "common", "contracts")
workspace_names = {
    "avrag-auth", "avrag-cache-redis", "avrag-llm", "avrag-share",
    "avrag-test-kit", "storage-local", "contracts",
}

for pkg in data:
    name = pkg.get("name", "")
    lic = pkg.get("license") or "UNKNOWN"
    upper = lic.upper()
    if any(x in upper for x in ("AGPL", "GPL")) and "LGPL" not in upper and "MPL" not in upper:
        blocked.append((name, lic))
    elif "SSPL" in upper or "BUSL" in upper:
        blocked.append((name, lic))
    elif lic == "UNKNOWN":
        if name.startswith(workspace_prefixes) or name in workspace_names:
            continue
        review.append((name, lic))

if blocked:
    print("ERROR: blocked licenses in Rust dependency tree:", file=sys.stderr)
    for n, l in sorted(blocked):
        print(f"  {n}: {l}", file=sys.stderr)
    sys.exit(1)

if review:
    print("ERROR: UNKNOWN license on third-party crates:", file=sys.stderr)
    for n, l in sorted(review):
        print(f"  {n}: {l}", file=sys.stderr)
    sys.exit(1)

print(f"OK: {len(data)} Rust crates scanned; no GPL/AGPL/SSPL/BUSL; no unexpected UNKNOWN")
PY

echo "==> Frontend production dependency scan (frontend_next)"
cd "${ROOT}/frontend_next"
if [[ ! -d node_modules/.pnpm ]]; then
  pnpm install --frozen-lockfile
fi
npx --yes license-checker --production --json > "${FE_JSON}"
python3 - "${FE_JSON}" <<'PY'
import json, sys
from pathlib import Path

data = json.loads(Path(sys.argv[1]).read_text())
blocked = []
for pkg, info in data.items():
    lic = info.get("licenses") or "UNKNOWN"
    upper = lic.upper()
    if "AGPL" in upper or ("GPL" in upper and "LGPL" not in upper and "MPL" not in upper):
        blocked.append((pkg, lic))
    if "SSPL" in upper:
        blocked.append((pkg, lic))

if blocked:
    print("ERROR: blocked licenses in frontend production deps:", file=sys.stderr)
    for n, l in sorted(blocked):
        print(f"  {n}: {l}", file=sys.stderr)
    sys.exit(1)

print(f"OK: {len(data)} frontend production packages scanned")
PY

echo "==> Optional sidecar reminder"
if grep -q 'PyMuPDF' "${ROOT}/avrag-rs/services/pdf-visual-renderer/requirements.txt" 2>/dev/null; then
  echo "NOTE: pdf-visual-renderer uses PyMuPDF (AGPL-3.0 or commercial)."
  echo "      Do not deploy in commercial SaaS unless licensed or replaced."
  echo "      Production: leave PDF_RENDERER_BASE_URL unset to skip E-class fallback."
fi

echo "License gate passed."
