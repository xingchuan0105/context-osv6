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
w("| P1 | MinIO (upload / Milvus compose) | AGPL-3.0 | Prefer cloud S3/OSS via `S3_*` env vars |")
w("| P1 | Redis **server 7.4+** (Linux SaaS/docker) | RSALv2 / SSPL | Internal cache only; pin ≤7.2 **or** use Valkey. **Not** the desktop Windows pin (see below). |")
w("| P2 | `@img/sharp-libvips-linux-x64` (Next.js web) | LGPL-3.0 | NOTICE only; desktop build uses `images.unoptimized` |")
w("| P2 | `cssparser` / `selectors` (via `scraper`) | MPL-2.0 | NOTICE; share file changes only if you modify MPL files |")
w("| P2 | `dompurify` | MPL-2.0 OR Apache-2.0 | Compliance: choose Apache-2.0 |")
w("| P2 | `markitdown[all]` transitive extras | varies | Worker image installs Microsoft markitdown (MIT core). Review extras on upgrade; do **not** reintroduce AGPL PDF stacks (e.g. PyMuPDF) without a separate legal review. |")
w("")
w("## Runtime infrastructure (not npm/cargo)")
w("")
w("| Component | Typical license | Notes |")
w("|-----------|-----------------|-------|")
w("| PostgreSQL | PostgreSQL License | Server DB (SaaS/local) and **desktop bundled** EDB Windows binaries |")
w("| pgvector | PostgreSQL License | Extension for `RETRIEVAL_BACKEND=pgvector`; also in desktop portable runtime |")
w("| Milvus | Apache-2.0 | Optional dense backend (`RETRIEVAL_BACKEND=milvus`) |")
w("| etcd | Apache-2.0 | Bundled with Milvus compose |")
w("| Redis (Linux server) | See checklist | Prefer Valkey or pre-SSPL pin for SaaS |")
w("| Paddle OCR Jobs | API Terms of Service | External SaaS, not open source |")
w("| LLM / Embedding / Search providers | API Terms of Service | DeepSeek, DashScope/SiliconFlow, Brave, etc. |")
w("")
w("## Document parsers (worker / avrag-runtime image)")
w("")
w("Baked into `avrag-runtime` (see `deploy/docker/avrag-runtime.Dockerfile`). Not Rust/npm crates.")
w("")
w("| Component | License | Role | Source |")
w("|-----------|---------|------|--------|")
w("| **markitdown** (Microsoft) | MIT | PDF / text / long-tail convert → markdown | https://github.com/microsoft/markitdown · PyPI `markitdown` |")
w("| **firecrawl-anydoc** | MIT | Office/ODF/RTF/EPUB/CSV → markdown (product non-PDF path) | https://github.com/firecrawl/anydoc · PyPI `firecrawl-anydoc` |")
w("| **anydoc-extract** | MIT (this repo) | Thin CLI wrapper over firecrawl-anydoc for the worker | `avrag-rs/scripts/anydoc-extract/` |")
w("")
w("## Desktop client — shell and bundled data-plane")
w("")
w("### Shell (Tauri 2)")
w("")
w("| Component | License | Notes |")
w("|-----------|---------|-------|")
w("| Tauri 2 + plugins (`desktop/src-tauri`) | MIT OR Apache-2.0 (upstream) | Desktop shell only; product logic is MIT Context-OS code |")
w("| WebView2 (Windows) | Microsoft software license | System/runtime dependency of Tauri on Windows |")
w("")
w("### Bundled portable runtime (optional NSIS `runtime/`)")
w("")
w("Pins: `desktop/runtime/bundled/pins.env`. Stage/pack writes `runtime/THIRD_PARTY.txt` via")
w("`scripts/stage-desktop-bundled-runtime.sh`. End-user setup may embed this tree under `$INSTDIR/runtime/`.")
w("")
w("| Component | Version pin (see pins.env) | License | Distribution notes |")
w("|-----------|----------------------------|---------|---------------------|")
w("| PostgreSQL Windows binaries | PG 16.x (EDB zip) | PostgreSQL License | Retain notices under `runtime/pgsql/` |")
w("| pgvector | 0.8.x matching PG 16 | PostgreSQL License | Upstream https://github.com/pgvector/pgvector ; Windows DLL may come from unofficial prebuild (andreiramani `pgvector_pgsql_windows`) — same license family |")
w("| Redis for Windows | **5.0.14.1** (tporadowski port) | **BSD-3-Clause** (historical Redis COPYING) | **Chosen to avoid SSPL/RSALv2**. Do not silently upgrade to Redis 7.4+ Windows builds without license review. Source: https://github.com/tporadowski/redis |")
w("")
w("Redis **desktop pin ≠ SaaS Redis**: commercial checklist P1 applies to Linux server Redis 7.4+; the client ships the BSD-era Windows port above.")
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
