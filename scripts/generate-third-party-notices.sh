#!/usr/bin/env bash
# Regenerate THIRD_PARTY_NOTICES.md from cargo-license and license-checker.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${ROOT}/THIRD_PARTY_NOTICES.md"
RUST_JSON="$(mktemp)"
FE_JSON="$(mktemp)"
trap 'rm -f "${RUST_JSON}" "${FE_JSON}"' EXIT

if ! command -v cargo-license >/dev/null 2>&1; then
  cargo install cargo-license --quiet
fi

cd "${ROOT}/avrag-rs"
cargo license --json > "${RUST_JSON}"

cd "${ROOT}/frontend_next"
if [[ ! -d node_modules/.pnpm ]]; then
  pnpm install --frozen-lockfile
fi
npx --yes license-checker --start node_modules/.pnpm --json > "${FE_JSON}"

python3 - "${OUT}" "${RUST_JSON}" "${FE_JSON}" <<'PY'
import json
import sys
from collections import defaultdict
from datetime import date
from pathlib import Path

out_path = Path(sys.argv[1])
rust = json.loads(Path(sys.argv[2]).read_text())
fe = json.loads(Path(sys.argv[3]).read_text())

lines: list[str] = []

def w(s: str = "") -> None:
    lines.append(s)

w("# Third-Party Notices")
w("")
w(f"_Generated: {date.today().isoformat()} via `scripts/generate-third-party-notices.sh`_")
w("")
w("This project (Context-OS / AVRag) is licensed under the [MIT License](LICENSE).")
w("Third-party components listed below are subject to their own licenses.")
w("")
w("## Commercial deployment checklist")
w("")
w("| Priority | Component | License | Action |")
w("|----------|-----------|---------|--------|")
w("| P0 | PyMuPDF (`pdf-visual-renderer`) | AGPL-3.0 or Artifex commercial | Do not deploy in SaaS unless licensed; leave `PDF_RENDERER_BASE_URL` unset |")
w("| P1 | MinIO (upload / Milvus compose) | AGPL-3.0 | Prefer cloud S3/OSS via `S3_*` env vars |")
w("| P1 | Redis server 7.4+ | RSALv2 / SSPL | Internal cache only; pin ≤7.2 or use Valkey |")
w("| P2 | `@img/sharp-libvips-linux-x64` (Next.js web) | LGPL-3.0 | NOTICE only; desktop build uses `images.unoptimized` |")
w("| P2 | `cssparser` / `selectors` (via `scraper`) | MPL-2.0 | NOTICE; share file changes only if you modify MPL files |")
w("| P2 | `dompurify` | MPL-2.0 OR Apache-2.0 | Compliance: choose Apache-2.0 |")
w("")
w("## Runtime infrastructure (not npm/cargo)")
w("")
w("| Component | Typical license | Notes |")
w("|-----------|-----------------|-------|")
w("| PostgreSQL | PostgreSQL License | Permissive |")
w("| Milvus | Apache-2.0 | Permissive |")
w("| etcd | Apache-2.0 | Bundled with Milvus compose |")
w("| Paddle OCR Jobs | API Terms of Service | External SaaS, not open source |")
w("| LLM / Embedding providers | API Terms of Service | DeepSeek, DashScope, Brave, etc. |")
w("")
w("## Rust dependencies (avrag-rs)")
w("")
w(f"Total crates: **{len(rust)}**")
w("")

by_rust: dict[str, list[str]] = defaultdict(list)
for p in rust:
    by_rust[p.get("license") or "UNKNOWN"].append(p["name"])

for lic in sorted(by_rust.keys(), key=lambda x: (-len(by_rust[x]), x)):
    names = sorted(set(by_rust[lic]))
    w(f"### {lic} ({len(names)} crates)")
    w("")
    for n in names:
        w(f"- {n}")
    w("")

w("## Frontend dependencies (frontend_next, transitive)")
w("")
w(f"Total packages: **{len(fe)}**")
w("")

by_fe: dict[str, list[str]] = defaultdict(list)
for pkg, info in fe.items():
    by_fe[info.get("licenses") or "UNKNOWN"].append(pkg)

for lic in sorted(by_fe.keys(), key=lambda x: (-len(by_fe[x]), x)):
    pkgs = sorted(by_fe[lic])
    w(f"### {lic} ({len(pkgs)} packages)")
    w("")
    for p in pkgs:
        w(f"- {p}")
    w("")

w("## Python sidecar (optional)")
w("")
w("`avrag-rs/services/pdf-visual-renderer/requirements.txt`:")
w("")
w("- **PyMuPDF** — Dual Licensed: GNU Affero GPL 3.0 or Artifex Commercial License")
w("- fastapi, uvicorn, pydantic, httpx, python-multipart — MIT / BSD / Apache-2.0")
w("")
w("## Regeneration")
w("")
w("```bash")
w("./scripts/generate-third-party-notices.sh")
w("./scripts/check-licenses.sh")
w("```")

out_path.write_text("\n".join(lines) + "\n")
print(f"Wrote {out_path}")
PY

echo "Done: ${OUT}"
